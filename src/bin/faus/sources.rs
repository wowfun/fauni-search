use crate::{
    client::{
        app_client, delete_json, fetch_json, patch_json, post_empty_json, post_json,
        resolve_base_url, AppRequest, ResolvedBaseUrl,
    },
    error::{CliError, CliFailure},
};
use clap::{ArgAction, Args, Subcommand};
use reqwest::{Client, Url};
use serde_json::{json, Map, Value};
use std::{env, path::PathBuf};

#[derive(Args, Debug)]
#[command(
    about = "Manage library sources through the App API",
    long_about = "Manage library source roots, inspect the source inventory, and queue refresh or rescan actions through the FauniSearch App API. This command does not start local processes.",
    after_help = "Examples:\n  faus sources roots list --library-id demo\n  faus sources roots create --library-id demo --root-path ./docs --include-extension pdf\n  faus sources list --library-id demo --status ready\n  faus sources refresh --library-id demo\n  faus sources rescan --library-id demo --source-root-id root_000001"
)]
pub(crate) struct SourcesArgs {
    #[command(subcommand)]
    command: SourcesCommand,
}

#[derive(Subcommand, Debug)]
enum SourcesCommand {
    #[command(about = "Manage source roots")]
    Roots(RootsArgs),
    #[command(about = "List source inventory")]
    List(ListSourcesArgs),
    #[command(about = "Queue a library or source-root refresh")]
    Refresh(SourceActionArgs),
    #[command(about = "Queue a library or source-root rescan")]
    Rescan(SourceActionArgs),
}

#[derive(Args, Debug)]
struct RootsArgs {
    #[command(subcommand)]
    command: RootsCommand,
}

#[derive(Subcommand, Debug)]
enum RootsCommand {
    #[command(about = "List source roots")]
    List(LibraryScopeArgs),
    #[command(about = "Create a source root")]
    Create(CreateSourceRootArgs),
    #[command(about = "Show one source root")]
    Show(SourceRootIdArgs),
    #[command(about = "Update one source root")]
    Update(UpdateSourceRootArgs),
    #[command(about = "Delete one source root")]
    Delete(SourceRootIdArgs),
}

#[derive(Args, Debug)]
struct LibraryScopeArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
}

#[derive(Args, Debug)]
struct CreateSourceRootArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(long, value_name = "PATH", help = "Local source root path")]
    root_path: PathBuf,
    #[arg(long, help = "Create the source root disabled")]
    disabled: bool,
    #[command(flatten)]
    rules: SourceRootRuleArgs,
}

#[derive(Args, Debug)]
struct SourceRootIdArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(value_name = "SOURCE_ROOT_ID", help = "Source root id")]
    source_root_id: String,
}

#[derive(Args, Debug)]
struct UpdateSourceRootArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(value_name = "SOURCE_ROOT_ID", help = "Source root id")]
    source_root_id: String,
    #[arg(long, value_name = "PATH", help = "Replacement local source root path")]
    root_path: Option<PathBuf>,
    #[arg(long, conflicts_with = "disable", help = "Enable the source root")]
    enable: bool,
    #[arg(long, conflicts_with = "enable", help = "Disable the source root")]
    disable: bool,
    #[command(flatten)]
    rules: SourceRootRuleArgs,
}

#[derive(Args, Debug, Default)]
struct SourceRootRuleArgs {
    #[arg(
        long = "include-glob",
        value_name = "GLOB",
        action = ArgAction::Append,
        help = "Include glob relative to the source root; can be repeated"
    )]
    include_globs: Vec<String>,
    #[arg(
        long = "exclude-glob",
        value_name = "GLOB",
        action = ArgAction::Append,
        help = "Exclude glob relative to the source root; can be repeated"
    )]
    exclude_globs: Vec<String>,
    #[arg(
        long = "include-extension",
        value_name = "EXT",
        action = ArgAction::Append,
        help = "Allowed source extension; can be repeated"
    )]
    include_extensions: Vec<String>,
}

#[derive(Args, Debug)]
struct ListSourcesArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(long, value_name = "SOURCE_ROOT_ID", help = "Filter by source root id")]
    source_root_id: Option<String>,
    #[arg(long, value_name = "TYPE", help = "Filter by source type")]
    source_type: Option<String>,
    #[arg(long, value_name = "STATUS", help = "Filter by source status")]
    status: Option<String>,
}

#[derive(Args, Debug)]
struct SourceActionArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Target library id")]
    library_id: String,
    #[arg(
        long,
        value_name = "SOURCE_ROOT_ID",
        help = "Limit action to one source root"
    )]
    source_root_id: Option<String>,
}

