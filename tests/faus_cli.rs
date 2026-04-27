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
    time::{SystemTime, UNIX_EPOCH},
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
    assert!(stdout.contains("faus sources roots list"));
    assert!(stdout.contains("faus import --library-id demo"));
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
    assert!(stdout.contains("--model"));
    assert!(stdout.contains("--dev"));
    assert!(!stdout.to_lowercase().contains("vite"));
}

#[test]
fn serve_model_help_exposes_modeld_flags() {
    let output = faus()
        .args(["serve", "model", "--help"])
        .output()
        .expect("faus serve model help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--host"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--model"));
    assert!(stdout.contains("--dev"));
    assert!(stdout.contains("modeld"));
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

#[test]
fn jobs_help_exposes_workflows() {
    let output = faus()
        .args(["jobs", "--help"])
        .output()
        .expect("faus jobs help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for command in ["list", "show", "cancel", "resume", "retry"] {
        assert!(stdout.contains(command), "missing {command} in help");
    }
    assert!(stdout.contains("Manage runtime jobs through the App API"));
    assert!(stdout.contains("faus jobs list --library-id"));
}

#[test]
fn jobs_list_help_describes_library_filter() {
    let output = faus()
        .args(["jobs", "list", "--help"])
        .output()
        .expect("faus jobs list help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--library-id"));
    assert!(stdout.contains("Filter jobs by library id"));
}

#[test]
fn import_help_describes_inputs() {
    let output = faus()
        .args(["import", "--help"])
        .output()
        .expect("faus import help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(stdout.contains("--library-id"));
    assert!(stdout.contains("Target library id"));
    assert!(stdout.contains("<PATH>"));
    assert!(stdout.contains("does not start local processes"));
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
async fn jobs_list_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .args(["--json", "jobs", "list"])
        .output()
        .expect("faus jobs list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(payload["data"]["jobs"][0]["job_id"], "job_1");
    assert_eq!(server.requests(), vec!["/jobs"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("import-env-base-url");
    let expected_path = cwd.join("report.pdf").to_string_lossy().to_string();

    let output = faus()
        .current_dir(&cwd)
        .env("FAUS_BASE_URL", &server.base_url)
        .args(["--json", "import", "--library-id", "demo", "report.pdf"])
        .output()
        .expect("faus import should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(
        payload["data"]["import"]["accepted"][0]["original_path"],
        expected_path
    );
    assert_eq!(payload["data"]["import"]["job_handle"], "job_1");
    assert_eq!(server.requests(), vec!["/libraries/demo/imports"]);
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
async fn jobs_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args([
            "--base-url",
            &base_url_with_slash,
            "--json",
            "jobs",
            "show",
            "job_1",
        ])
        .output()
        .expect("faus jobs show should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(payload["data"]["job"]["job_id"], "job_1");
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/jobs/job_1"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args([
            "--base-url",
            &base_url_with_slash,
            "--json",
            "import",
            "--library-id",
            "demo",
            "/tmp/report.pdf",
        ])
        .output()
        .expect("faus import should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(payload["data"]["import"]["job"]["job_id"], "job_1");
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/libraries/demo/imports"]);
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
async fn jobs_list_filters_by_library_id() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "jobs",
            "list",
            "--library-id",
            "demo",
        ])
        .output()
        .expect("faus jobs list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["jobs"][0]["library_id"], "demo");
    assert_eq!(server.requests(), vec!["/jobs?library_id=demo"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jobs_actions_use_action_paths() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let cancel = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "jobs",
            "cancel",
            "job_1",
        ])
        .output()
        .expect("faus jobs cancel should run");
    assert_success(&cancel);
    let cancel_payload = stdout_json(&cancel);
    assert_eq!(cancel_payload["data"]["job"]["phase"], "cancel_requested");

    let resume = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "jobs",
            "resume",
            "job_1",
        ])
        .output()
        .expect("faus jobs resume should run");
    assert_success(&resume);
    let resume_payload = stdout_json(&resume);
    assert_eq!(resume_payload["data"]["job"]["status"], "queued");

    let retry = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "jobs",
            "retry",
            "job_1",
        ])
        .output()
        .expect("faus jobs retry should run");
    assert_success(&retry);
    let retry_payload = stdout_json(&retry);
    assert_eq!(retry_payload["data"]["job"]["job_id"], "job_2");
    assert_eq!(retry_payload["data"]["job"]["retried_from_job_id"], "job_1");
    assert_eq!(
        retry_payload["debug"]["request_url"],
        format!("{}/jobs/job_1/retry", server.base_url)
    );

    let records = server.records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/jobs/job_1/cancel");
    assert_eq!(records[1].method, "POST");
    assert_eq!(records[1].path, "/jobs/job_1/resume");
    assert_eq!(records[2].method, "POST");
    assert_eq!(records[2].path, "/jobs/job_1/retry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_sends_paths_in_order_and_outputs_import() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("import-path-order");
    let relative = cwd.join("relative.pdf").to_string_lossy().to_string();
    let absolute = "/tmp/faus-absolute.pdf";
    let rejected = cwd.join("reject.txt").to_string_lossy().to_string();

    let output = faus()
        .current_dir(&cwd)
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "import",
            "--library-id",
            "demo",
            "relative.pdf",
            absolute,
            "reject.txt",
        ])
        .output()
        .expect("faus import should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(
        payload["data"]["import"]["accepted"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        payload["data"]["import"]["rejected"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        payload["debug"]["request_url"],
        format!("{}/libraries/demo/imports", server.base_url)
    );
    assert_eq!(payload["debug"]["http_status"], 200);

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/libraries/demo/imports");
    assert_eq!(
        records[0].body,
        Some(json!({
            "paths": [
                relative,
                absolute,
                rejected,
            ]
        }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_human_output_summarizes_rejections_and_job() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("import-human");
    let rejected = cwd.join("reject.txt").to_string_lossy().to_string();

    let output = faus()
        .current_dir(&cwd)
        .args([
            "--base-url",
            &server.base_url,
            "import",
            "--library-id",
            "demo",
            "ok.pdf",
            "reject.txt",
        ])
        .output()
        .expect("faus import should run");

    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("accepted=1"));
    assert!(stdout.contains("rejected=1"));
    assert!(stdout.contains("job=job_1"));
    assert!(stdout.contains("status=queued"));
    assert!(stdout.contains(&rejected));
    assert!(stdout.contains("unsupported_type"));
}

#[test]
fn sources_help_exposes_workflows() {
    let output = faus()
        .args(["sources", "--help"])
        .output()
        .expect("faus sources help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for command in ["roots", "list", "refresh", "rescan"] {
        assert!(stdout.contains(command), "missing {command} in help");
    }
    assert!(stdout.contains("source inventory"));
    assert!(stdout.contains("does not start local processes"));
}

#[test]
fn sources_roots_help_exposes_lifecycle_commands() {
    let output = faus()
        .args(["sources", "roots", "--help"])
        .output()
        .expect("faus sources roots help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for command in ["list", "create", "show", "update", "delete"] {
        assert!(stdout.contains(command), "missing {command} in help");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_roots_list_uses_faus_base_url_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .args(["--json", "sources", "roots", "list", "--library-id", "demo"])
        .output()
        .expect("faus sources roots list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(
        payload["data"]["source_roots"][0]["source_root_id"],
        "root_1"
    );
    assert_eq!(server.requests(), vec!["/libraries/demo/source-roots"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_roots_create_sends_body_and_outputs_source_root() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("sources-root-create");
    let expected_path = cwd.join("docs").to_string_lossy().to_string();

    let output = faus()
        .current_dir(&cwd)
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "sources",
            "roots",
            "create",
            "--library-id",
            "demo",
            "--root-path",
            "docs",
            "--disabled",
            "--include-glob",
            "**/*.pdf",
            "--exclude-glob",
            "**/drafts/**",
            "--include-extension",
            "pdf",
        ])
        .output()
        .expect("faus sources roots create should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["source_root"]["source_root_id"], "root_1");
    assert_eq!(payload["data"]["source_root"]["root_path"], expected_path);
    assert_eq!(
        payload["debug"]["request_url"],
        format!("{}/libraries/demo/source-roots", server.base_url)
    );
    assert_eq!(payload["debug"]["http_status"], 201);

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/libraries/demo/source-roots");
    assert_eq!(
        records[0].body,
        Some(json!({
            "root_path": expected_path,
            "enabled": false,
            "rules": {
                "include_globs": ["**/*.pdf"],
                "exclude_globs": ["**/drafts/**"],
                "include_extensions": ["pdf"]
            }
        }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_roots_show_update_and_delete_use_expected_paths() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("sources-root-update");
    let updated_path = cwd.join("library").to_string_lossy().to_string();

    let show = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "show",
            "--library-id",
            "demo",
            "root_1",
        ])
        .output()
        .expect("faus sources roots show should run");
    assert_success(&show);
    let show_payload = stdout_json(&show);
    assert_eq!(
        show_payload["data"]["source_root"]["source_root_id"],
        "root_1"
    );

    let update = faus()
        .current_dir(&cwd)
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "update",
            "--library-id",
            "demo",
            "root_1",
            "--root-path",
            "library",
            "--disable",
            "--include-glob",
            "**/*.md",
        ])
        .output()
        .expect("faus sources roots update should run");
    assert_success(&update);
    let update_payload = stdout_json(&update);
    assert_eq!(update_payload["data"]["source_root"]["enabled"], false);

    let delete = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "delete",
            "--library-id",
            "demo",
            "root_1",
        ])
        .output()
        .expect("faus sources roots delete should run");
    assert_success(&delete);

    let records = server.records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].method, "GET");
    assert_eq!(records[0].path, "/libraries/demo/source-roots/root_1");
    assert_eq!(records[1].method, "PATCH");
    assert_eq!(records[1].path, "/libraries/demo/source-roots/root_1");
    assert_eq!(
        records[1].body,
        Some(json!({
            "root_path": updated_path,
            "enabled": false,
            "rules": {
                "include_globs": ["**/*.md"],
                "exclude_globs": [],
                "include_extensions": []
            }
        }))
    );
    assert_eq!(records[2].method, "DELETE");
    assert_eq!(records[2].path, "/libraries/demo/source-roots/root_1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_list_filters_are_url_encoded() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "list",
            "--library-id",
            "demo",
            "--source-root-id",
            "root 1",
            "--source-type",
            "video/mp4",
            "--status",
            "needs attention",
        ])
        .output()
        .expect("faus sources list should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["sources"][0]["source_id"], "src_1");
    assert_eq!(
        server.requests(),
        vec!["/libraries/demo/sources?source_root_id=root+1&source_type=video%2Fmp4&status=needs+attention"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_refresh_and_rescan_use_library_and_root_paths() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let refresh_library = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "refresh",
            "--library-id",
            "demo",
        ])
        .output()
        .expect("faus sources refresh should run");
    assert_success(&refresh_library);
    let refresh_payload = stdout_json(&refresh_library);
    assert_eq!(refresh_payload["data"]["action"]["job_handle"], "job_1");

    let rescan_root = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "rescan",
            "--library-id",
            "demo",
            "--source-root-id",
            "root_1",
        ])
        .output()
        .expect("faus sources rescan should run");
    assert_success(&rescan_root);

    let records = server.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/libraries/demo/refresh");
    assert_eq!(records[1].method, "POST");
    assert_eq!(
        records[1].path,
        "/libraries/demo/source-roots/root_1/rescan"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args([
            "--base-url",
            &base_url_with_slash,
            "--json",
            "sources",
            "roots",
            "show",
            "--library-id",
            "demo",
            "root_1",
        ])
        .output()
        .expect("faus sources roots show should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(
        flag_server.requests(),
        vec!["/libraries/demo/source-roots/root_1"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "list",
            "--library-id",
            "demo",
        ])
        .output()
        .expect("faus sources roots list should run");

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
async fn sources_rejects_missing_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "list",
            "--library-id",
            "demo",
        ])
        .output()
        .expect("faus sources roots list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/libraries/demo/source-roots", server.base_url)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sources_rejects_non_json_response() {
    let server = StatusServer::start(RuntimeMode::NotJson).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "sources",
            "roots",
            "list",
            "--library-id",
            "demo",
        ])
        .output()
        .expect("faus sources roots list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/libraries/demo/source-roots", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
}

