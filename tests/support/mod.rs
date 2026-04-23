#![allow(dead_code)]

use axum::{
    body::{to_bytes, Body},
    extract::{Json, Path as AxumPath, State},
    http::{header, Method, Request, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use fauni_search::{build_app, new_state};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle, time::Duration};
use tower::util::ServiceExt;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const TEST_ENV_KEYS: &[&str] = &[
    "APP_RUNTIME_DIR",
    "APP_HOST",
    "APP_PORT",
    "SIDECAR_HOST",
    "SIDECAR_PORT",
    "QDRANT_URL",
    "FAUNI_ENV",
    "EMBEDDING_MODEL_ID",
    "EMBEDDING_MODEL_REVISION",
    "FAUNI_TEST_SIDECAR_EMBED_DELAY_MS",
];

pub struct TestEnv {
    _env_lock: MutexGuard<'static, ()>,
    restore: EnvRestore,
    pub runtime_dir: PathBuf,
    _sidecar_stub: SidecarStub,
    _qdrant_stub: Option<QdrantStub>,
}

impl TestEnv {
    pub async fn new(name: &str) -> Self {
        Self::new_inner(name, false).await
    }

    pub async fn new_with_qdrant(name: &str) -> Self {
        Self::new_inner(name, true).await
    }

    async fn new_inner(name: &str, with_qdrant: bool) -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let runtime_dir = unique_test_path(name);
        fs::create_dir_all(&runtime_dir).expect("test runtime dir should be creatable");

        let restore = EnvRestore::capture(TEST_ENV_KEYS);
        env::set_var("APP_RUNTIME_DIR", &runtime_dir);
        env::set_var("APP_HOST", "127.0.0.1");
        env::set_var("APP_PORT", "39010");
        env::set_var("SIDECAR_HOST", "127.0.0.1");
        env::set_var("SIDECAR_PORT", "39011");
        env::set_var("QDRANT_URL", "http://127.0.0.1:63999");
        env::set_var("FAUNI_ENV", "test");
        env::set_var("EMBEDDING_MODEL_ID", "athrael-soju/colqwen3.5-4.5B-v3");
        env::set_var("EMBEDDING_MODEL_REVISION", "main");
        let sidecar_stub = SidecarStub::start("127.0.0.1:39011").await;
        let qdrant_stub = if with_qdrant {
            Some(QdrantStub::start("127.0.0.1:63999").await)
        } else {
            None
        };

        Self {
            _env_lock: lock,
            restore,
            runtime_dir,
            _sidecar_stub: sidecar_stub,
            _qdrant_stub: qdrant_stub,
        }
    }

    pub async fn boot(&self) -> TestApp {
        let state = new_state().await.expect("test state should bootstrap");
        TestApp {
            app: build_app(state),
        }
    }

    pub fn create_dir(&self, relative_path: &str) -> PathBuf {
        let path = self.runtime_dir.join(relative_path);
        fs::create_dir_all(&path).expect("test fixture dir should be creatable");
        path
    }

    pub fn write_bytes(&self, relative_path: &str, bytes: &[u8]) -> PathBuf {
        let path = self.runtime_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test fixture parent dir should be creatable");
        }
        fs::write(&path, bytes).expect("test fixture file should be writable");
        path
    }

    pub fn write_test_pdf(&self, relative_path: &str, page_count: usize) -> PathBuf {
        let path = self.runtime_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test fixture parent dir should be creatable");
        }

        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let resources_id = document.add_object(dictionary! {});

        let mut page_refs = Vec::new();
        for _ in 0..page_count {
            let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
            let page_id = document.new_object_id();
            let page = dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            };
            document.objects.insert(page_id, Object::Dictionary(page));
            page_refs.push(Object::Reference(page_id));
        }

        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_refs,
                "Count" => page_count as i64,
            }),
        );
        document.objects.insert(
            catalog_id,
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);
        document.compress();
        document.save(&path).expect("test PDF should be writable");
        path
    }

    pub fn repo_path(&self, relative_path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path)
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.restore.restore();
        let _ = fs::remove_dir_all(&self.runtime_dir);
    }
}

