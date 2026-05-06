use crate::{
    client::{app_client, delete_json, fetch_json, resolve_base_url, AppRequest, ResolvedBaseUrl},
    error::{CliError, CliFailure},
};
use clap::{Args, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Args, Debug)]
#[command(
    about = "Inspect and clear query history through the App API",
    after_help = "Examples:\n  faus queries list\n  faus queries show query_000001\n  faus queries delete query_000001\n  faus queries clear"
)]
pub(crate) struct QueriesArgs {
    #[command(subcommand)]
    command: QueriesCommand,
}

#[derive(Subcommand, Debug)]
enum QueriesCommand {
    #[command(about = "List query history")]
    List(ListQueriesArgs),
    #[command(about = "Show one query history entry")]
    Show(QueryIdArgs),
    #[command(about = "Delete one query history entry")]
    Delete(QueryIdArgs),
    #[command(about = "Clear all query history")]
    Clear,
}

#[derive(Args, Debug)]
struct ListQueriesArgs {
    #[arg(long, value_name = "N", help = "Maximum number of entries")]
    limit: Option<usize>,
    #[arg(long, value_name = "CURSOR", help = "Pagination cursor")]
    cursor: Option<String>,
    #[arg(long, value_name = "KIND", help = "Filter by query kind")]
    query_kind: Option<String>,
    #[arg(long, value_name = "SOURCE", help = "Filter by source")]
    source: Option<String>,
    #[arg(long, value_name = "STATUS", help = "Filter by status")]
    status: Option<String>,
}

#[derive(Args, Debug)]
struct QueryIdArgs {
    #[arg(value_name = "QUERY_ID", help = "Query history id")]
    query_id: String,
}

pub(crate) async fn run_queries(
    args: QueriesArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_queries_command(&client, &base, args.command)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_queries_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: QueriesCommand,
) -> Result<QueriesResponse, CliError> {
    match command {
        QueriesCommand::List(args) => {
            let path = query_list_path(args);
            let request = base.request(path);
            let fetched = fetch_json(client, &request).await?;
            let data = data_object_from_envelope(&fetched.value, &request)?;
            Ok(QueriesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: QueriesOutput::List { data },
            })
        }
        QueriesCommand::Show(args) => {
            let request = base.request(format!("/queries/history/{}", args.query_id));
            let fetched = fetch_json(client, &request).await?;
            let data = data_object_from_envelope(&fetched.value, &request)?;
            Ok(QueriesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: QueriesOutput::Entry { data },
            })
        }
        QueriesCommand::Delete(args) => {
            let request = base.request(format!("/queries/history/{}", args.query_id));
            let fetched = delete_json(client, &request).await?;
            let data = data_object_from_envelope(&fetched.value, &request)?;
            Ok(QueriesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: QueriesOutput::Deleted { data },
            })
        }
        QueriesCommand::Clear => {
            let request = base.request("/queries/history");
            let fetched = delete_json(client, &request).await?;
            let data = data_object_from_envelope(&fetched.value, &request)?;
            Ok(QueriesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: QueriesOutput::Deleted { data },
            })
        }
    }
}

fn query_list_path(args: ListQueriesArgs) -> String {
    let mut pairs = Vec::new();
    if let Some(limit) = args.limit {
        pairs.push(format!("limit={limit}"));
    }
    if let Some(cursor) = args.cursor {
        pairs.push(format!("cursor={cursor}"));
    }
    if let Some(query_kind) = args.query_kind {
        pairs.push(format!("query_kind={query_kind}"));
    }
    if let Some(source) = args.source {
        pairs.push(format!("source={source}"));
    }
    if let Some(status) = args.status {
        pairs.push(format!("status={status}"));
    }
    if pairs.is_empty() {
        "/queries/history".to_string()
    } else {
        format!("/queries/history?{}", pairs.join("&"))
    }
}

fn data_object_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data object"));
    };
    if !data.is_object() {
        return Err(invalid_success_envelope(request, "data object"));
    }
    Ok(data.clone())
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
    response: &QueriesResponse,
    debug: bool,
) {
    let mut payload = match &response.output {
        QueriesOutput::List { data } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "history": data,
            },
        }),
        QueriesOutput::Entry { data } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "query": data,
            },
        }),
        QueriesOutput::Deleted { data } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "delete": data,
            },
        }),
    };
    if debug {
        payload["debug"] = json!({
            "base_url_source": base_url_source,
            "request_url": response.request_url,
            "http_status": response.http_status,
        });
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("query CLI JSON output must serialize")
    );
}

fn print_human_output(response: QueriesResponse) {
    match response.output {
        QueriesOutput::List { data } => {
            let items = data
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            println!("queries={}", items.len());
            for item in items {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    string_field(&item, "query_id").unwrap_or("-"),
                    string_field(&item, "query_kind").unwrap_or("-"),
                    string_field(&item, "status").unwrap_or("-"),
                    string_field(&item, "scope_summary").unwrap_or("-"),
                    string_field(&item, "input_summary").unwrap_or("-")
                );
            }
            if let Some(cursor) = data.get("next_cursor").and_then(Value::as_str) {
                println!("next_cursor={cursor}");
            }
        }
        QueriesOutput::Entry { data } => {
            println!(
                "query_id={}",
                string_field(&data, "query_id").unwrap_or("-")
            );
            println!("kind={}", string_field(&data, "query_kind").unwrap_or("-"));
            println!("status={}", string_field(&data, "status").unwrap_or("-"));
            println!(
                "summary={}",
                string_field(&data, "input_summary").unwrap_or("-")
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&data).expect("query detail must serialize")
            );
        }
        QueriesOutput::Deleted { data } => {
            println!("deleted={}", usize_field(&data, "deleted").unwrap_or(0));
            println!(
                "query_assets_deleted={}",
                usize_field(&data, "query_assets_deleted").unwrap_or(0)
            );
        }
    }
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn usize_field(value: &Value, field: &str) -> Option<usize> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

struct QueriesResponse {
    request_url: String,
    http_status: u16,
    output: QueriesOutput,
}

enum QueriesOutput {
    List { data: Value },
    Entry { data: Value },
    Deleted { data: Value },
}