#[test]
fn search_help_describes_flag_based_inputs() {
    let output = faus()
        .args(["search", "--help"])
        .output()
        .expect("faus search help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for flag in [
        "--library-id",
        "--all-libraries",
        "--text",
        "--image",
        "--video",
        "--document",
        "--top-k",
        "--cursor",
        "--target-content-type",
    ] {
        assert!(stdout.contains(flag), "missing {flag} in help");
    }
    assert!(stdout.contains("combined queries"));
    assert!(stdout.contains("faus search --library-id demo --text"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_text_library_scope_sends_body_and_outputs_json() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .env("FAUS_BASE_URL", &server.base_url)
        .args([
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "terminal screen",
        ])
        .output()
        .expect("faus search should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["base_url"], server.base_url);
    assert_eq!(
        payload["data"]["search"]["results"][0]["library_id"],
        "demo"
    );
    assert_eq!(server.requests(), vec!["/search/text"]);

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].method, "POST");
    assert_eq!(records[0].path, "/search/text");
    assert_eq!(
        records[0].body,
        Some(json!({
            "text": "terminal screen",
            "search_scope": {
                "kind": "library",
                "library_id": "demo"
            }
        }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_text_all_libraries_sends_common_flags_and_debug() {
    let server = StatusServer::start(RuntimeMode::Ok).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--debug",
            "--json",
            "search",
            "--all-libraries",
            "--text",
            "quarterly report",
            "--top-k",
            "7",
            "--cursor",
            "cursor_1",
            "--target-content-type",
            "image",
            "--target-content-type",
            "video",
        ])
        .output()
        .expect("faus search should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["search"]["debug"]["request_debug"], true);
    assert_eq!(
        payload["debug"]["search_request_url"],
        format!("{}/search/text", server.base_url)
    );
    assert_eq!(payload["debug"]["search_http_status"], 200);

    let records = server.records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].body,
        Some(json!({
            "text": "quarterly report",
            "search_scope": {
                "kind": "all_libraries"
            },
            "top_k": 7,
            "cursor": "cursor_1",
            "target_content_types": ["image", "video"],
            "debug": true
        }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_file_inputs_upload_then_search() {
    let server = StatusServer::start(RuntimeMode::Ok).await;
    let cwd = temp_test_dir("search-files");
    let image = cwd.join("query.png");
    let video = cwd.join("clip.mp4");
    let document = cwd.join("report.pdf");
    fs::write(&image, b"image").expect("image fixture should be written");
    fs::write(&video, b"video").expect("video fixture should be written");
    fs::write(&document, b"%PDF").expect("document fixture should be written");

    let image_output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--image",
            image.to_str().unwrap(),
        ])
        .output()
        .expect("faus image search should run");
    assert_success(&image_output);
    let image_payload = stdout_json(&image_output);
    assert_eq!(
        image_payload["data"]["query_asset"]["temp_asset_id"],
        "asset_image_1"
    );

    let video_output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--video",
            video.to_str().unwrap(),
            "--video-start-ms",
            "42000",
            "--video-end-ms",
            "50000",
        ])
        .output()
        .expect("faus video search should run");
    assert_success(&video_output);

    let document_output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--document",
            document.to_str().unwrap(),
            "--document-start-page",
            "1",
            "--document-end-page",
            "3",
        ])
        .output()
        .expect("faus document search should run");
    assert_success(&document_output);

    assert_eq!(
        server.requests(),
        vec![
            "/libraries/demo/query-assets/images",
            "/search/image",
            "/libraries/demo/query-assets/videos",
            "/search/video",
            "/libraries/demo/query-assets/documents",
            "/search/document",
        ]
    );
    let records = server.records();
    assert_eq!(
        records[1].body.as_ref().unwrap()["image_input"]["kind"],
        "temp_asset"
    );
    assert_eq!(
        records[1].body.as_ref().unwrap()["image_input"]["temp_asset_id"],
        "asset_image_1"
    );
    assert_eq!(
        records[3].body.as_ref().unwrap()["video_input"]["locator"],
        json!({ "start_ms": 42000, "end_ms": 50000 })
    );
    assert_eq!(
        records[5].body.as_ref().unwrap()["document_input"]["locator"],
        json!({ "start_page": 1, "end_page": 3 })
    );
}

