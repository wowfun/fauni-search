use crate::{
    client::{app_client, fetch_json, resolve_base_url, ResolvedBaseUrl},
    error::{CliError, CliFailure},
    serve::{
        find_repo_root, load_default_env, required_env, run_serve_with_ready_hook, ReadyHook,
        ServeArgs, ServeOutput, ServeReady,
    },
};
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use reqwest::Client;
use serde_json::json;
use std::{
    env,
    error::Error,
    fs,
    future::Future,
    io,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tokio::time::{sleep, Duration, Instant};
use tower_http::services::ServeDir;

const DEFAULT_PROBE_RETRY_WINDOW: Duration = Duration::from_secs(2);
const DEFAULT_PROBE_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const WEB_PROXY_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run_web(
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let web_config =
        resolve_web_config().map_err(|error| CliFailure::client(error, json_output))?;
    ensure_web_assets(&web_config).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;

    match probe_initial_web(&client, &base).await {
        Ok(probe) => {
            run_local_web_server(&base, &web_config, probe, false, json_output, debug)
                .await
                .map_err(|error| CliFailure::client(error, json_output))?;
            return Ok(());
        }
        Err(error) if should_start_default_runtime(&base, &error) => {}
        Err(error) => return Err(CliFailure::client(error, json_output)),
    }

    let output = if json_output {
        ServeOutput::Stderr
    } else {
        ServeOutput::Stdout
    };
    let hook: ReadyHook = Box::new(move |ready| {
        Box::pin(async move {
            let base = ResolvedBaseUrl {
                base_url: ready.base_url.clone(),
                source: "default",
            };
            let client = app_client()?;
            let probe = probe_started_runtime(&client, &ready).await?;
            run_local_web_server(&base, &web_config, probe, true, json_output, debug).await?;
            Ok(())
        }) as Pin<Box<dyn Future<Output = crate::serve::CliResult<()>>>>
    });

    run_serve_with_ready_hook(ServeArgs::default_runtime(), debug, output, Some(hook))
        .await
        .map_err(|error| serve_error_to_failure(error, json_output))
}

async fn probe_initial_web(client: &Client, base: &ResolvedBaseUrl) -> Result<WebProbe, CliError> {
    if base.source == "default" {
        probe_web_with_retry(client, base).await
    } else {
        probe_web(client, base).await
    }
}

async fn probe_web_with_retry(
    client: &Client,
    base: &ResolvedBaseUrl,
) -> Result<WebProbe, CliError> {
    let started_at = Instant::now();
    loop {
        match probe_web(client, base).await {
            Ok(probe) => return Ok(probe),
            Err(error) => {
                if !is_startup_probe_error(&error)
                    || started_at.elapsed() >= DEFAULT_PROBE_RETRY_WINDOW
                {
                    return Err(error);
                }
                sleep(DEFAULT_PROBE_RETRY_INTERVAL).await;
            }
        }
    }
}

async fn probe_started_runtime(client: &Client, ready: &ServeReady) -> Result<WebProbe, CliError> {
    let base = ResolvedBaseUrl {
        base_url: ready.base_url.clone(),
        source: "default",
    };
    let mut probe = probe_web(client, &base).await?;
    probe.health_url = ready.health_url.clone();
    Ok(probe)
}

async fn run_local_web_server(
    base: &ResolvedBaseUrl,
    web_config: &WebConfig,
    probe: WebProbe,
    server_started: bool,
    json_output: bool,
    debug: bool,
) -> Result<(), CliError> {
    let listener = tokio::net::TcpListener::bind(&web_config.bind_addr)
        .await
        .map_err(|error| web_bind_error(web_config, error))?;
    let local_addr = listener.local_addr().map_err(|error| {
        CliError::new(
            "web_bind_failed",
            format!(
                "Could not inspect Web server bind address {}: {error}",
                web_config.bind_addr
            ),
        )
    })?;
    let web_url = format!("http://{local_addr}");
    let app = build_web_app(web_config.clone(), base.base_url.clone())?;
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
    });
    let opened = open_browser(&web_url);

    if json_output {
        let mut payload = json!({
            "status": "ok",
            "data": {
                "base_url": base.base_url,
                "web_url": web_url,
                "opened": opened,
                "server_started": server_started,
            },
        });
        if debug {
            payload["debug"] = json!({
                "base_url_source": base.source,
                "health_url": probe.health_url,
                "web_url": web_url,
                "health_status": probe.health_status,
                "startup": if server_started { "started_runtime" } else { "connected" },
            });
        }
        println!(
            "{}",
            serde_json::to_string(&payload).expect("web JSON should serialize")
        );
    } else {
        if server_started {
            println!("Started local runtime.");
        }
        println!("Web URL: {web_url}");
        if opened {
            println!("Opened browser.");
        } else {
            println!(
                "Could not open a browser automatically; open this URL manually: {}",
                web_url
            );
        }
        if debug {
            println!("Health URL: {}", probe.health_url);
            println!("Health HTTP status: {}", probe.health_status);
        }
    }

    if env::var("FAUS_TEST_WEB_EXIT_AFTER_READY").ok().as_deref() == Some("1") {
        server_task.abort();
        return Ok(());
    }

    server_task
        .await
        .map_err(|error| {
            CliError::new(
                "web_server_failed",
                format!("Web server task failed: {error}"),
            )
        })?
        .map_err(|error| {
            CliError::new("web_server_failed", format!("Web server failed: {error}"))
        })?;

    Ok(())
}