pub(crate) async fn run_sources(
    args: SourcesArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_sources_command(&client, &base, args.command)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_sources_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: SourcesCommand,
) -> Result<SourcesResponse, CliError> {
    match command {
        SourcesCommand::Roots(args) => execute_roots_command(client, base, args.command).await,
        SourcesCommand::List(args) => {
            let request = request_with_query(
                base,
                format!("/libraries/{}/sources", args.library_id),
                [
                    ("source_root_id", args.source_root_id.as_deref()),
                    ("source_type", args.source_type.as_deref()),
                    ("status", args.status.as_deref()),
                ],
            )?;
            let fetched = fetch_json(client, &request).await?;
            let sources = sources_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::Sources { sources },
            })
        }
        SourcesCommand::Refresh(args) => execute_source_action(client, base, args, "refresh").await,
        SourcesCommand::Rescan(args) => execute_source_action(client, base, args, "rescan").await,
    }
}

async fn execute_roots_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: RootsCommand,
) -> Result<SourcesResponse, CliError> {
    match command {
        RootsCommand::List(args) => {
            let request = base.request(format!("/libraries/{}/source-roots", args.library_id));
            let fetched = fetch_json(client, &request).await?;
            let source_roots = source_roots_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::SourceRoots { source_roots },
            })
        }
        RootsCommand::Create(args) => {
            let request = base.request(format!("/libraries/{}/source-roots", args.library_id));
            let body = create_source_root_body(args)?;
            let fetched = post_json(client, &request, &body).await?;
            let source_root = source_root_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::SourceRoot { source_root },
            })
        }
        RootsCommand::Show(args) => {
            let request = base.request(format!(
                "/libraries/{}/source-roots/{}",
                args.library_id, args.source_root_id
            ));
            let fetched = fetch_json(client, &request).await?;
            let source_root = source_root_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::SourceRoot { source_root },
            })
        }
        RootsCommand::Update(args) => {
            let request = base.request(format!(
                "/libraries/{}/source-roots/{}",
                args.library_id, args.source_root_id
            ));
            let body = update_source_root_body(args)?;
            let fetched = patch_json(client, &request, &body).await?;
            let source_root = source_root_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::SourceRoot { source_root },
            })
        }
        RootsCommand::Delete(args) => {
            let request = base.request(format!(
                "/libraries/{}/source-roots/{}",
                args.library_id, args.source_root_id
            ));
            let fetched = delete_json(client, &request).await?;
            let source_root = source_root_from_envelope(&fetched.value, &request)?;
            Ok(SourcesResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: SourcesOutput::SourceRoot { source_root },
            })
        }
    }
}

async fn execute_source_action(
    client: &Client,
    base: &ResolvedBaseUrl,
    args: SourceActionArgs,
    action: &str,
) -> Result<SourcesResponse, CliError> {
    let path = match args.source_root_id {
        Some(source_root_id) => format!(
            "/libraries/{}/source-roots/{}/{}",
            args.library_id, source_root_id, action
        ),
        None => format!("/libraries/{}/{}", args.library_id, action),
    };
    let request = base.request(path);
    let fetched = post_empty_json(client, &request).await?;
    let action = action_from_envelope(&fetched.value, &request)?;
    Ok(SourcesResponse {
        request_url: request.url,
        http_status: fetched.status,
        output: SourcesOutput::Action { action },
    })
}

fn create_source_root_body(args: CreateSourceRootArgs) -> Result<Value, CliError> {
    let mut body = Map::new();
    body.insert(
        "root_path".to_string(),
        json!(absolute_path(args.root_path)?),
    );
    if args.disabled {
        body.insert("enabled".to_string(), Value::Bool(false));
    }
    if args.rules.has_any() {
        body.insert("rules".to_string(), args.rules.to_json());
    }
    Ok(Value::Object(body))
}

fn update_source_root_body(args: UpdateSourceRootArgs) -> Result<Value, CliError> {
    let mut body = Map::new();
    if let Some(root_path) = args.root_path {
        body.insert("root_path".to_string(), json!(absolute_path(root_path)?));
    }
    if args.enable {
        body.insert("enabled".to_string(), Value::Bool(true));
    }
    if args.disable {
        body.insert("enabled".to_string(), Value::Bool(false));
    }
    if args.rules.has_any() {
        body.insert("rules".to_string(), args.rules.to_json());
    }
    Ok(Value::Object(body))
}

impl SourceRootRuleArgs {
    fn has_any(&self) -> bool {
        !self.include_globs.is_empty()
            || !self.exclude_globs.is_empty()
            || !self.include_extensions.is_empty()
    }

    fn to_json(&self) -> Value {
        json!({
            "include_globs": self.include_globs,
            "exclude_globs": self.exclude_globs,
            "include_extensions": self.include_extensions,
        })
    }
}

fn absolute_path(path: PathBuf) -> Result<String, CliError> {
    if path.is_absolute() {
        return Ok(path.to_string_lossy().into_owned());
    }
    let cwd = env::current_dir().map_err(|error| {
        CliError::new(
            "current_dir_failed",
            format!("Could not resolve current working directory: {error}"),
        )
    })?;
    Ok(cwd.join(path).to_string_lossy().into_owned())
}