#[test]
fn search_multiple_inputs_returns_not_supported_without_http() {
    let output = faus()
        .args([
            "--base-url",
            "http://127.0.0.1:1",
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "terminal",
            "--image",
            "/tmp/query.png",
        ])
        .output()
        .expect("faus search should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "not_supported");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("combined search"));
}

#[test]
fn search_file_all_libraries_returns_not_supported_without_http() {
    let output = faus()
        .args([
            "--base-url",
            "http://127.0.0.1:1",
            "--json",
            "search",
            "--all-libraries",
            "--image",
            "/tmp/query.png",
        ])
        .output()
        .expect("faus search should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "not_supported");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("--library-id"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_base_url_flag_overrides_env_and_trims_trailing_slash() {
    let env_server = StatusServer::start(RuntimeMode::Ok).await;
    let flag_server = StatusServer::start(RuntimeMode::Ok).await;
    let base_url_with_slash = format!("{}/", flag_server.base_url);

    let output = faus()
        .env("FAUS_BASE_URL", &env_server.base_url)
        .args([
            "--base-url",
            &base_url_with_slash,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "query",
        ])
        .output()
        .expect("faus search should run");

    assert_success(&output);
    let payload = stdout_json(&output);
    assert_eq!(payload["data"]["base_url"], flag_server.base_url);
    assert_eq!(env_server.requests(), Vec::<String>::new());
    assert_eq!(flag_server.requests(), vec!["/search/text"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "query",
        ])
        .output()
        .expect("faus search should run");

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
async fn search_rejects_missing_success_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "query",
        ])
        .output()
        .expect("faus search should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/search/text", server.base_url)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_rejects_non_json_response() {
    let server = StatusServer::start(RuntimeMode::NotJson).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "search",
            "--library-id",
            "demo",
            "--text",
            "query",
        ])
        .output()
        .expect("faus search should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/search/text", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
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
async fn jobs_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "jobs", "list"])
        .output()
        .expect("faus jobs list should run");

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
async fn import_preserves_server_error_envelope() {
    let server = StatusServer::start(RuntimeMode::ErrorEnvelope).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "import",
            "--library-id",
            "demo",
            "/tmp/report.pdf",
        ])
        .output()
        .expect("faus import should run");

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
async fn jobs_rejects_missing_list_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "jobs", "list"])
        .output()
        .expect("faus jobs list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/jobs", server.base_url)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_rejects_missing_data_envelope() {
    let server = StatusServer::start(RuntimeMode::MissingData).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "import",
            "--library-id",
            "demo",
            "/tmp/report.pdf",
        ])
        .output()
        .expect("faus import should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("App API contract"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/libraries/demo/imports", server.base_url)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jobs_rejects_non_json_response() {
    let server = StatusServer::start(RuntimeMode::NotJson).await;

    let output = faus()
        .args(["--base-url", &server.base_url, "--json", "jobs", "list"])
        .output()
        .expect("faus jobs list should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/jobs", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn import_rejects_non_json_response() {
    let server = StatusServer::start(RuntimeMode::NotJson).await;

    let output = faus()
        .args([
            "--base-url",
            &server.base_url,
            "--json",
            "import",
            "--library-id",
            "demo",
            "/tmp/report.pdf",
        ])
        .output()
        .expect("faus import should run");

    assert!(!output.status.success());
    let payload = stdout_json(&output);
    assert_eq!(payload["error"]["code"], "invalid_response");
    assert!(payload["error"]["hint"]
        .as_str()
        .expect("hint should be present")
        .contains("FauniSearch App API server"));
    assert_eq!(
        payload["error"]["details"]["request_url"],
        format!("{}/libraries/demo/imports", server.base_url)
    );
    assert_eq!(payload["error"]["details"]["http_status"], 200);
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

fn temp_test_dir(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("faus-cli-{name}-{}-{now}", std::process::id()));
    fs::create_dir_all(&path).expect("test temp dir should be created");
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
            .route("/jobs", get(list_jobs))
            .route("/jobs/{job_id}", get(show_job))
            .route("/jobs/{job_id}/cancel", post(cancel_job))
            .route("/jobs/{job_id}/resume", post(resume_job))
            .route("/jobs/{job_id}/retry", post(retry_job))
            .route("/libraries", get(list_libraries).post(create_library))
            .route(
                "/libraries/{library_id}",
                get(show_library).patch(rename_library),
            )
            .route("/libraries/{library_id}/archive", post(archive_library))
            .route("/libraries/{library_id}/restore", post(restore_library))
            .route("/libraries/{library_id}/imports", post(import_paths))
            .route(
                "/libraries/{library_id}/source-roots",
                get(list_source_roots).post(create_source_root),
            )
            .route(
                "/libraries/{library_id}/source-roots/{source_root_id}",
                get(show_source_root)
                    .patch(update_source_root)
                    .delete(delete_source_root),
            )
            .route("/libraries/{library_id}/sources", get(list_sources))
            .route(
                "/libraries/{library_id}/refresh",
                post(refresh_library_sources),
            )
            .route(
                "/libraries/{library_id}/rescan",
                post(rescan_library_sources),
            )
            .route(
                "/libraries/{library_id}/source-roots/{source_root_id}/refresh",
                post(refresh_source_root),
            )
            .route(
                "/libraries/{library_id}/source-roots/{source_root_id}/rescan",
                post(rescan_source_root),
            )
            .route(
                "/libraries/{library_id}/query-assets/images",
                post(upload_query_image),
            )
            .route(
                "/libraries/{library_id}/query-assets/videos",
                post(upload_query_video),
            )
            .route(
                "/libraries/{library_id}/query-assets/documents",
                post(upload_query_document),
            )
            .route("/search/text", post(search_text))
            .route("/search/image", post(search_image))
            .route("/search/video", post(search_video))
            .route("/search/document", post(search_document))
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

async fn list_jobs(State(state): State<StatusServerState>, uri: axum::http::Uri) -> Response {
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/jobs")
        .to_string();
    state.record_http("GET", &path, None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": {
                "jobs": [
                    job_snapshot("job_1", "demo", "import", "running", "indexing", None)
                ]
            }
        }))
        .into_response(),
    }
}

