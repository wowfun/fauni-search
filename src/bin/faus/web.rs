use crate::{
    client::{fetch_json, fetch_text, resolve_base_url, FetchedText, ResolvedBaseUrl},
    error::{CliError, CliFailure},
    serve::{run_serve_with_ready_hook, ReadyHook, ServeArgs, ServeOutput, ServeReady},
};
use reqwest::Client;
use serde_json::json;
use std::{env, error::Error, future::Future, pin::Pin};

pub(crate) async fn run_web(
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = Client::new();

    match probe_web(&client, &base.base_url).await {
        Ok(probe) => {
            finish_web(&base, probe, false, json_output, debug);
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
            let client = Client::new();
            let probe = probe_started_runtime(&client, &ready).await?;
            finish_web(&base, probe, true, json_output, debug);
            Ok(())
        }) as Pin<Box<dyn Future<Output = crate::serve::CliResult<()>>>>
    });

    run_serve_with_ready_hook(ServeArgs::default_runtime(), debug, output, Some(hook))
        .await
        .map_err(|error| serve_error_to_failure(error, json_output))
}

async fn probe_started_runtime(client: &Client, ready: &ServeReady) -> Result<WebProbe, CliError> {
    let mut probe = probe_web(client, &ready.base_url).await?;
    probe.health_url = ready.health_url.clone();
    probe.web_url = ready.web_url.clone();
    Ok(probe)
}

fn finish_web(
    base: &ResolvedBaseUrl,
    probe: WebProbe,
    server_started: bool,
    json_output: bool,
    debug: bool,
) {
    let opened = open_browser(&probe.web_url);

    if json_output {
        let mut payload = json!({
            "status": "ok",
            "data": {
                "base_url": base.base_url,
                "web_url": probe.web_url,
                "opened": opened,
                "server_started": server_started,
            },
        });
        if debug {
            payload["debug"] = json!({
                "base_url_source": base.source,
                "health_url": probe.health_url,
                "web_url": probe.web_url,
                "health_status": probe.health_status,
                "web_status": probe.web_status,
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
        println!("Web URL: {}", probe.web_url);
        if opened {
            println!("Opened browser.");
        } else {
            println!(
                "Could not open a browser automatically; open this URL manually: {}",
                probe.web_url
            );
        }
        if debug {
            println!("Health URL: {}", probe.health_url);
            println!("Health HTTP status: {}", probe.health_status);
            println!("Web HTTP status: {}", probe.web_status);
        }
    }
}

async fn probe_web(client: &Client, base_url: &str) -> Result<WebProbe, CliError> {
    let health_url = format!("{base_url}/health");
    let health = fetch_json(client, &health_url).await.and_then(|fetched| {
        if fetched.value.is_object() {
            Ok(fetched)
        } else {
            Err(CliError::new(
                "invalid_response",
                format!("{health_url} did not return a JSON object"),
            ))
        }
    })?;

    let web_url = base_url.to_string();
    let web = fetch_text(client, &web_url).await?;
    validate_web_response(&web_url, &web)?;

    Ok(WebProbe {
        health_url,
        web_url,
        health_status: health.status,
        web_status: web.status,
    })
}

fn validate_web_response(url: &str, web: &FetchedText) -> Result<(), CliError> {
    if web.status == 503 {
        return Err(CliError::new(
            "web_assets_missing",
            format!("{url} reports that Web assets are not built"),
        )
        .with_details(json!({
            "url": url,
            "http_status": web.status,
            "body": web.body,
        })));
    }
    if !(200..300).contains(&web.status) {
        return Err(CliError::new(
            "invalid_response",
            format!("{url} returned HTTP {} for the Web entry", web.status),
        )
        .with_details(json!({ "url": url, "http_status": web.status })));
    }
    let content_type = web.content_type.as_deref().unwrap_or_default();
    if !content_type.starts_with("text/html") {
        return Err(CliError::new(
            "invalid_response",
            format!("{url} did not return HTML for the Web entry"),
        )
        .with_details(json!({
            "url": url,
            "http_status": web.status,
            "content_type": content_type,
        })));
    }
    Ok(())
}

fn should_start_default_runtime(base: &ResolvedBaseUrl, error: &CliError) -> bool {
    base.source == "default" && error.code == "connection_failed"
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
    web_url: String,
    health_status: u16,
    web_status: u16,
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
    fn non_connection_error_does_not_start_runtime() {
        let base = ResolvedBaseUrl {
            base_url: "http://127.0.0.1:53210".to_string(),
            source: "default",
        };
        let error = CliError::new("invalid_response", "bad");

        assert!(!should_start_default_runtime(&base, &error));
    }
}