async fn probe_web(client: &Client, base: &ResolvedBaseUrl) -> Result<WebProbe, CliError> {
    let health_request = base.request("/health");
    let health = fetch_json(client, &health_request).await.and_then(|fetched| {
        if fetched.value.is_object() {
            Ok(fetched)
        } else {
            Err(CliError::new(
                "invalid_response",
                format!("{} did not return a JSON object", health_request.url),
            ))
            .map_err(|error| {
                error
                    .with_hint("The target may still be starting, the port may be occupied by another process, or the URL may not be a FauniSearch App API server.")
                    .with_details(health_request.details(Some(fetched.status)))
            })
        }
    })?;

    Ok(WebProbe {
        health_url: health_request.url,
        health_status: health.status,
    })
}

fn resolve_web_config() -> Result<WebConfig, CliError> {
    let repo_root = find_repo_root(&env::current_dir().map_err(|error| {
        CliError::new(
            "web_config_failed",
            format!("Could not inspect cwd: {error}"),
        )
    })?)
    .map_err(|error| CliError::new("web_config_failed", error.to_string()))?;
    load_default_env(&repo_root)
        .map_err(|error| CliError::new("web_config_failed", error.to_string()))?;
    let host = required_env("UI_HOST")
        .map_err(|error| CliError::new("web_config_failed", error.to_string()))?;
    let port = required_env("UI_PORT")
        .map_err(|error| CliError::new("web_config_failed", error.to_string()))?;
    let bind_addr: SocketAddr = format!("{host}:{port}").parse().map_err(|error| {
        CliError::new(
            "web_config_failed",
            format!("Invalid UI_HOST/UI_PORT bind address {host}:{port}: {error}"),
        )
    })?;
    Ok(WebConfig {
        bind_addr,
        index_path: repo_root.join("ui/dist/index.html"),
        assets_dir: repo_root.join("ui/dist/assets"),
    })
}

fn ensure_web_assets(config: &WebConfig) -> Result<(), CliError> {
    if config.index_path.is_file() {
        return Ok(());
    }
    Err(CliError::new(
        "web_assets_missing",
        format!(
            "Web assets are not built. Expected {}.",
            config.index_path.display()
        ),
    )
    .with_details(json!({ "index_path": config.index_path })))
}

fn build_web_app(config: WebConfig, app_base_url: String) -> Result<Router, CliError> {
    let state = WebServerState {
        config,
        app_base_url,
        client: app_client()?,
    };
    let assets_dir = state.config.assets_dir.clone();
    Ok(Router::new()
        .route("/", get(web_index))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .fallback(web_fallback)
        .with_state(Arc::new(state)))
}

async fn web_index(State(state): State<Arc<WebServerState>>) -> Response {
    web_index_response(&state.config.index_path)
}

async fn web_fallback(
    State(state): State<Arc<WebServerState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if is_api_path(uri.path()) {
        return proxy_app_request(state, method, uri, headers, body).await;
    }
    if method == Method::GET {
        return web_index_response(&state.config.index_path);
    }
    StatusCode::NOT_FOUND.into_response()
}

fn web_index_response(index_path: &FsPath) -> Response {
    match fs::read(index_path) {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            bytes,
        )
            .into_response(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Web assets are not built. Expected ui/dist/index.html.",
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Failed to read Web assets: {error}"),
        )
            .into_response(),
    }
}