async fn show_job(Path(job_id): Path<String>, State(state): State<StatusServerState>) -> Response {
    let path = format!("/jobs/{job_id}");
    state.record_http("GET", &path, None);
    Json(json!({
        "data": job_snapshot(&job_id, "demo", "import", "running", "indexing", None)
    }))
    .into_response()
}

async fn cancel_job(
    Path(job_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/jobs/{job_id}/cancel");
    state.record_http("POST", &path, None);
    Json(json!({
        "data": job_snapshot(&job_id, "demo", "import", "running", "cancel_requested", None)
    }))
    .into_response()
}

async fn resume_job(
    Path(job_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/jobs/{job_id}/resume");
    state.record_http("POST", &path, None);
    Json(json!({
        "data": job_snapshot(&job_id, "demo", "import", "queued", "queued", None)
    }))
    .into_response()
}

async fn retry_job(Path(job_id): Path<String>, State(state): State<StatusServerState>) -> Response {
    let path = format!("/jobs/{job_id}/retry");
    state.record_http("POST", &path, None);
    Json(json!({
        "data": job_snapshot("job_2", "demo", "import", "queued", "queued", Some(&job_id))
    }))
    .into_response()
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

async fn import_paths(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    let path = format!("/libraries/{library_id}/imports");
    state.record_http("POST", &path, Some(body.clone()));
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": import_snapshot(&library_id, &body)
        }))
        .into_response(),
    }
}

