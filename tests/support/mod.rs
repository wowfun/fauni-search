#![allow(dead_code)]

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    Router,
};
use fauni_search::{build_app, new_state};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
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
];

pub struct TestEnv {
    _env_lock: MutexGuard<'static, ()>,
    restore: EnvRestore,
    pub runtime_dir: PathBuf,
}

impl TestEnv {
    pub async fn new(name: &str) -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test env mutex should not be poisoned");
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

        Self {
            _env_lock: lock,
            restore,
            runtime_dir,
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
