use crate::error::CliError;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::{env, time::Duration};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:53210";
const STATUS_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) struct ResolvedBaseUrl {
    pub(crate) base_url: String,
    pub(crate) source: &'static str,
}

impl ResolvedBaseUrl {
    pub(crate) fn request(&self, path: impl AsRef<str>) -> AppRequest {
        let path = path.as_ref();
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        AppRequest {
            base_url: self.base_url.clone(),
            base_url_source: self.source,
            url: format!("{}{}", self.base_url, path),
        }
    }
}

#[derive(Debug)]
pub(crate) struct AppRequest {
    pub(crate) base_url: String,
    pub(crate) base_url_source: &'static str,
    pub(crate) url: String,
}

impl AppRequest {
    pub(crate) fn details(&self, http_status: Option<u16>) -> Value {
        let mut details = json!({
            "base_url": self.base_url,
            "base_url_source": self.base_url_source,
            "request_url": self.url,
        });
        if let Some(http_status) = http_status {
            details["http_status"] = json!(http_status);
        }
        details
    }
}

#[derive(Debug)]
pub(crate) struct FetchedJson {
    pub(crate) status: u16,
    pub(crate) value: Value,
}

pub(crate) fn resolve_base_url(base_url_arg: Option<String>) -> Result<ResolvedBaseUrl, CliError> {
    let (raw, source) = match base_url_arg {
        Some(value) => (value, "flag"),
        None => match env::var("FAUS_BASE_URL") {
            Ok(value) => (value, "env"),
            Err(_) => (DEFAULT_BASE_URL.to_string(), "default"),
        },
    };
    let trimmed = raw.trim();
    let url = Url::parse(trimmed).map_err(|error| {
        CliError::new(
            "invalid_base_url",
            format!("Invalid base URL `{trimmed}`: {error}"),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(CliError::new(
            "invalid_base_url",
            format!("Base URL must be an HTTP or HTTPS URL: `{trimmed}`"),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(CliError::new(
            "invalid_base_url",
            format!("Base URL must not include query or fragment: `{trimmed}`"),
        ));
    }
    Ok(ResolvedBaseUrl {
        base_url: url.as_str().trim_end_matches('/').to_string(),
        source,
    })
}

pub(crate) fn app_client() -> Result<Client, CliError> {
    Client::builder().no_proxy().build().map_err(|error| {
        CliError::new(
            "client_setup_failed",
            format!("Could not create App API HTTP client: {error}"),
        )
    })
}

pub(crate) async fn fetch_json(
    client: &Client,
    request: &AppRequest,
) -> Result<FetchedJson, CliError> {
    send_json_request(client.get(&request.url), request).await
}

pub(crate) async fn post_json(
    client: &Client,
    request: &AppRequest,
    body: &Value,
) -> Result<FetchedJson, CliError> {
    send_json_request(client.post(&request.url).json(body), request).await
}

pub(crate) async fn patch_json(
    client: &Client,
    request: &AppRequest,
    body: &Value,
) -> Result<FetchedJson, CliError> {
    send_json_request(client.patch(&request.url).json(body), request).await
}

pub(crate) async fn post_empty_json(
    client: &Client,
    request: &AppRequest,
) -> Result<FetchedJson, CliError> {
    send_json_request(client.post(&request.url), request).await
}

async fn send_json_request(
    request: reqwest::RequestBuilder,
    context: &AppRequest,
) -> Result<FetchedJson, CliError> {
    let response = request
        .timeout(STATUS_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            CliError::new(
                "connection_failed",
                format!("Could not connect to {}: {error}", context.url),
            )
            .with_hint(connection_hint(context))
            .with_details(context.details(None))
            .with_retryable(true)
        })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        CliError::new(
            "connection_failed",
            format!("Failed to read response from {}: {error}", context.url),
        )
        .with_hint(connection_hint(context))
        .with_details(context.details(Some(status.as_u16())))
        .with_retryable(true)
    })?;
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        CliError::new(
            "invalid_response",
            format!("{} did not return JSON: {error}", context.url),
        )
        .with_hint(invalid_json_hint())
        .with_details(context.details(Some(status.as_u16())))
    })?;

    if !status.is_success() {
        return Err(error_from_envelope(&value).unwrap_or_else(|| {
            CliError::new(
                "invalid_response",
                format!(
                    "{} returned HTTP {status} without an ErrorEnvelope",
                    context.url
                ),
            )
            .with_details(context.details(Some(status.as_u16())))
        }));
    }

    Ok(FetchedJson {
        status: status.as_u16(),
        value,
    })
}

fn error_from_envelope(value: &Value) -> Option<CliError> {
    let error = value.get("error")?.as_object()?;
    let code = error.get("code")?.as_str()?.to_string();
    let message = error.get("message")?.as_str()?.to_string();
    let details = error.get("details").cloned();
    let retryable = error.get("retryable").and_then(Value::as_bool);
    Some(CliError {
        code,
        message,
        hint: None,
        details,
        retryable,
    })
}

fn connection_hint(context: &AppRequest) -> &'static str {
    if context.base_url_source == "default" {
        "Start the local runtime with `faus serve` or open it with `faus web`, then retry."
    } else {
        "Check `--base-url` or `FAUS_BASE_URL` and make sure it points to a running FauniSearch App API server."
    }
}

fn invalid_json_hint() -> &'static str {
    "The target may still be starting, the port may be occupied by another process, or the URL may not be a FauniSearch App API server."
}