async fn list_source_roots(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}/source-roots");
    state.record_http("GET", &path, None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": {
                "source_roots": [
                    source_root_snapshot("root_1", "/tmp/demo", true)
                ]
            }
        }))
        .into_response(),
    }
}

async fn create_source_root(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    let path = format!("/libraries/{library_id}/source-roots");
    state.record_http("POST", &path, Some(body.clone()));
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => {
            let root_path = body
                .get("root_path")
                .and_then(Value::as_str)
                .unwrap_or("/tmp/demo");
            let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            (
                StatusCode::CREATED,
                Json(json!({
                    "data": source_root_snapshot("root_1", root_path, enabled)
                })),
            )
                .into_response()
        }
    }
}

async fn show_source_root(
    Path((library_id, source_root_id)): Path<(String, String)>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}/source-roots/{source_root_id}");
    state.record_http("GET", &path, None);
    Json(json!({
        "data": {
            "source_root": source_root_snapshot(&source_root_id, "/tmp/demo", true)
        }
    }))
    .into_response()
}

async fn update_source_root(
    Path((library_id, source_root_id)): Path<(String, String)>,
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    let path = format!("/libraries/{library_id}/source-roots/{source_root_id}");
    state.record_http("PATCH", &path, Some(body.clone()));
    let root_path = body
        .get("root_path")
        .and_then(Value::as_str)
        .unwrap_or("/tmp/demo");
    let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    Json(json!({
        "data": source_root_snapshot(&source_root_id, root_path, enabled)
    }))
    .into_response()
}

