use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    process::{Command, Output},
    sync::{Arc, Mutex},
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

fn faus() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_faus"));
    command.env_remove("FAUS_BASE_URL");
    command.env_remove("FAUS_TEST_BROWSER_OPEN");
    command
}

#[test]
fn serve_help_exposes_runtime_flags() {
    let output = faus()
        .args(["serve", "--help"])
        .output()
        .expect("faus help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--host"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--dev"));
    assert!(!stdout.to_lowercase().contains("vite"));
}

#[test]
fn serve_rejects_base_url_without_starting_runtime() {
    let output = faus()
        .args(["--base-url", "http://127.0.0.1:53210", "serve"])
        .output()
        .expect("faus should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--base-url"));
    assert!(stderr.contains("--host"));
    assert!(stderr.contains("--port"));
}

#[test]
fn serve_json_is_rejected_until_streaming_contract_exists() {
    let output = faus()
        .args(["--json", "serve"])
        .output()
        .expect("faus should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("serve --json"));
}

#[test]
fn status_help_exposes_client_flags() {
    let output = faus()
        .args(["status", "--help"])
        .output()
        .expect("faus status help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--base-url"));
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("--debug"));
}

#[test]
fn web_help_exposes_client_flags() {
    let output = faus()
        .args(["web", "--help"])
        .output()
        .expect("faus web help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--base-url"));
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("--debug"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .args(["--json", "status"])
        .output()
        .expect("faus status should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(payload["data"]["health"]["status"], "ok");
    assert_eq!(
        payload["data"]["runtime_status"]["qdrant"]["status"],
        "runtime_unavailable"
    );
    assert_eq!(server.requests(), vec!["/health", "/runtime/status"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(payload["data"]["web_url"], server.base_url);
    assert_eq!(payload["data"]["opened"], true);
    assert_eq!(payload["data"]["server_started"], false);
    assert_eq!(server.requests(), vec!["/health", "/"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args(["--base-url", &base_url_with_slash, "--json", "status"])
        .output()
        .expect("faus status should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/health", "/runtime/status"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &base_url_with_slash, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(payload["data"]["web_url"], flag_server.base_url);
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/health", "/"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_debug_json_includes_request_metadata() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "status",
        ])
        .output()
        .expect("faus status should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["debug"]["base_url_source"], "flag");
    assert_eq!(
        payload["debug"]["health_url"],
        format!("{}/health", server.base_url)
    );
    assert_eq!(payload["debug"]["health_status"], 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_debug_json_includes_request_metadata() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &server.base_url, "--debug", "--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["debug"]["base_url_source"], "flag");
    assert_eq!(
        payload["debug"]["health_url"],
        format!("{}/health", server.base_url)
    );
    assert_eq!(payload["debug"]["web_url"], server.base_url);
    assert_eq!(payload["debug"]["health_status"], 200);
    assert_eq!(payload["debug"]["web_status"], 200);
    assert_eq!(payload["debug"]["startup"], "connected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_human_output_is_short_summary() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "status"])
        .output()
        .expect("faus status should run");

    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains(&format!("Base URL: {}", server.base_url)));
    assert!(stdout.contains("App: ok"));
    assert!(stdout.contains("Qdrant: runtime_unavailable"));
    assert!(stdout.contains("local_sidecar=available"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_opener_failure_still_prints_url() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "fail")
        .args(["--base-url", &server.base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["opened"], false);
    assert_eq!(payload["data"]["web_url"], server.base_url);
}

#[test]
fn status_invalid_base_url_returns_json_error() {
    let output = faus()
        .args(["--json", "--base-url", "ftp://example.com", "status"])
        .output()
        .expect("faus status should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["error"]["code"], "invalid_base_url");
}

#[test]
fn web_invalid_base_url_returns_json_error() {
    let output = faus()
        .args(["--json", "--base-url", "ftp://example.com", "web"])
        .output()
        .expect("faus web should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["error"]["code"], "invalid_base_url");
}

#[test]
fn web_explicit_unreachable_server_returns_connection_failed() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral port should bind");
    let base_url = format!(
        "http://{}",
        listener
            .local_addr()
            .expect("ephemeral address should be available")
    );
    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &base_url, "--json", "web"])
        .output()
        .expect("faus web should run");
    drop(listener);

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "connection_failed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "status"])
        .output()
        .expect("faus status should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["error"]["code"], "runtime_unavailable");
    assert_eq!(payload["error"]["message"], "Qdrant is offline");
    assert_eq!(payload["error"]["details"]["component"], "qdrant");
    assert_eq!(payload["error"]["retryable"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_reports_missing_assets_from_root() {
    let server = StatusServer::start(RuntimeMode::WebAssetsMissing).await;

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &server.base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "web_assets_missing");
    assert_eq!(payload["error"]["details"]["http_status"], 503);
    assert_eq!(server.requests(), vec!["/health", "/"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_rejects_missing_runtime_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "status"])
        .output()
        .expect("faus status should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_rejects_non_json_response() {
    let server = StatusServer::start(RuntimeMode::NotJson).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "status"])
        .output()
        .expect("faus status should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[derive(Clone, Copy)]
enum RuntimeMode {
    Ok,
    ErrorEnvelope,
    MissingData,
    NotJson,
    WebAssetsMissing,
}

struct StatusServer {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl StatusServer {
    async fn start(runtime_mode: RuntimeMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener.local_addr().expect("local address should exist");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = StatusServerState {
            requests: requests.clone(),
            runtime_mode,
        };
        let app = Router::new()
            .route("/health", get(health))
            .route("/", get(web_root))
            .route("/runtime/status", get(runtime_status))
            .with_state(state);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });

        Self {
            base_url: format!("http://{address}"),
            requests,
            shutdown: Some(shutdown_tx),
            handle,
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

impl Drop for StatusServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.handle.abort();
    }
}

#[derive(Clone)]
struct StatusServerState {
    requests: Arc<Mutex<Vec<String>>>,
    runtime_mode: RuntimeMode,
}

impl StatusServerState {
    fn record(&self, path: &str) {
        self.requests
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(path.to_string());
    }
}

async fn health(State(state): State<StatusServerState>) -> Response {
    state.record("/health");
    Json(json!({
        "service": "fauni-search",
        "status": "ok",
        "env": "test",
        "libraries": 1,
        "jobs": 0,
    }))
    .into_response()
}

async fn web_root(State(state): State<StatusServerState>) -> Response {
    state.record("/");
    match state.runtime_mode {
        RuntimeMode::WebAssetsMissing => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Web assets are not built. Expected ui/dist/index.html.",
        )
            .into_response(),
        _ => (
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            "<!doctype html><html><body><div id=\"app\"></div></body></html>",
        )
            .into_response(),
    }
}

async fn runtime_status(State(state): State<StatusServerState>) -> Response {
    state.record("/runtime/status");
    match state.runtime_mode {
        RuntimeMode::Ok | RuntimeMode::WebAssetsMissing => Json(json!({
            "data": {
                "app": {
                    "component_id": "app",
                    "status": "available",
                },
                "qdrant": {
                    "component_id": "qdrant",
                    "status": "runtime_unavailable",
                    "message": "Qdrant is not reachable",
                },
                "providers": [
                    {
                        "provider_id": "local_sidecar",
                        "status": "available",
                    }
                ],
            }
        }))
        .into_response(),
        RuntimeMode::ErrorEnvelope => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": {
                    "code": "runtime_unavailable",
                    "message": "Qdrant is offline",
                    "details": {
                        "component": "qdrant"
                    },
                    "retryable": true,
                }
            })),
        )
            .into_response(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
    }
}
