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

#[derive(Debug)]
pub(crate) struct FetchedJson {
    pub(crate) status: u16,
    pub(crate) value: Value,
}

#[derive(Debug)]
pub(crate) struct FetchedText {
    pub(crate) status: u16,
    pub(crate) content_type: Option<String>,
    pub(crate) body: String,
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

pub(crate) async fn fetch_json(client: &Client, url: &str) -> Result<FetchedJson, CliError> {
    let response = client
        .get(url)
        .timeout(STATUS_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            CliError::new(
                "connection_failed",
                format!(
                    "Could not connect to {url}; start the service with `faus serve` or scripts/local/run.sh: {error}"
                ),
            )
            .with_retryable(true)
        })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        CliError::new(
            "connection_failed",
            format!("Failed to read response from {url}: {error}"),
        )
        .with_retryable(true)
    })?;
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        CliError::new(
            "invalid_response",
            format!("{url} did not return JSON: {error}"),
        )
    })?;

    if !status.is_success() {
        return Err(error_from_envelope(&value).unwrap_or_else(|| {
            CliError::new(
                "invalid_response",
                format!("{url} returned HTTP {status} without an ErrorEnvelope"),
            )
            .with_details(json!({ "http_status": status.as_u16() }))
        }));
    }

    Ok(FetchedJson {
        status: status.as_u16(),
        value,
    })
}

pub(crate) async fn fetch_text(client: &Client, url: &str) -> Result<FetchedText, CliError> {
    let response = client
        .get(url)
        .timeout(STATUS_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            CliError::new(
                "connection_failed",
                format!(
                    "Could not connect to {url}; start the service with `faus serve` or scripts/local/run.sh: {error}"
                ),
            )
            .with_retryable(true)
        })?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().await.map_err(|error| {
        CliError::new(
            "connection_failed",
            format!("Failed to read response from {url}: {error}"),
        )
        .with_retryable(true)
    })?;

    Ok(FetchedText {
        status: status.as_u16(),
        content_type,
        body,
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
        details,
        retryable,
    })
}