async fn delete_source_root(
    Path((library_id, source_root_id)): Path<(String, String)>,
    State(state): State<StatusServerState>,
) -> Response {
    let path = format!("/libraries/{library_id}/source-roots/{source_root_id}");
    state.record_http("DELETE", &path, None);
    Json(json!({
        "data": source_root_snapshot(&source_root_id, "/tmp/demo", false)
    }))
    .into_response()
}

async fn list_sources(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
    uri: axum::http::Uri,
) -> Response {
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/sources")
        .to_string();
    state.record_http("GET", &path, None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": {
                "sources": [
                    source_snapshot(&library_id, "src_1", "root_1")
                ]
            }
        }))
        .into_response(),
    }
}

async fn refresh_library_sources(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    source_action_response(
        state,
        format!("/libraries/{library_id}/refresh"),
        None,
        "refresh",
    )
}

async fn rescan_library_sources(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    source_action_response(
        state,
        format!("/libraries/{library_id}/rescan"),
        None,
        "rescan",
    )
}

async fn refresh_source_root(
    Path((library_id, source_root_id)): Path<(String, String)>,
    State(state): State<StatusServerState>,
) -> Response {
    source_action_response(
        state,
        format!("/libraries/{library_id}/source-roots/{source_root_id}/refresh"),
        Some(source_root_id),
        "refresh",
    )
}

