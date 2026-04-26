use crate::{
    client::{app_client, fetch_json, resolve_base_url},
    error::{CliError, CliFailure},
};
use serde_json::{json, Value};

pub(crate) async fn run_status(
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let health_request = base.request("/health");
    let runtime_status_request = base.request("/runtime/status");
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;

    let health = fetch_json(&client, &health_request)
        .await
        .and_then(|fetched| {
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
        })
        .map_err(|error| CliFailure::client(error, json_output))?;

    let runtime_envelope = fetch_json(&client, &runtime_status_request)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;
    let Some(runtime_status) = runtime_envelope.value.get("data").cloned() else {
        return Err(CliFailure::client(
            CliError::new(
                "invalid_response",
                format!(
                    "{} did not return a SuccessEnvelope data object",
                    runtime_status_request.url
                ),
            )
            .with_hint("The server responded, but the payload did not match the FauniSearch App API contract.")
            .with_details(runtime_status_request.details(Some(runtime_envelope.status))),
            json_output,
        ));
    };
    if !runtime_status.is_object() {
        return Err(CliFailure::client(
            CliError::new(
                "invalid_response",
                format!(
                    "{} did not return a SuccessEnvelope data object",
                    runtime_status_request.url
                ),
            )
            .with_hint("The server responded, but the payload did not match the FauniSearch App API contract.")
            .with_details(runtime_status_request.details(Some(runtime_envelope.status))),
            json_output,
        ));
    }

    if json_output {
        let mut payload = json!({
            "status": "ok",
            "data": {
                "base_url": base.base_url,
                "health": health.value,
                "runtime_status": runtime_status,
            },
        });
        if debug {
            payload["debug"] = json!({
                "base_url_source": base.source,
                "health_url": health_request.url,
                "runtime_status_url": runtime_status_request.url,
                "health_status": health.status,
                "runtime_status_http_status": runtime_envelope.status,
            });
        }
        println!(
            "{}",
            serde_json::to_string(&payload).expect("status JSON should serialize")
        );
    } else {
        print_status_human(
            &base.base_url,
            &health.value,
            &runtime_status,
            debug.then_some((
                health_request.url.as_str(),
                runtime_status_request.url.as_str(),
            )),
        );
    }

    Ok(())
}

fn print_status_human(
    base_url: &str,
    health: &Value,
    runtime_status: &Value,
    debug_urls: Option<(&str, &str)>,
) {
    println!("Base URL: {base_url}");
    println!("App: {}", value_status(health));
    println!(
        "Runtime app: {}",
        component_summary(runtime_status.get("app"))
    );
    println!(
        "Qdrant: {}",
        component_summary(runtime_status.get("qdrant"))
    );
    println!("Providers: {}", providers_summary(runtime_status));
    if let Some((health_url, runtime_status_url)) = debug_urls {
        println!("Health URL: {health_url}");
        println!("Runtime Status URL: {runtime_status_url}");
    }
}

fn value_status(value: &Value) -> String {
    value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn component_summary(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "unknown".to_string();
    };
    let status = value_status(value);
    match value.get("message").and_then(Value::as_str) {
        Some(message) if !message.trim().is_empty() => format!("{status} - {message}"),
        _ => status,
    }
}

fn providers_summary(runtime_status: &Value) -> String {
    let Some(providers) = runtime_status.get("providers").and_then(Value::as_array) else {
        return "unknown".to_string();
    };
    if providers.is_empty() {
        return "none".to_string();
    }
    providers
        .iter()
        .map(|provider| {
            let id = provider
                .get("provider_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let status = provider
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            format!("{id}={status}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}
