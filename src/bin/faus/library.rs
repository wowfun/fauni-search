use crate::{
    client::{
        app_client, fetch_json, patch_json, post_empty_json, post_json, resolve_base_url,
        AppRequest, ResolvedBaseUrl,
    },
    error::{CliError, CliFailure},
};
use clap::{Args, Subcommand};
use reqwest::Client;
use serde_json::{json, Map, Value};

#[derive(Args, Debug)]
#[command(
    about = "Manage libraries through the App API",
    after_help = "Examples:\n  faus library list\n  faus library create --display-name \"Research\"\n  faus library show demo"
)]
pub(crate) struct LibraryArgs {
    #[command(subcommand)]
    command: LibraryCommand,
}

#[derive(Subcommand, Debug)]
enum LibraryCommand {
    #[command(about = "List libraries")]
    List,
    #[command(about = "Create a library")]
    Create(CreateLibraryArgs),
    #[command(about = "Show one library")]
    Show(LibraryIdArgs),
    #[command(about = "Rename a library")]
    Rename(RenameLibraryArgs),
    #[command(about = "Archive a library")]
    Archive(LibraryIdArgs),
    #[command(about = "Restore an archived library")]
    Restore(LibraryIdArgs),
}

#[derive(Args, Debug)]
struct CreateLibraryArgs {
    #[arg(long, value_name = "NAME", help = "Human-facing library display name")]
    display_name: String,
    #[arg(long, value_name = "ID", help = "Optional stable library id")]
    library_id: Option<String>,
}

#[derive(Args, Debug)]
struct RenameLibraryArgs {
    #[arg(value_name = "LIBRARY_ID", help = "Library id")]
    library_id: String,
    #[arg(long, value_name = "NAME", help = "New human-facing display name")]
    display_name: String,
}

#[derive(Args, Debug)]
struct LibraryIdArgs {
    #[arg(value_name = "LIBRARY_ID", help = "Library id")]
    library_id: String,
}

pub(crate) async fn run_library(
    args: LibraryArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_library_command(&client, &base, args.command)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_library_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: LibraryCommand,
) -> Result<LibraryResponse, CliError> {
    match command {
        LibraryCommand::List => {
            let request = base.request("/libraries");
            let fetched = fetch_json(client, &request).await?;
            let libraries = libraries_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::List { libraries },
            })
        }
        LibraryCommand::Create(args) => {
            let request = base.request("/libraries");
            let fetched = post_json(client, &request, &create_library_body(args)).await?;
            let library = library_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::Library { library },
            })
        }
        LibraryCommand::Show(args) => {
            let request = base.request(format!("/libraries/{}", args.library_id));
            let fetched = fetch_json(client, &request).await?;
            let library = library_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::Library { library },
            })
        }
        LibraryCommand::Rename(args) => {
            let request = base.request(format!("/libraries/{}", args.library_id));
            let fetched = patch_json(
                client,
                &request,
                &json!({ "display_name": args.display_name }),
            )
            .await?;
            let library = library_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::Library { library },
            })
        }
        LibraryCommand::Archive(args) => {
            let request = base.request(format!("/libraries/{}/archive", args.library_id));
            let fetched = post_empty_json(client, &request).await?;
            let library = library_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::Library { library },
            })
        }
        LibraryCommand::Restore(args) => {
            let request = base.request(format!("/libraries/{}/restore", args.library_id));
            let fetched = post_empty_json(client, &request).await?;
            let library = library_from_envelope(&fetched.value, &request)?;
            Ok(LibraryResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: LibraryOutput::Library { library },
            })
        }
    }
}

fn create_library_body(args: CreateLibraryArgs) -> Value {
    let mut body = Map::new();
    body.insert("display_name".to_string(), Value::String(args.display_name));
    if let Some(library_id) = args.library_id {
        body.insert("library_id".to_string(), Value::String(library_id));
    }
    Value::Object(body)
}

fn libraries_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(libraries) = value.get("data").and_then(|data| data.get("libraries")) else {
        return Err(invalid_success_envelope(request, "data.libraries array"));
    };
    if !libraries.is_array() {
        return Err(invalid_success_envelope(request, "data.libraries array"));
    }
    Ok(libraries.clone())
}

fn library_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(library) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data object"));
    };
    if !library.is_object() {
        return Err(invalid_success_envelope(request, "data object"));
    }
    Ok(library.clone())
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
    response: &LibraryResponse,
    debug: bool,
) {
    let mut payload = match &response.output {
        LibraryOutput::List { libraries } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "libraries": libraries,
            },
        }),
        LibraryOutput::Library { library } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "library": library,
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
        serde_json::to_string(&payload).expect("library JSON should serialize")
    );
}

fn print_human_output(response: LibraryResponse) {
    match response.output {
        LibraryOutput::List { libraries } => {
            let libraries = libraries.as_array().expect("libraries should be an array");
            if libraries.is_empty() {
                println!("No libraries.");
                return;
            }
            for library in libraries {
                println!("{}", library_summary(library));
            }
        }
        LibraryOutput::Library { library } => println!("{}", library_summary(&library)),
    }
}

fn library_summary(library: &Value) -> String {
    let id = string_field(library, "id");
    let lifecycle_state = string_field(library, "lifecycle_state");
    let display_name = string_field(library, "display_name");
    let accepted_items = library
        .get("counts")
        .and_then(|counts| counts.get("accepted_items"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let pending_jobs = library
        .get("counts")
        .and_then(|counts| counts.get("pending_jobs"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(
        "{id}\t{lifecycle_state}\t{display_name}\taccepted={accepted_items}\tpending={pending_jobs}"
    )
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

struct LibraryResponse {
    request_url: String,
    http_status: u16,
    output: LibraryOutput,
}

enum LibraryOutput {
    List { libraries: Value },
    Library { library: Value },
}