pub struct TestApp {
    app: Router,
}

impl TestApp {
    pub async fn get_json(&self, path: &str) -> TestResponse {
        self.request(
            Request::builder()
                .method(Method::GET)
                .uri(path)
                .body(Body::empty())
                .expect("GET request should build"),
        )
        .await
    }

    pub async fn post_json(&self, path: &str, body: Value) -> TestResponse {
        self.request(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&body).expect("JSON request body should serialize"),
                ))
                .expect("POST request should build"),
        )
        .await
    }

    pub async fn post_empty(&self, path: &str) -> TestResponse {
        self.request(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .body(Body::empty())
                .expect("POST request should build"),
        )
        .await
    }

    pub async fn patch_json(&self, path: &str, body: Value) -> TestResponse {
        self.request(
            Request::builder()
                .method(Method::PATCH)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&body).expect("JSON request body should serialize"),
                ))
                .expect("PATCH request should build"),
        )
        .await
    }

    pub async fn delete(&self, path: &str) -> TestResponse {
        self.request(
            Request::builder()
                .method(Method::DELETE)
                .uri(path)
                .body(Body::empty())
                .expect("DELETE request should build"),
        )
        .await
    }

    pub async fn post_multipart(
        &self,
        path: &str,
        fields: Vec<(String, String)>,
        file: Option<MultipartFile>,
    ) -> TestResponse {
        let files = file.into_iter().collect::<Vec<_>>();
        self.post_multipart_with_files(path, fields, files).await
    }

    pub async fn post_multipart_with_files(
        &self,
        path: &str,
        fields: Vec<(String, String)>,
        files: Vec<MultipartFile>,
    ) -> TestResponse {
        let boundary = "fauni-search-test-boundary";
        let mut body = Vec::new();

        for (name, value) in fields {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }

        for file in files {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                    file.field_name, file.filename
                )
                .as_bytes(),
            );
            body.extend_from_slice(
                format!("Content-Type: {}\r\n\r\n", file.content_type).as_bytes(),
            );
            body.extend_from_slice(&file.bytes);
            body.extend_from_slice(b"\r\n");
        }

        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        self.request(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("multipart request should build"),
        )
        .await
    }

    async fn request(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .app
            .clone()
            .oneshot(request)
            .await
            .expect("router request should succeed");
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable")
            .to_vec();

        TestResponse {
            status,
            headers,
            body,
        }
    }
}

