use crate::{
    client::{app_client, post_json, resolve_base_url, AppRequest, ResolvedBaseUrl},
    error::{CliError, CliFailure},
};
use clap::Args;
use reqwest::Client;
use serde_json::{json, Value};
use std::{env, path::PathBuf};

#[derive(Args, Debug)]
#[command(
    about = "Submit local paths for import through the App API",
    long_about = "Submit one or more local paths to a FauniSearch library import endpoint. This command does not start local processes and does not wait for indexing to finish.",
    after_help = "Examples:\n  faus import --library-id demo report.pdf\n  faus --json import --library-id demo ./docs/report.pdf"
)]
pub(crate) struct ImportArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(
        value_name = "PATH",
        required = true,
        num_args = 1..,
        help = "Local path(s) to submit for import"
    )]
    paths: Vec<PathBuf>,
}

pub(crate) async fn run_import(
    args: ImportArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_import_command(&client, &base, args)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_import_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    args: ImportArgs,
) -> Result<ImportResponse, CliError> {
    let request = base.request(format!("/libraries/{}/imports", args.library_id));
    let paths = absolute_paths(args.paths)?;
    let fetched = post_json(client, &request, &json!({ "paths": paths })).await?;
    let import = import_from_envelope(&fetched.value, &request)?;
    Ok(ImportResponse {
        request_url: request.url,
        http_status: fetched.status,
        import,
    })
}

fn absolute_paths(paths: Vec<PathBuf>) -> Result<Vec<String>, CliError> {
    let cwd = env::current_dir().map_err(|error| {
        CliError::new(
            "current_dir_failed",
            format!("Could not resolve current working directory: {error}"),
        )
    })?;
    Ok(paths
        .into_iter()
        .map(|path| {
            let absolute = if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            };
            absolute.to_string_lossy().into_owned()
        })
        .collect())
}

fn import_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(import) = value.get("data") else {
        return Err(invalid_success_envelope(
            request,
            "data object with accepted and rejected arrays",
        ));
    };
    if !import.is_object() {
        return Err(invalid_success_envelope(
            request,
            "data object with accepted and rejected arrays",
        ));
    }
    let accepted = import.get("accepted");
    let rejected = import.get("rejected");
    if !matches!(accepted, Some(Value::Array(_))) || !matches!(rejected, Some(Value::Array(_))) {
        return Err(invalid_success_envelope(
            request,
            "data.accepted and data.rejected arrays",
        ));
    }
    Ok(import.clone())
}

fn invalid_success_envelope(request: &AppRequest, expected: &str) -> CliError {
    CliError::new(
        "invalid_response",
        format!(
            "{} did not return a SuccessEnvelope {expected}",
            request.url
        ),
    )
    .with_hint(
        "The server responded, but the payload did not match the FauniSearch App API contract.",
    )
    .with_details(request.details(None))
}

fn print_json_output(
    base_url: &str,
    base_url_source: &str,
    response: &ImportResponse,
    debug: bool,
) {
    let mut payload = json!({
        "status": "ok",
        "data": {
            "base_url": base_url,
            "import": response.import,
        },
    });
    if debug {
        payload["debug"] = json!({
            "base_url_source": base_url_source,
            "request_url": response.request_url,
            "http_status": response.http_status,
        });
    }
    println!(
        "{}",
        serde_json::to_string(&payload).expect("import JSON should serialize")
    );
}

fn print_human_output(response: ImportResponse) {
    let accepted_count = response
        .import
        .get("accepted")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let rejected = response
        .import
        .get("rejected")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    println!(
        "accepted={accepted_count}\trejected={}\t{}",
        rejected.len(),
        job_summary(&response.import)
    );

    for item in rejected {
        println!(
            "rejected\t{}\t{}\t{}",
            string_field(&item, "original_path"),
            string_field(&item, "reason_code"),
            string_field(&item, "message")
        );
    }
}

fn job_summary(import: &Value) -> String {
    if let Some(job) = import.get("job").filter(|value| value.is_object()) {
        return format!(
            "job={}\tstatus={}\tphase={}",
            string_field(job, "job_id"),
            string_field(job, "status"),
            string_field(job, "phase")
        );
    }
    if let Some(job_handle) = import.get("job_handle").and_then(Value::as_str) {
        return format!("job={job_handle}");
    }
    "job=none".to_string()
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

struct ImportResponse {
    request_url: String,
    http_status: u16,
    import: Value,
}