async fn rescan_source_root(
    Path((library_id, source_root_id)): Path<(String, String)>,
    State(state): State<StatusServerState>,
) -> Response {
    source_action_response(
        state,
        format!("/libraries/{library_id}/source-roots/{source_root_id}/rescan"),
        Some(source_root_id),
        "rescan",
    )
}

fn source_action_response(
    state: StatusServerState,
    path: String,
    source_root_id: Option<String>,
    action: &str,
) -> Response {
    state.record_http("POST", &path, None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "meta": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": source_action_snapshot(source_root_id.as_deref().unwrap_or("root_1"), action)
        }))
        .into_response(),
    }
}

async fn upload_query_image(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    upload_query_asset(state, &library_id, "images", "image", "image/png")
}

async fn upload_query_video(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    upload_query_asset(state, &library_id, "videos", "video", "video/mp4")
}

async fn upload_query_document(
    Path(library_id): Path<String>,
    State(state): State<StatusServerState>,
) -> Response {
    upload_query_asset(
        state,
        &library_id,
        "documents",
        "document",
        "application/pdf",
    )
}

fn upload_query_asset(
    state: StatusServerState,
    library_id: &str,
    route_segment: &str,
    source_type: &str,
    content_type: &str,
) -> Response {
    let path = format!("/libraries/{library_id}/query-assets/{route_segment}");
    state.record_http("POST", &path, None);
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "data": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => (
            StatusCode::CREATED,
            Json(json!({
                "data": {
                    "temp_asset_id": format!("asset_{source_type}_1"),
                    "preview": {
                        "url": format!("/libraries/{library_id}/query-assets/{route_segment}/asset_{source_type}_1/preview")
                    },
                    "source_type": source_type,
                    "content_type": content_type,
                    "original_filename": format!("query.{source_type}")
                }
            })),
        )
            .into_response(),
    }
}

async fn search_text(State(state): State<StatusServerState>, Json(body): Json<Value>) -> Response {
    search_response(state, "/search/text", "text", body)
}

async fn search_image(State(state): State<StatusServerState>, Json(body): Json<Value>) -> Response {
    search_response(state, "/search/image", "image", body)
}

async fn search_video(State(state): State<StatusServerState>, Json(body): Json<Value>) -> Response {
    search_response(state, "/search/video", "video", body)
}

async fn search_document(
    State(state): State<StatusServerState>,
    Json(body): Json<Value>,
) -> Response {
    search_response(state, "/search/document", "document", body)
}