async fn proxy_app_request(
    state: Arc<WebServerState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target_url = format!("{}{}", state.app_base_url, path_and_query);
    let mut request = state
        .client
        .request(method, &target_url)
        .timeout(WEB_PROXY_TIMEOUT)
        .body(body);
    for (name, value) in headers.iter() {
        if matches!(
            name,
            &header::HOST | &header::CONTENT_LENGTH | &header::CONNECTION
        ) {
            continue;
        }
        request = request.header(name, value);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("Failed to proxy App API request to {target_url}: {error}"),
            )
                .into_response();
        }
    };
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("Failed to read App API proxy response from {target_url}: {error}"),
            )
                .into_response();
        }
    };
    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(bytes)).unwrap_or_else(|error| {
        (
            StatusCode::BAD_GATEWAY,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Failed to build App API proxy response: {error}"),
        )
            .into_response()
    })
}

fn is_api_path(path: &str) -> bool {
    matches!(
        path,
        "/openapi.json"
            | "/health"
            | "/routes"
            | "/runtime"
            | "/settings"
            | "/libraries"
            | "/jobs"
            | "/search"
    ) || path.starts_with("/runtime/")
        || path.starts_with("/settings/")
        || path.starts_with("/libraries/")
        || path.starts_with("/jobs/")
        || path.starts_with("/search/")
}

fn web_bind_error(config: &WebConfig, error: io::Error) -> CliError {
    let code = if error.kind() == io::ErrorKind::AddrInUse {
        "web_port_unavailable"
    } else {
        "web_bind_failed"
    };
    CliError::new(
        code,
        format!(
            "Could not bind Web server at {}; stop the existing process or free UI_HOST/UI_PORT: {error}",
            config.bind_addr
        ),
    )
    .with_details(json!({ "bind_addr": config.bind_addr.to_string() }))
}

fn should_start_default_runtime(base: &ResolvedBaseUrl, error: &CliError) -> bool {
    base.source == "default" && is_startup_probe_error(error)
}

fn is_startup_probe_error(error: &CliError) -> bool {
    error.code == "connection_failed"
        || (error.code == "invalid_response"
            && error.message.contains("/health")
            && (error.message.contains("did not return JSON")
                || error.message.contains("did not return a JSON object")))
}

fn open_browser(url: &str) -> bool {
    match env::var("FAUS_TEST_BROWSER_OPEN").ok().as_deref() {
        Some("ok") => return true,
        Some("fail") => return false,
        _ => {}
    }
    webbrowser::open(url).is_ok()
}

fn serve_error_to_failure(error: Box<dyn Error>, json_output: bool) -> CliFailure {
    match error.downcast::<CliError>() {
        Ok(error) => CliFailure::client(*error, json_output),
        Err(error) => CliFailure::human(error),
    }
}

struct WebProbe {
    health_url: String,
    health_status: u16,
}

#[derive(Clone)]
struct WebConfig {
    bind_addr: SocketAddr,
    index_path: PathBuf,
    assets_dir: PathBuf,
}

struct WebServerState {
    config: WebConfig,
    app_base_url: String,
    client: Client,
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_connection_failure_starts_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "default",
        };
        let error = CliError::new("connection_failed", "offline");

        assert!(should_start_default_runtime(&base, &error));
    }

    #[test]
    fn explicit_connection_failure_does_not_start_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "flag",
        };
        let error = CliError::new("connection_failed", "offline");

        assert!(!should_start_default_runtime(&base, &error));
    }

    #[test]
    fn default_health_invalid_response_starts_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "default",
        };
        let error = CliError::new(
            "invalid_response",
            "http://127.0.0.1:53210/health did not return JSON: EOF while parsing a value",
        );

        assert!(should_start_default_runtime(&base, &error));
    }

    #[test]
    fn explicit_health_invalid_response_does_not_start_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "flag",
        };
        let error = CliError::new(
            "invalid_response",
            "http://127.0.0.1:53210/health did not return JSON: EOF while parsing a value",
        );

        assert!(!should_start_default_runtime(&base, &error));
    }

    #[test]
    fn root_invalid_response_does_not_start_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "default",
        };
        let error = CliError::new(
            "invalid_response",
            "http://127.0.0.1:53210 returned HTTP 500 for the Web entry",
        );

        assert!(!should_start_default_runtime(&base, &error));
    }

    #[test]
    fn missing_web_assets_returns_diagnostic_error() {
        let missing_root =
            env::temp_dir().join(format!("faus-missing-web-assets-{}", std::process::id()));
        let config = WebConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            index_path: missing_root.join("index.html"),
            assets_dir: missing_root.join("assets"),
        };

        let error = ensure_web_assets(&config).expect_err("missing index should fail");

        assert_eq!(error.code, "web_assets_missing");
    }
}