pub struct MultipartFile {
    pub field_name: String,
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub struct TestResponse {
    pub status: StatusCode,
    #[allow(dead_code)]
    pub headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

impl TestResponse {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("response body should be valid JSON")
    }
}

struct EnvRestore {
    values: BTreeMap<&'static str, Option<String>>,
}

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        let values = keys
            .iter()
            .map(|key| (*key, env::var(key).ok()))
            .collect::<BTreeMap<_, _>>();
        Self { values }
    }

    fn restore(&self) {
        for (key, value) in &self.values {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

fn unique_test_path(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis();
    let pid = std::process::id();
    let sanitized = name.replace('/', "-");
    Path::new("target")
        .join("test-runtime")
        .join(format!("{sanitized}-{pid}-{millis}"))
}

struct SidecarStub {
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl SidecarStub {
    async fn start(address: &str) -> Self {
        let app = Router::new()
            .route("/capabilities", get(sidecar_capabilities))
            .route("/embed", post(sidecar_embed));
        let listener = TcpListener::bind(address)
            .await
            .expect("sidecar test stub should bind");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });
        Self {
            shutdown: Some(shutdown_tx),
            task,
        }
    }
}

impl Drop for SidecarStub {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

async fn sidecar_capabilities() -> impl IntoResponse {
    Json(json!({
        "runtime_kind": "local_python",
        "status": "ok",
        "availability": {
            "can_service": true,
            "load_error": null
        },
        "embedding_capabilities": {
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false
        },
        "execution_input_types": ["text", "image", "document", "video"],
        "runtime_adapters": [
            "document_query_via_page_images",
            "video_query_via_frame_images"
        ],
        "operations": [
            sidecar_operation_payload("query_embedding"),
            sidecar_operation_payload("image_query_embedding"),
            sidecar_operation_payload("video_query_embedding"),
            sidecar_operation_payload("document_query_embedding"),
            sidecar_operation_payload("document_embedding")
        ]
    }))
}

async fn sidecar_embed(Json(payload): Json<Value>) -> impl IntoResponse {
    let delay_ms = env::var("FAUNI_TEST_SIDECAR_EMBED_DELAY_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    if delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    let operation_kind = payload
        .get("operation_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let embeddings = match operation_kind {
        "query_embedding" => vec![json!({
            "vectors": [[0.1_f32, 0.2, 0.3], [0.4, 0.5, 0.6]],
            "pooled_vector": [0.25_f32, 0.35, 0.45]
        })],
        "image_query_embedding" => vec![json!({
            "vectors": [[1.0_f32, 2.0, 3.0]],
            "pooled_vector": [1.0_f32, 2.0, 3.0]
        })],
        "video_query_embedding" => vec![json!({
            "vectors": [[4.0_f32, 5.0, 6.0], [7.0, 8.0, 9.0]],
            "pooled_vector": [5.5_f32, 6.5, 7.5]
        })],
        "document_query_embedding" => vec![json!({
            "vectors": [[10.0_f32, 11.0, 12.0]],
            "pooled_vector": [10.0_f32, 11.0, 12.0]
        })],
        "document_embedding" => payload
            .pointer("/inputs/documents")
            .and_then(Value::as_array)
            .map(|documents| {
                documents
                    .iter()
                    .enumerate()
                    .map(|(index, document)| {
                        let base = 13.0_f32 + index as f32;
                        json!({
                            "path": document.get("path").cloned().unwrap_or(Value::Null),
                            "source_type": "pdf",
                            "kind": "document_page",
                            "locator": document.get("locator").cloned().unwrap_or(Value::Null),
                            "vectors": [[base, base + 1.0, base + 2.0]],
                            "pooled_vector": [base, base + 1.0, base + 2.0]
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        _ => vec![json!({
            "vectors": [[0.0_f32, 0.0, 0.0]],
            "pooled_vector": [0.0_f32, 0.0, 0.0]
        })],
    };

    Json(json!({ "data": { "embeddings": embeddings } }))
}

fn sidecar_operation_payload(operation_kind: &str) -> Value {
    json!({
        "operation_kind": operation_kind,
        "supported": true,
        "model": {
            "model_id": "athrael-soju/colqwen3.5-4.5B-v3",
            "revision": "main"
        }
    })
}

#[derive(Clone, Default)]
struct QdrantStubState {
    aliases: BTreeMap<String, String>,
    collections: BTreeMap<String, QdrantCollection>,
}

#[derive(Clone, Default)]
struct QdrantCollection {
    vector_size: usize,
    points: BTreeMap<u64, Value>,
}

struct QdrantStub {
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl QdrantStub {
    async fn start(address: &str) -> Self {
        let state = Arc::new(Mutex::new(QdrantStubState::default()));
        let app = Router::new()
            .route("/collections", get(qdrant_list_collections))
            .route("/aliases", get(qdrant_list_aliases))
            .route("/collections/aliases", post(qdrant_update_aliases))
            .route(
                "/collections/:collection_name",
                get(qdrant_get_collection)
                    .put(qdrant_create_collection)
                    .delete(qdrant_delete_collection),
            )
            .route(
                "/collections/:collection_name/points",
                put(qdrant_upsert_points),
            )
            .route(
                "/collections/:collection_name/points/query",
                post(qdrant_query_points),
            )
            .route(
                "/collections/:collection_name/points/delete",
                post(qdrant_delete_points),
            )
            .with_state(state);
        let listener = TcpListener::bind(address)
            .await
            .expect("qdrant test stub should bind");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });
        Self {
            shutdown: Some(shutdown_tx),
            task,
        }
    }
}

impl Drop for QdrantStub {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

async fn qdrant_list_collections(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
) -> impl IntoResponse {
    let state = state.lock().unwrap();
    let collections = state
        .collections
        .keys()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();
    Json(json!({ "result": { "collections": collections } }))
}

async fn qdrant_list_aliases(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
) -> impl IntoResponse {
    let state = state.lock().unwrap();
    let aliases = state
        .aliases
        .iter()
        .map(|(alias_name, collection_name)| {
            json!({
                "alias_name": alias_name,
                "collection_name": collection_name,
            })
        })
        .collect::<Vec<_>>();
    Json(json!({ "result": { "aliases": aliases } }))
}

async fn qdrant_update_aliases(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if let Some(actions) = payload.get("actions").and_then(Value::as_array) {
        for action in actions {
            if let Some(alias_name) = action
                .pointer("/delete_alias/alias_name")
                .and_then(Value::as_str)
            {
                state.aliases.remove(alias_name);
            }
            if let (Some(alias_name), Some(collection_name)) = (
                action
                    .pointer("/create_alias/alias_name")
                    .and_then(Value::as_str),
                action
                    .pointer("/create_alias/collection_name")
                    .and_then(Value::as_str),
            ) {
                state
                    .aliases
                    .insert(alias_name.to_string(), collection_name.to_string());
            }
        }
    }
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn qdrant_get_collection(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
) -> impl IntoResponse {
    let state = state.lock().unwrap();
    let Some(collection) = state.collections.get(&collection_name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "status": { "error": "not found" } })),
        );
    };

    (
        StatusCode::OK,
        Json(json!({
            "result": {
                "config": {
                    "params": {
                        "vectors": {
                            "mv": {
                                "size": collection.vector_size
                            }
                        }
                    }
                }
            }
        })),
    )
}

async fn qdrant_create_collection(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let init_from = payload
        .pointer("/init_from/collection")
        .and_then(Value::as_str)
        .map(str::to_string);
    let vector_size = payload
        .pointer("/vectors/mv/size")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default();

    let mut state = state.lock().unwrap();
    let mut collection = init_from
        .and_then(|source| state.collections.get(&source).cloned())
        .unwrap_or_default();
    collection.vector_size = vector_size.max(collection.vector_size);
    state.collections.insert(collection_name, collection);

    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn qdrant_delete_collection(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    state.collections.remove(&collection_name);
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn qdrant_upsert_points(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    let collection = state.collections.entry(collection_name).or_default();
    if let Some(points) = payload.get("points").and_then(Value::as_array) {
        for point in points {
            let Some(point_id) = point.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if let Some(point_payload) = point.get("payload") {
                collection.points.insert(point_id, point_payload.clone());
            }
        }
    }

    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn qdrant_query_points(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let state = state.lock().unwrap();
    let resolved_name = state
        .aliases
        .get(&collection_name)
        .cloned()
        .unwrap_or(collection_name);
    let points = state
        .collections
        .get(&resolved_name)
        .map(|collection| {
            collection
                .points
                .values()
                .enumerate()
                .map(|(index, point_payload)| {
                    json!({
                        "score": 1.0_f32 - (index as f32 * 0.1_f32),
                        "payload": point_payload,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(points.len());

    (
        StatusCode::OK,
        Json(json!({
            "result": {
                "points": points.into_iter().take(limit).collect::<Vec<_>>()
            }
        })),
    )
}

async fn qdrant_delete_points(
    State(state): State<Arc<Mutex<QdrantStubState>>>,
    AxumPath(collection_name): AxumPath<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    if let Some(collection) = state.collections.get_mut(&collection_name) {
        if let Some(point_ids) = payload.get("points").and_then(Value::as_array) {
            for point_id in point_ids.iter().filter_map(Value::as_u64) {
                collection.points.remove(&point_id);
            }
        }
    }

    (StatusCode::OK, Json(json!({ "status": "ok" })))
}