fn search_response(
    state: StatusServerState,
    path: &str,
    search_kind: &str,
    body: Value,
) -> Response {
    state.record_http("POST", path, Some(body.clone()));
    match state.runtime_mode {
        RuntimeMode::ErrorEnvelope => server_error_envelope(),
        RuntimeMode::MissingData => Json(json!({ "data": {} })).into_response(),
        RuntimeMode::NotJson => (StatusCode::OK, "not-json").into_response(),
        _ => Json(json!({
            "data": search_snapshot(search_kind, &body)
        }))
        .into_response(),
    }
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

fn source_root_snapshot(source_root_id: &str, root_path: &str, enabled: bool) -> Value {
    json!({
        "source_root_id": source_root_id,
        "root_path": root_path,
        "enabled": enabled,
        "status": if enabled { "ready" } else { "disabled" },
        "watch_state": if enabled { "watching" } else { "disabled" },
        "coverage_summary": {
            "observed_file_count": 3,
            "matched_file_count": 2,
            "active_source_count": 1,
            "inactive_source_count": 1,
            "last_scan_at_ms": 123456
        },
        "rules": {
            "include_globs": ["**/*"],
            "exclude_globs": [],
            "include_extensions": ["pdf"]
        }
    })
}

fn source_snapshot(library_id: &str, source_id: &str, source_root_id: &str) -> Value {
    json!({
        "source_id": source_id,
        "source_path": format!("/tmp/{library_id}/report.pdf"),
        "source_type": "pdf",
        "kind": "document",
        "status": "ready",
        "relative_path": "report.pdf",
        "source_root_id": source_root_id,
        "source_root_path": format!("/tmp/{library_id}"),
        "source_root_label": source_root_id,
        "visual_unit_count": 2,
        "representative_visual_unit": {
            "visual_unit_id": "vu_1",
            "source_id": source_id,
            "kind": "document_page",
            "source_type": "pdf",
            "locator": { "page": 1 }
        },
        "representative_preview": {
            "url": format!("/libraries/{library_id}/visual-units/vu_1/preview")
        }
    })
}

fn source_action_snapshot(source_root_id: &str, action: &str) -> Value {
    json!({
        "accepted": [
            {
                "source_root_id": source_root_id,
                "root_path": "/tmp/demo",
                "action": action
            }
        ],
        "rejected": [],
        "job_handle": "job_1",
        "job": job_snapshot("job_1", "demo", "source_action", "queued", action, None)
    })
}

fn search_snapshot(search_kind: &str, body: &Value) -> Value {
    let library_id = body
        .get("search_scope")
        .and_then(|scope| scope.get("library_id"))
        .and_then(Value::as_str)
        .or_else(|| body.get("library_id").and_then(Value::as_str))
        .unwrap_or("demo");
    let mut data = json!({
        "results": [
            {
                "library_id": library_id,
                "visual_unit_id": "vu_1",
                "source_id": "src_1",
                "preview": {
                    "url": format!("/libraries/{library_id}/visual-units/vu_1/preview")
                },
                "source_path": format!("/tmp/{search_kind}-source"),
                "source_type": search_kind,
                "kind": match search_kind {
                    "video" => "video_segment",
                    "document" => "document_page",
                    "image" => "image",
                    _ => "text_chunk",
                },
                "locator": match search_kind {
                    "video" => json!({ "start_ms": 42000, "end_ms": 50000 }),
                    "document" => json!({ "page": 1 }),
                    _ => json!({ "path": format!("/tmp/{search_kind}-source") }),
                },
                "cursor": "cursor_1",
                "score": 8.25
            }
        ],
        "next_cursor": "cursor_2",
        "unsupported_content_types": []
    });
    if body.get("debug").and_then(Value::as_bool).unwrap_or(false) {
        data["debug"] = json!({ "request_debug": true });
    }
    data
}

fn import_snapshot(library_id: &str, body: &Value) -> Value {
    let paths = body
        .get("paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();

    for path in paths {
        let original_path = path.as_str().unwrap_or("unknown").to_string();
        if original_path.contains("reject") {
            rejected.push(json!({
                "original_path": original_path,
                "normalized_path": original_path,
                "reason_code": "unsupported_type",
                "message": "Only supported media files are accepted.",
            }));
        } else {
            accepted.push(json!({
                "original_path": original_path,
                "normalized_path": original_path,
                "reason_code": "accepted",
                "message": "Accepted as document input for the library.",
                "source_id": "src_1",
                "source_type": "document",
                "kind": "document_page",
                "visual_units": [],
            }));
        }
    }

    json!({
        "accepted": accepted,
        "rejected": rejected,
        "job_handle": "job_1",
        "job": job_snapshot("job_1", library_id, "import", "queued", "intake", None),
    })
}

fn job_snapshot(
    job_id: &str,
    library_id: &str,
    kind: &str,
    status: &str,
    phase: &str,
    retried_from_job_id: Option<&str>,
) -> Value {
    let mut job = json!({
        "job_id": job_id,
        "library_id": library_id,
        "kind": kind,
        "status": status,
        "phase": phase,
        "progress": {
            "completed": 1,
            "total": 3,
            "unit": "items"
        },
        "cancelable": status == "running" || status == "queued",
        "retryable": true,
        "current_attempt": {
            "attempt": 2,
            "status": status,
            "summary": "Indexing demo content"
        }
    });
    if let Some(retried_from_job_id) = retried_from_job_id {
        job["retried_from_job_id"] = json!(retried_from_job_id);
    }
    job
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
