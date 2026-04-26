use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex},
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

fn faus() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_faus"));
    command.env_remove("FAUS_BASE_URL");
    command.env_remove("FAUS_TEST_BROWSER_OPEN");
    command.env_remove("FAUS_TEST_WEB_EXIT_AFTER_READY");
    command.env_remove("FAUNI_ENV_FILE");
    command
}

#[test]
fn top_help_describes_cli_and_examples() {
    let output = faus().arg("--help").output().expect("faus help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("FauniSearch product CLI"));
    assert!(stdout.contains("Use a FauniSearch App API base URL"));
    assert!(stdout.contains("Print stable machine-readable JSON"));
    assert!(stdout.contains("Examples:"));
    assert!(stdout.contains("faus serve"));
    assert!(stdout.contains("faus library list"));
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
    assert!(stdout.contains("does not start local processes"));
    assert!(stdout.contains("faus --json status"));
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
    assert!(stdout.contains("does not start Vite"));
    assert!(stdout.contains("faus --json web"));
}

#[test]
fn library_help_exposes_workflows() {
    let output = faus()
        .args(["library", "--help"])
        .output()
        .expect("faus library help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for command in ["list", "create", "show", "rename", "archive", "restore"] {
        assert!(stdout.contains(command), "missing {command} in help");
    }
    assert!(stdout.contains("Manage libraries through the App API"));
    assert!(stdout.contains("faus library create --display-name"));
    assert!(!stdout.contains("delete"));
}

#[test]
fn library_create_help_describes_inputs() {
    let output = faus()
        .args(["library", "create", "--help"])
        .output()
        .expect("faus library create help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--display-name"));
    assert!(stdout.contains("Human-facing library display name"));
    assert!(stdout.contains("--library-id"));
    assert!(stdout.contains("Optional stable library id"));
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
    let web_port = free_port();
    let env_file = write_web_env(web_port);
    let expected_web_url = format!("http://127.0.0.1:{web_port}");

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .env("FAUS_TEST_WEB_EXIT_AFTER_READY", "1")
        .env("FAUNI_ENV_FILE", &env_file)
        .args(["--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(payload["data"]["web_url"], expected_web_url);
    assert_eq!(payload["data"]["opened"], true);
    assert_eq!(payload["data"]["server_started"], false);
    assert_eq!(server.requests(), vec!["/health"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_list_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .args(["--json", "library", "list"])
        .output()
        .expect("faus library list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(payload["data"]["libraries"][0]["id"], "demo");
    assert_eq!(server.requests(), vec!["/libraries"]);
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
    let web_port = free_port();
    let env_file = write_web_env(web_port);
    let expected_web_url = format!("http://127.0.0.1:{web_port}");

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .env("FAUS_TEST_WEB_EXIT_AFTER_READY", "1")
        .env("FAUNI_ENV_FILE", &env_file)
        .args(["--base-url", &base_url_with_slash, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(payload["data"]["web_url"], expected_web_url);
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/health"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_server_serves_dist_and_proxies_api_requests() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let web_port = free_port();
    let env_file = write_web_env(web_port);

    let mut child = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .env("FAUNI_ENV_FILE", &env_file)
        .args(["--base-url", &server.base_url, "--json", "web"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("faus web should spawn");

    let payload = read_child_json_line(&mut child);
    let web_url = payload["data"]["web_url"]
        .as_str()
        .expect("web_url should be a string")
        .to_string();
    assert_eq!(web_url, format!("http://127.0.0.1:{web_port}"));

    let client = reqwest::Client::new();
    let root = client
        .get(&web_url)
        .send()
        .await
        .expect("web root should respond");
    assert_eq!(root.status(), StatusCode::OK);
    let content_type = root
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.starts_with("text/html"));
    assert!(root
        .text()
        .await
        .expect("web root should have body")
        .contains("<div id=\"app\"></div>"));

    let libraries = client
        .get(format!("{web_url}/libraries"))
        .send()
        .await
        .expect("proxied libraries should respond");
    assert_eq!(libraries.status(), StatusCode::OK);
    let libraries_body: Value = libraries
        .json()
        .await
        .expect("proxied libraries should be JSON");
    assert_eq!(libraries_body["data"]["libraries"][0]["id"], "demo");

    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(server.requests(), vec!["/health", "/libraries"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args([
            "--base-url",
            &base_url_with_slash,
            "--json",
            "library",
            "show",
            "demo",
        ])
        .output()
        .expect("faus library show should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(payload["data"]["library"]["id"], "demo");
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/libraries/demo"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_ignores_ambient_proxy_env() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("proxy port should bind");
    let proxy_url = format!(
        "http://{}",
        listener.local_addr().expect("proxy address should exist")
    );
    drop(listener);

    let output = faus()
        .env("HTTP_PROXY", &proxy_url)
        .env("HTTPS_PROXY", &proxy_url)
        .env("ALL_PROXY", &proxy_url)
        .env("http_proxy", &proxy_url)
        .env("https_proxy", &proxy_url)
        .env("all_proxy", &proxy_url)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .args(["--base-url", &server.base_url, "--json", "library", "list"])
        .output()
        .expect("faus library list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["libraries"][0]["id"], "demo");
    assert_eq!(server.requests(), vec!["/libraries"]);
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
    let web_port = free_port();
    let env_file = write_web_env(web_port);
    let expected_web_url = format!("http://127.0.0.1:{web_port}");

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .env("FAUS_TEST_WEB_EXIT_AFTER_READY", "1")
        .env("FAUNI_ENV_FILE", &env_file)
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
    assert_eq!(payload["debug"]["web_url"], expected_web_url);
    assert_eq!(payload["debug"]["health_status"], 200);
    assert_eq!(payload["debug"]["startup"], "connected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_create_sends_body_and_outputs_library() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "library",
            "create",
            "--display-name",
            "Demo Library",
            "--library-id",
            "demo-lib",
        ])
        .output()
        .expect("faus library create should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["library"]["id"], "demo-lib");
    assert_eq!(payload["data"]["library"]["display_name"], "Demo Library");
    assert_eq!(
        payload["debug"]["request_url"],
        format!("{}/libraries", server.base_url)
    );
    assert_eq!(payload["debug"]["http_status"], 201);

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/libraries");
    assert_eq!(
        records[0].body,
        Some(json!({
            "display_name": "Demo Library",
            "library_id": "demo-lib"
        }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_rename_sends_patch_body_and_outputs_library() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "library",
            "rename",
            "demo",
            "--display-name",
            "Renamed Demo",
        ])
        .output()
        .expect("faus library rename should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["library"]["id"], "demo");
    assert_eq!(payload["data"]["library"]["display_name"], "Renamed Demo");

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].method, "PATCH");
    assert_eq!(records[0].path, "/libraries/demo");
    assert_eq!(
        records[0].body,
        Some(json!({ "display_name": "Renamed Demo" }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_archive_and_restore_use_action_paths() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let archive = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "library",
            "archive",
            "demo",
        ])
        .output()
        .expect("faus library archive should run");
    assert_success(&archive);
    let archive_payload = stdout_json(&archive);
    assert_eq!(
        archive_payload["data"]["library"]["lifecycle_state"],
        "archived"
    );

    let restore = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "library",
            "restore",
            "demo",
        ])
        .output()
        .expect("faus library restore should run");
    assert_success(&restore);
    let restore_payload = stdout_json(&restore);
    assert_eq!(
        restore_payload["data"]["library"]["lifecycle_state"],
        "active"
    );

    let records = server.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/libraries/demo/archive");
    assert_eq!(records[1].method, "POST");
    assert_eq!(records[1].path, "/libraries/demo/restore");
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
    let web_port = free_port();
    let env_file = write_web_env(web_port);
    let expected_web_url = format!("http://127.0.0.1:{web_port}");

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "fail")
        .env("FAUS_TEST_WEB_EXIT_AFTER_READY", "1")
        .env("FAUNI_ENV_FILE", &env_file)
        .args(["--base-url", &server.base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["opened"], false);
    assert_eq!(payload["data"]["web_url"], expected_web_url);
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
fn status_explicit_connection_failure_prints_human_hint() {
    let port = free_port();
    let base_url = format!("http://127.0.0.1:{port}");

    let output = faus()
        .args(["--base-url", &base_url, "status"])
        .output()
        .expect("faus status should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("[error] connection_failed"));
    assert!(stderr.contains("[hint]"));
    assert!(stderr.contains("--base-url"));
    assert!(stderr.contains("FAUS_BASE_URL"));
}

#[test]
fn web_explicit_unreachable_server_returns_connection_failed() {
    let port = free_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "connection_failed");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("--base-url"));
    assert_eq!(payload["error"]["details"]["base_url"], base_url);
    assert_eq!(payload["error"]["details"]["base_url_source"], "flag");
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{base_url}/health")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_explicit_empty_health_response_returns_invalid_response() {
    let server = StatusServer::start(RuntimeMode::EmptyHealth).await;

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .args(["--base-url", &server.base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(payload["error"]["details"]["base_url"], server.base_url);
    assert_eq!(payload["error"]["details"]["base_url_source"], "flag");
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/health", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
    assert_eq!(server.requests(), vec!["/health"]);
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
    assert!(payload["error"].get("hint").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "library", "list"])
        .output()
        .expect("faus library list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["error"]["code"], "runtime_unavailable");
    assert_eq!(payload["error"]["message"], "Qdrant is offline");
    assert_eq!(payload["error"]["details"]["component"], "qdrant");
    assert_eq!(payload["error"]["retryable"], true);
    assert!(payload["error"].get("hint").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_reports_occupied_web_port() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("web port should bind");
    let occupied_port = listener
        .local_addr()
        .expect("web port address should exist")
        .port();
    let env_file = write_web_env(occupied_port);

    let output = faus()
        .env("FAUS_TEST_BROWSER_OPEN", "ok")
        .env("FAUNI_ENV_FILE", &env_file)
        .args(["--base-url", &server.base_url, "--json", "web"])
        .output()
        .expect("faus web should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "web_port_unavailable");
    assert_eq!(server.requests(), vec!["/health"]);
    drop(listener);
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
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/runtime/status", server.base_url)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_rejects_missing_list_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "library", "list"])
        .output()
        .expect("faus library list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/libraries", server.base_url)
    );
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
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/runtime/status", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
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

fn read_child_json_line(child: &mut Child) -> Value {
    let stdout = child.stdout.take().expect("child stdout should be piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("child should write a JSON line");
    serde_json::from_str(&line).unwrap_or_else(|error| {
        let _ = child.kill();
        panic!("child stdout line should be JSON: {error}; line: {line:?}");
    })
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("free port listener should bind")
        .local_addr()
        .expect("free port listener should have address")
        .port()
}

fn write_web_env(web_port: u16) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "faus-web-test-{}-{web_port}.env",
        std::process::id()
    ));
    fs::write(&path, format!("UI_HOST=127.0.0.1\nUI_PORT={web_port}\n"))
        .expect("test Web env file should be written");
    path
}

#[derive(Clone, Copy)]
enum RuntimeMode {
    Ok,
    ErrorEnvelope,
    MissingData,
    NotJson,
    EmptyHealth,
}

struct StatusServer {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    records: Arc<Mutex<Vec<RecordedRequest>>>,
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
        let records = Arc::new(Mutex::new(Vec::new()));
        let state = StatusServerState {
            requests: requests.clone(),
            records: records.clone(),
            runtime_mode,
        };
        let app = Router::new()
            .route("/health", get(health))
            .route("/", get(web_root))
            .route("/runtime/status", get(runtime_status))
            .route("/libraries", get(list_libraries).post(create_library))
            .route(
                "/libraries/{library_id}",
                get(show_library).patch(rename_library),
            )
            .route("/libraries/{library_id}/archive", post(archive_library))
            .route("/libraries/{library_id}/restore", post(restore_library))
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
            records,
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

    fn records(&self) -> Vec<RecordedRequest> {
        self.records
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
    records: Arc<Mutex<Vec<RecordedRequest>>>,
    runtime_mode: RuntimeMode,
}

impl StatusServerState {
    fn record(&self, path: &str) {
        self.requests
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(path.to_string());
    }

    fn record_http(&self, method: &str, path: &str, body: Option<Value>) {
        self.record(path);
        self.records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(RecordedRequest {
                method: method.to_string(),
                path: path.to_string(),
                body,
            });
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RecordedRequest {
    method: String,
    path: String,
    body: Option<Value>,
}

async fn health(State(state): State<StatusServerState>) -> Response {
    state.record("/health");
    match state.runtime_mode {
        RuntimeMode::EmptyHealth => (StatusCode::OK, "").into_response(),
        _ => Json(json!({
            "service": "fauni-search",
            "status": "ok",
            "env": "test",
            "libraries": 1,
            "jobs": 0,
        }))
        .into_response(),
    }
}

async fn web_root(State(state): State<StatusServerState>) -> Response {
    state.record("/");
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><html><body><div id=\"app\"></div></body></html>",
    )
        .into_response()
}

async fn runtime_status(State(state): State<StatusServerState>) -> Response {
    state.record("/runtime/status");
    match state.runtime_mode {
        RuntimeMode::Ok | RuntimeMode::EmptyHealth => Json(json!({
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

async fn list_libraries(State(state): State<StatusServerState>) -> Response {
    state.record_http("GET", "/libraries", None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": {
                "libraries": [
                    library_snapshot("demo", "Demo Library", "active")
                ]
            }
        }))
        .into_response(),
    }
}

async fn create_library(
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    state.record_http("POST", "/libraries", Some(body.clone()));
    let library_id = body
        .get("library_id")
        .and_then(Value::as_str)
        .unwrap_or("demo");
    let display_name = body
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("Demo Library");
    (
        StatusCode::CREATED,
        Json(json!({
            "data": library_snapshot(library_id, display_name, "active")
        })),
    )
        .into_response()
}

async fn show_library(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}");
    state.record_http("GET", &path, None);
    Json(json!({
        "data": library_snapshot(&library_id, "Demo Library", "active")
    }))
    .into_response()
}

async fn rename_library(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    let path = format!("/libraries/{library_id}");
    state.record_http("PATCH", &path, Some(body.clone()));
    let display_name = body
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("Demo Library");
    Json(json!({
        "data": library_snapshot(&library_id, display_name, "active")
    }))
    .into_response()
}

async fn archive_library(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}/archive");
    state.record_http("POST", &path, None);
    Json(json!({
        "data": library_snapshot(&library_id, "Demo Library", "archived")
    }))
    .into_response()
}

async fn restore_library(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}/restore");
    state.record_http("POST", &path, None);
    Json(json!({
        "data": library_snapshot(&library_id, "Demo Library", "active")
    }))
    .into_response()
}

fn library_snapshot(library_id: &str, display_name: &str, lifecycle_state: &str) -> Value {
    json!({
        "id": library_id,
        "display_name": display_name,
        "lifecycle_state": lifecycle_state,
        "counts": {
            "accepted_items": 3,
            "pending_jobs": 1,
        },
        "latest_job_id": null,
    })
}

fn server_error_envelope() -> Response {
    (
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
        .into_response()
}