fn request_with_query<'a>(
    base: &ResolvedBaseUrl,
    path: impl AsRef<str>,
    pairs: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Result<AppRequest, CliError> {
    let mut request = base.request(path);
    let mut url = Url::parse(&request.url).map_err(|error| {
        CliError::new(
            "invalid_base_url",
            format!("Could not build request URL `{}`: {error}", request.url),
        )
    })?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            if let Some(value) = value {
                query.append_pair(key, value);
            }
        }
    }
    request.url = url.to_string();
    Ok(request)
}

fn source_roots_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(source_roots) = value.get("data").and_then(|data| data.get("source_roots")) else {
        return Err(invalid_success_envelope(request, "data.source_roots array"));
    };
    if !source_roots.is_array() {
        return Err(invalid_success_envelope(request, "data.source_roots array"));
    }
    Ok(source_roots.clone())
}

fn source_root_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data object"));
    };
    let source_root = data.get("source_root").unwrap_or(data);
    if !source_root.is_object()
        || !matches!(source_root.get("source_root_id"), Some(Value::String(_)))
    {
        return Err(invalid_success_envelope(request, "data source root object"));
    }
    Ok(source_root.clone())
}

fn sources_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(sources) = value.get("data").and_then(|data| data.get("sources")) else {
        return Err(invalid_success_envelope(request, "data.sources array"));
    };
    if !sources.is_array() {
        return Err(invalid_success_envelope(request, "data.sources array"));
    }
    Ok(sources.clone())
}

fn action_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(
            request,
            "data object with accepted and rejected arrays",
        ));
    };
    if !data.is_object()
        || !matches!(data.get("accepted"), Some(Value::Array(_)))
        || !matches!(data.get("rejected"), Some(Value::Array(_)))
    {
        return Err(invalid_success_envelope(
            request,
            "data.accepted and data.rejected arrays",
        ));
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
    response: &SourcesResponse,
    debug: bool,
) {
    let mut payload = match &response.output {
        SourcesOutput::SourceRoots { source_roots } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "source_roots": source_roots,
            },
        }),
        SourcesOutput::SourceRoot { source_root } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "source_root": source_root,
            },
        }),
        SourcesOutput::Sources { sources } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "sources": sources,
            },
        }),
        SourcesOutput::Action { action } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "action": action,
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
        serde_json::to_string(&payload).expect("sources JSON should serialize")
    );
}

fn print_human_output(response: SourcesResponse) {
    match response.output {
        SourcesOutput::SourceRoots { source_roots } => {
            let roots = source_roots
                .as_array()
                .expect("source roots should be an array");
            if roots.is_empty() {
                println!("No source roots.");
                return;
            }
            for root in roots {
                println!("{}", source_root_summary(root));
            }
        }
        SourcesOutput::SourceRoot { source_root } => {
            println!("{}", source_root_summary(&source_root))
        }
        SourcesOutput::Sources { sources } => {
            let sources = sources.as_array().expect("sources should be an array");
            if sources.is_empty() {
                println!("No sources.");
                return;
            }
            for source in sources {
                println!("{}", source_summary(source));
            }
        }
        SourcesOutput::Action { action } => println!("{}", action_summary(&action)),
    }
}

fn source_root_summary(root: &Value) -> String {
    let coverage = root.get("coverage_summary").unwrap_or(&Value::Null);
    format!(
        "{}\tenabled={}\tstatus={}\twatch={}\tobserved={}\tmatched={}\tactive={}\tinactive={}\t{}",
        string_field(root, "source_root_id"),
        bool_field(root, "enabled"),
        string_field(root, "status"),
        string_field(root, "watch_state"),
        number_field(coverage, "observed_file_count"),
        number_field(coverage, "matched_file_count"),
        number_field(coverage, "active_source_count"),
        number_field(coverage, "inactive_source_count"),
        string_field(root, "root_path"),
    )
}

fn source_summary(source: &Value) -> String {
    format!(
        "{}\t{}\t{}\tstatus={}\troot={}\tassets={}\t{}",
        string_field(source, "source_id"),
        string_field(source, "source_type"),
        string_field(source, "kind"),
        string_field(source, "status"),
        source
            .get("source_root_id")
            .and_then(Value::as_str)
            .unwrap_or("none"),
        number_field(source, "asset_count"),
        string_field(source, "source_uri"),
    )
}

fn action_summary(action: &Value) -> String {
    let accepted = action
        .get("accepted")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let rejected = action
        .get("rejected")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let job = action.get("job").unwrap_or(&Value::Null);
    format!(
        "accepted={accepted}\trejected={rejected}\tjob={}\tstatus={}\tphase={}",
        action
            .get("job_handle")
            .and_then(Value::as_str)
            .or_else(|| job.get("job_id").and_then(Value::as_str))
            .unwrap_or("none"),
        string_field(job, "status"),
        string_field(job, "phase"),
    )
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn number_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

struct SourcesResponse {
    request_url: String,
    http_status: u16,
    output: SourcesOutput,
}

enum SourcesOutput {
    SourceRoots { source_roots: Value },
    SourceRoot { source_root: Value },
    Sources { sources: Value },
    Action { action: Value },
}
