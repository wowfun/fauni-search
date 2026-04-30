use crate::{
    client::{
        app_client, fetch_json, post_empty_json, post_json, post_multipart_file, resolve_base_url,
        AppRequest, ResolvedBaseUrl,
    },
    error::{CliError, CliFailure},
};
use clap::{ArgAction, Args, ValueEnum};
use reqwest::Client;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::time::sleep;

const DEFAULT_WAIT_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[derive(Args, Debug)]
#[command(
    about = "Find Asset results through the App API",
    long_about = "Prepare a local folder through public FauniSearch App APIs, or search an explicit existing scope, then return concrete Asset locations. This command does not start local processes.",
    after_help = "Examples:\n  faus find ./notes --text \"quarterly revenue\"\n  faus find ./notes --image ./query.png\n  faus find --all-libraries --text \"financial statement analysis\"\n  faus find --library-id demo --image ./query.png"
)]
pub(crate) struct FindArgs {
    #[arg(value_name = "FOLDER", help = "Local folder to prepare and search")]
    folder: Option<PathBuf>,
    #[arg(long, value_name = "TEXT", help = "Text query")]
    text: Option<String>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Local image file to upload and search with"
    )]
    image: Option<PathBuf>,
    #[arg(long, value_name = "N", help = "Maximum number of search results")]
    top_k: Option<usize>,
    #[arg(
        long = "target-content-type",
        value_name = "TYPE",
        action = ArgAction::Append,
        help = "Restrict search target content type; can be repeated"
    )]
    target_content_types: Vec<String>,
    #[arg(
        long,
        value_name = "LIBRARY_ID",
        help = "Use an existing library, or search that library when no folder is provided"
    )]
    library_id: Option<String>,
    #[arg(
        long,
        help = "Search all existing indexed libraries when no folder is provided"
    )]
    all_libraries: bool,
    #[arg(long, help = "Run source-root rescan instead of refresh")]
    rescan: bool,
    #[arg(
        long,
        value_enum,
        help = "How long to wait before searching in folder mode"
    )]
    wait_mode: Option<WaitMode>,
    #[arg(
        long,
        value_name = "MS",
        help = "Maximum time to wait for the prepare job in folder mode"
    )]
    wait_timeout_ms: Option<u64>,
    #[arg(
        long,
        value_name = "MS",
        help = "Polling interval while waiting for the prepare job in folder mode"
    )]
    poll_interval_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum WaitMode {
    Complete,
    Partial,
}

impl WaitMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
        }
    }
}

pub(crate) async fn run_find(
    args: FindArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let command =
        FindCommand::from_args(args).map_err(|error| CliFailure::client(error, json_output))?;

    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_find_command(&client, &base, command, debug)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_find_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: FindCommand,
    debug: bool,
) -> Result<FindResponse, CliError> {
    match &command.scope {
        FindScope::Folder(folder) => {
            execute_folder_find_command(client, base, &command, folder, debug).await
        }
        FindScope::Library { library_id } => {
            execute_scope_find_command(
                client,
                base,
                &command,
                SearchRequestScope::Library(library_id),
                debug,
            )
            .await
        }
        FindScope::AllLibraries => {
            execute_scope_find_command(
                client,
                base,
                &command,
                SearchRequestScope::AllLibraries,
                debug,
            )
            .await
        }
    }
}

async fn execute_folder_find_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: &FindCommand,
    folder: &FolderFindScope,
    debug: bool,
) -> Result<FindResponse, CliError> {
    let folder_path = canonical_folder(&folder.folder)?;
    let folder_path_string = folder_path.to_string_lossy().into_owned();
    let folder_uri = local_file_uri(&folder_path_string);
    let library_selection = resolve_library_selection(folder.library_id.as_deref(), &folder_path);
    let mut requests = Vec::new();

    let (library, reused_library) =
        ensure_library(client, base, &library_selection, &mut requests).await?;
    let library_id = string_field(&library, "id")
        .or_else(|| folder.library_id.clone())
        .unwrap_or_else(|| library_selection.library_id.clone());

    let (source_root, reused_source_root) = ensure_source_root(
        client,
        base,
        &library_id,
        &folder_path_string,
        &mut requests,
    )
    .await?;
    let source_root_id = required_string_field(&source_root, "source_root_id")?;

    let action = if folder.rescan { "rescan" } else { "refresh" };
    let action_data = trigger_source_root_action(
        client,
        base,
        &library_id,
        &source_root_id,
        action,
        &mut requests,
    )
    .await?;
    let job_id = action_job_id(&action_data);
    let mut job_poll_count = 0usize;
    let mut last_job = None;
    let search = if let Some(job_id) = job_id.as_deref() {
        if folder.wait_mode == WaitMode::Partial {
            let waited = wait_for_partial_results(
                client,
                base,
                &library_id,
                &source_root_id,
                &folder_uri,
                action,
                job_id,
                &command,
                debug,
                &mut requests,
            )
            .await?;
            job_poll_count = waited.poll_count;
            last_job = waited.job;
            waited.search
        } else {
            let waited = wait_for_prepare_job(
                client,
                base,
                &library_id,
                &source_root_id,
                action,
                job_id,
                folder.wait_timeout,
                folder.poll_interval,
                &mut requests,
            )
            .await?;
            job_poll_count = waited.poll_count;
            last_job = Some(waited.job);
            execute_find_search(
                client,
                base,
                SearchRequestScope::Library(&library_id),
                Some(&folder_uri),
                &command,
                debug,
                &mut requests,
            )
            .await?
        }
    } else if action_rejected(&action_data) {
        return Err(CliError::new(
            "prepare_failed",
            "The source-root prepare action was rejected and did not queue a job.",
        )
        .with_details(json!({
            "library_id": library_id,
            "source_root_id": source_root_id,
            "action": action,
            "prepare": action_data,
        })));
    } else {
        execute_find_search(
            client,
            base,
            SearchRequestScope::Library(&library_id),
            Some(&folder_uri),
            &command,
            debug,
            &mut requests,
        )
        .await?
    };

    let results = wrap_find_results(&search.search, Some(&library_id), Some(&source_root_id))?;
    let search_debug = search.search.get("debug").cloned();

    Ok(FindResponse {
        scope: json!({
            "kind": "folder",
            "library_id": library_id.clone(),
            "path_prefix": folder_uri,
        }),
        folder_input: Some(folder.folder_input.clone()),
        folder_path: Some(folder_path_string),
        library_id: Some(library_id),
        source_root_id: Some(source_root_id),
        reused_library: Some(reused_library),
        reused_source_root: Some(reused_source_root),
        prepare_status: last_job
            .as_ref()
            .and_then(|job| string_field(job, "status"))
            .map(|status| {
                if status == "completed" {
                    "ready".to_string()
                } else {
                    status
                }
            })
            .unwrap_or_else(|| "ready".to_string()),
        prepare_action: action.to_string(),
        wait_mode: folder.wait_mode.as_str().to_string(),
        job_id,
        results,
        search_debug,
        query_asset: search.query_asset,
        requests,
        job_poll_count,
        last_job,
    })
}

async fn execute_scope_find_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: &FindCommand,
    scope: SearchRequestScope<'_>,
    debug: bool,
) -> Result<FindResponse, CliError> {
    let mut requests = Vec::new();
    let search =
        execute_find_search(client, base, scope, None, command, debug, &mut requests).await?;
    let library_id = scope.library_id().map(ToString::to_string);
    let results = wrap_find_results(&search.search, library_id.as_deref(), None)?;
    let search_debug = search.search.get("debug").cloned();

    Ok(FindResponse {
        scope: scope.to_json(),
        folder_input: None,
        folder_path: None,
        library_id,
        source_root_id: None,
        reused_library: None,
        reused_source_root: None,
        prepare_status: "skipped".to_string(),
        prepare_action: "none".to_string(),
        wait_mode: "none".to_string(),
        job_id: None,
        results,
        search_debug,
        query_asset: search.query_asset,
        requests,
        job_poll_count: 0,
        last_job: None,
    })
}

async fn ensure_library(
    client: &Client,
    base: &ResolvedBaseUrl,
    selection: &LibrarySelection,
    requests: &mut Vec<RequestTrace>,
) -> Result<(Value, bool), CliError> {
    let request = base.request(format!("/libraries/{}", selection.library_id));
    match fetch_json(client, &request).await {
        Ok(fetched) => {
            requests.push(RequestTrace::new(
                "library_show",
                "GET",
                &request,
                fetched.status,
            ));
            Ok((library_from_envelope(&fetched.value, &request)?, true))
        }
        Err(error) if error.code == "not_found" && selection.managed => {
            let request = base.request("/libraries");
            let body = json!({
                "library_id": selection.library_id,
                "display_name": selection.display_name,
            });
            let fetched = post_json(client, &request, &body).await?;
            requests.push(RequestTrace::new(
                "library_create",
                "POST",
                &request,
                fetched.status,
            ));
            Ok((library_from_envelope(&fetched.value, &request)?, false))
        }
        Err(error) => Err(error),
    }
}

async fn ensure_source_root(
    client: &Client,
    base: &ResolvedBaseUrl,
    library_id: &str,
    folder_path: &str,
    requests: &mut Vec<RequestTrace>,
) -> Result<(Value, bool), CliError> {
    let request = base.request(format!("/libraries/{library_id}/source-roots"));
    let fetched = fetch_json(client, &request).await?;
    requests.push(RequestTrace::new(
        "source_roots_list",
        "GET",
        &request,
        fetched.status,
    ));
    let source_roots = source_roots_from_envelope(&fetched.value, &request)?;
    if let Some(existing) = source_roots
        .as_array()
        .and_then(|roots| {
            roots
                .iter()
                .find(|root| string_field(root, "root_path").as_deref() == Some(folder_path))
        })
        .cloned()
    {
        return Ok((existing, true));
    }

    let request = base.request(format!("/libraries/{library_id}/source-roots"));
    let body = json!({ "root_path": folder_path });
    let fetched = post_json(client, &request, &body).await?;
    requests.push(RequestTrace::new(
        "source_root_create",
        "POST",
        &request,
        fetched.status,
    ));
    Ok((source_root_from_envelope(&fetched.value, &request)?, false))
}

async fn trigger_source_root_action(
    client: &Client,
    base: &ResolvedBaseUrl,
    library_id: &str,
    source_root_id: &str,
    action: &str,
    requests: &mut Vec<RequestTrace>,
) -> Result<Value, CliError> {
    let request = base.request(format!(
        "/libraries/{library_id}/source-roots/{source_root_id}/{action}"
    ));
    let fetched = post_empty_json(client, &request).await?;
    requests.push(RequestTrace::new(
        "source_root_action",
        "POST",
        &request,
        fetched.status,
    ));
    action_from_envelope(&fetched.value, &request)
}

async fn wait_for_prepare_job(
    client: &Client,
    base: &ResolvedBaseUrl,
    library_id: &str,
    source_root_id: &str,
    action: &str,
    job_id: &str,
    timeout: Duration,
    poll_interval: Duration,
    requests: &mut Vec<RequestTrace>,
) -> Result<WaitedJob, CliError> {
    let started = Instant::now();
    let mut poll_count = 0usize;

    loop {
        let request = base.request(format!("/jobs/{job_id}"));
        let fetched = fetch_json(client, &request).await?;
        requests.push(RequestTrace::new(
            "job_show",
            "GET",
            &request,
            fetched.status,
        ));
        poll_count += 1;
        let job = job_from_envelope(&fetched.value, &request)?;

        match job.get("status").and_then(Value::as_str) {
            Some("completed") => {
                return Ok(WaitedJob { job, poll_count });
            }
            Some("failed" | "canceled") => {
                return Err(CliError::new(
                    "prepare_failed",
                    format!(
                        "Prepare job `{job_id}` reached terminal status `{}`.",
                        string_field(&job, "status").unwrap_or_else(|| "unknown".to_string())
                    ),
                )
                .with_details(prepare_error_details(
                    library_id,
                    source_root_id,
                    action,
                    job_id,
                    &job,
                )));
            }
            Some("queued" | "running") | None => {}
            Some(other) => {
                return Err(CliError::new(
                    "invalid_response",
                    format!("Prepare job `{job_id}` returned unknown status `{other}`."),
                )
                .with_details(prepare_error_details(
                    library_id,
                    source_root_id,
                    action,
                    job_id,
                    &job,
                )));
            }
        }

        if started.elapsed() >= timeout {
            return Err(CliError::new(
                "wait_timeout",
                format!("Timed out waiting for prepare job `{job_id}` to complete."),
            )
            .with_hint("Retry with a larger `--wait-timeout-ms`, or inspect the job with `faus jobs show`.")
            .with_details(prepare_error_details(
                library_id,
                source_root_id,
                action,
                job_id,
                &job,
            ))
            .with_retryable(true));
        }
        sleep(poll_interval).await;
    }
}

async fn wait_for_partial_results(
    client: &Client,
    base: &ResolvedBaseUrl,
    library_id: &str,
    source_root_id: &str,
    folder_path: &str,
    action: &str,
    job_id: &str,
    command: &FindCommand,
    debug: bool,
    requests: &mut Vec<RequestTrace>,
) -> Result<PartialWaitedSearch, CliError> {
    let started = Instant::now();
    let mut poll_count = 0usize;
    let mut last_job: Option<Value>;

    loop {
        match execute_find_search(
            client,
            base,
            SearchRequestScope::Library(library_id),
            Some(folder_path),
            command,
            debug,
            requests,
        )
        .await
        {
            Ok(search) if search_result_count(&search.search) > 0 => {
                let request = base.request(format!("/jobs/{job_id}"));
                let fetched = fetch_json(client, &request).await?;
                requests.push(RequestTrace::new(
                    "job_show",
                    "GET",
                    &request,
                    fetched.status,
                ));
                poll_count += 1;
                let job = job_from_envelope(&fetched.value, &request)?;
                return Ok(PartialWaitedSearch {
                    search,
                    job: Some(job),
                    poll_count,
                });
            }
            Ok(search) => {
                let request = base.request(format!("/jobs/{job_id}"));
                let fetched = fetch_json(client, &request).await?;
                requests.push(RequestTrace::new(
                    "job_show",
                    "GET",
                    &request,
                    fetched.status,
                ));
                poll_count += 1;
                let job = job_from_envelope(&fetched.value, &request)?;
                match job.get("status").and_then(Value::as_str) {
                    Some("completed") => {
                        return Ok(PartialWaitedSearch {
                            search,
                            job: Some(job),
                            poll_count,
                        });
                    }
                    Some("failed" | "canceled") => {
                        return Err(CliError::new(
                            "prepare_failed",
                            format!(
                                "Prepare job `{job_id}` reached terminal status `{}` before active results were available.",
                                string_field(&job, "status")
                                    .unwrap_or_else(|| "unknown".to_string())
                            ),
                        )
                        .with_details(prepare_error_details(
                            library_id,
                            source_root_id,
                            action,
                            job_id,
                            &job,
                        )));
                    }
                    Some("queued" | "running") | None => {
                        last_job = Some(job);
                    }
                    Some(other) => {
                        return Err(CliError::new(
                            "invalid_response",
                            format!("Prepare job `{job_id}` returned unknown status `{other}`."),
                        )
                        .with_details(prepare_error_details(
                            library_id,
                            source_root_id,
                            action,
                            job_id,
                            &job,
                        )));
                    }
                }
            }
            Err(error) if error.code == "not_ready" => {
                let request = base.request(format!("/jobs/{job_id}"));
                let fetched = fetch_json(client, &request).await?;
                requests.push(RequestTrace::new(
                    "job_show",
                    "GET",
                    &request,
                    fetched.status,
                ));
                poll_count += 1;
                let job = job_from_envelope(&fetched.value, &request)?;
                match job.get("status").and_then(Value::as_str) {
                    Some("completed") => return Err(error),
                    Some("failed" | "canceled") => {
                        return Err(CliError::new(
                            "prepare_failed",
                            format!(
                                "Prepare job `{job_id}` reached terminal status `{}` before active results were available.",
                                string_field(&job, "status")
                                    .unwrap_or_else(|| "unknown".to_string())
                            ),
                        )
                        .with_details(prepare_error_details(
                            library_id,
                            source_root_id,
                            action,
                            job_id,
                            &job,
                        )));
                    }
                    _ => {
                        last_job = Some(job);
                    }
                }
            }
            Err(error) => return Err(error),
        }

        let FindScope::Folder(folder) = &command.scope else {
            return Err(CliError::new(
                "invalid_state",
                "Partial wait can only run in folder mode.",
            ));
        };
        if started.elapsed() >= folder.wait_timeout {
            let details = last_job.as_ref().map_or_else(
                || {
                    json!({
                        "library_id": library_id,
                        "source_root_id": source_root_id,
                        "action": action,
                        "job_id": job_id,
                    })
                },
                |job| prepare_error_details(library_id, source_root_id, action, job_id, job),
            );
            return Err(CliError::new(
                "wait_timeout",
                format!("Timed out waiting for active results from prepare job `{job_id}`."),
            )
            .with_hint("Retry with a larger `--wait-timeout-ms`, use default complete mode, or inspect the job with `faus jobs show`.")
            .with_details(details)
            .with_retryable(true));
        }
        sleep(folder.poll_interval).await;
    }
}

async fn execute_find_search(
    client: &Client,
    base: &ResolvedBaseUrl,
    scope: SearchRequestScope<'_>,
    path_prefix: Option<&str>,
    command: &FindCommand,
    debug: bool,
    requests: &mut Vec<RequestTrace>,
) -> Result<FindSearchResponse, CliError> {
    match &command.input {
        FindInput::Text(text) => {
            let request = base.request("/search/text");
            let body = text_search_body(text, scope, path_prefix, command, debug);
            let fetched = post_json(client, &request, &body).await?;
            requests.push(RequestTrace::new(
                "search",
                "POST",
                &request,
                fetched.status,
            ));
            Ok(FindSearchResponse {
                query_asset: None,
                search: search_from_envelope(&fetched.value, &request)?,
            })
        }
        FindInput::Image(path) => {
            let Some(library_id) = scope.library_id() else {
                return Err(CliError::new(
                    "not_supported",
                    "`faus find --all-libraries --image` is not supported because query image upload is library-scoped.",
                )
                .with_hint("Use `--library-id <library_id>` for image find, or use `--text` with `--all-libraries`."));
            };
            let upload_request =
                base.request(format!("/libraries/{library_id}/query-assets/images"));
            let uploaded = post_multipart_file(client, &upload_request, "file", path).await?;
            requests.push(RequestTrace::new(
                "query_asset_upload",
                "POST",
                &upload_request,
                uploaded.status,
            ));
            let query_asset = query_asset_from_envelope(&uploaded.value, &upload_request)?;
            let temp_asset_id = required_string_field(&query_asset, "temp_asset_id")?;

            let request = base.request("/search/image");
            let body = image_search_body(scope, temp_asset_id, path_prefix, command, debug);
            let fetched = post_json(client, &request, &body).await?;
            requests.push(RequestTrace::new(
                "search",
                "POST",
                &request,
                fetched.status,
            ));
            Ok(FindSearchResponse {
                query_asset: Some(query_asset),
                search: search_from_envelope(&fetched.value, &request)?,
            })
        }
    }
}

fn text_search_body(
    text: &str,
    scope: SearchRequestScope<'_>,
    path_prefix: Option<&str>,
    command: &FindCommand,
    debug: bool,
) -> Value {
    let mut body = Map::new();
    body.insert("text".to_string(), Value::String(text.to_string()));
    body.insert("search_scope".to_string(), scope.to_json());
    insert_common_search_fields(&mut body, path_prefix, command, debug);
    Value::Object(body)
}

fn image_search_body(
    scope: SearchRequestScope<'_>,
    temp_asset_id: String,
    path_prefix: Option<&str>,
    command: &FindCommand,
    debug: bool,
) -> Value {
    let mut body = Map::new();
    if let Some(library_id) = scope.library_id() {
        body.insert(
            "library_id".to_string(),
            Value::String(library_id.to_string()),
        );
    }
    body.insert("search_scope".to_string(), scope.to_json());
    body.insert(
        "image_input".to_string(),
        json!({ "kind": "temp_asset", "temp_asset_id": temp_asset_id }),
    );
    insert_common_search_fields(&mut body, path_prefix, command, debug);
    Value::Object(body)
}

fn insert_common_search_fields(
    body: &mut Map<String, Value>,
    path_prefix: Option<&str>,
    command: &FindCommand,
    debug: bool,
) {
    if let Some(path_prefix) = path_prefix {
        body.insert(
            "filters".to_string(),
            json!({
                "path_prefix": path_prefix,
            }),
        );
    }
    if let Some(top_k) = command.top_k {
        body.insert("top_k".to_string(), json!(top_k));
    }
    if !command.target_content_types.is_empty() {
        body.insert(
            "target_content_types".to_string(),
            json!(command.target_content_types),
        );
    }
    if debug {
        body.insert("debug".to_string(), Value::Bool(true));
    }
}

fn wrap_find_results(
    search: &Value,
    library_id: Option<&str>,
    source_root_id: Option<&str>,
) -> Result<Value, CliError> {
    let results = search
        .get("results")
        .and_then(Value::as_array)
        .expect("search_from_envelope ensures results array exists");
    let mut wrapped = Vec::with_capacity(results.len());
    for result in results {
        let Some(result_object) = result.as_object() else {
            return Err(CliError::new(
                "invalid_response",
                "Search results must be JSON objects.",
            ));
        };
        let mut output = result_object.clone();
        let mut location = Map::new();
        insert_location_field(&mut location, result, "library_id", library_id);
        insert_location_field(&mut location, result, "source_root_id", source_root_id);
        for field in [
            "source_id",
            "asset_id",
            "source_uri",
            "source_type",
            "asset_type",
            "locator",
            "preview",
            "job_id",
        ] {
            insert_location_field(&mut location, result, field, None);
        }
        output.insert(
            "locations".to_string(),
            Value::Array(vec![Value::Object(location)]),
        );
        wrapped.push(Value::Object(output));
    }
    Ok(Value::Array(wrapped))
}

fn search_result_count(search: &Value) -> usize {
    search
        .get("results")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn insert_location_field(
    location: &mut Map<String, Value>,
    result: &Value,
    field: &str,
    fallback: Option<&str>,
) {
    if let Some(value) = result.get(field) {
        location.insert(field.to_string(), value.clone());
    } else if let Some(value) = fallback {
        location.insert(field.to_string(), Value::String(value.to_string()));
    }
}

fn canonical_folder(path: &Path) -> Result<PathBuf, CliError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        CliError::new(
            "invalid_folder",
            format!("Could not resolve folder `{}`: {error}", path.display()),
        )
        .with_hint("Pass an existing local folder path.")
        .with_details(json!({ "folder": path.to_string_lossy() }))
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        CliError::new(
            "invalid_folder",
            format!(
                "Could not read folder metadata `{}`: {error}",
                canonical.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(CliError::new(
            "invalid_folder",
            format!("`{}` is not a directory.", canonical.display()),
        )
        .with_details(json!({ "folder": canonical.to_string_lossy() })));
    }
    fs::read_dir(&canonical).map_err(|error| {
        CliError::new(
            "invalid_folder",
            format!("Could not read folder `{}`: {error}", canonical.display()),
        )
        .with_hint("Check folder permissions and retry.")
        .with_details(json!({ "folder": canonical.to_string_lossy() }))
    })?;
    Ok(canonical)
}

fn local_file_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{path}")
    }
}

fn resolve_library_selection(library_id: Option<&str>, folder_path: &Path) -> LibrarySelection {
    match library_id {
        Some(library_id) => LibrarySelection {
            library_id: library_id.to_string(),
            display_name: String::new(),
            managed: false,
        },
        None => {
            let path = folder_path.to_string_lossy();
            let hash = sha256_hex_prefix(path.as_bytes(), 16);
            let basename = folder_path
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(path.as_ref());
            LibrarySelection {
                library_id: format!("faus-find-{hash}"),
                display_name: format!("faus find: {basename}"),
                managed: true,
            }
        }
    }
}

fn sha256_hex_prefix(bytes: &[u8], len: usize) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .take(len)
        .map(|nibble| char::from_digit(nibble as u32, 16).expect("nibble is valid hex"))
        .collect()
}

fn library_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data library object"));
    };
    if !data.is_object() || !matches!(data.get("id"), Some(Value::String(_))) {
        return Err(invalid_success_envelope(request, "data.id string"));
    }
    Ok(data.clone())
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
        return Err(invalid_success_envelope(request, "data source root object"));
    };
    let source_root = data.get("source_root").unwrap_or(data);
    if !source_root.is_object()
        || !matches!(source_root.get("source_root_id"), Some(Value::String(_)))
    {
        return Err(invalid_success_envelope(
            request,
            "data.source_root_id string",
        ));
    }
    Ok(source_root.clone())
}

fn action_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(
            request,
            "data action object with accepted and rejected arrays",
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

fn job_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(job) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data job object"));
    };
    if !job.is_object() || !matches!(job.get("job_id"), Some(Value::String(_))) {
        return Err(invalid_success_envelope(request, "data.job_id string"));
    }
    Ok(job.clone())
}

fn query_asset_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(
            request,
            "data object with temp_asset_id",
        ));
    };
    if !data.is_object() || !matches!(data.get("temp_asset_id"), Some(Value::String(_))) {
        return Err(invalid_success_envelope(
            request,
            "data.temp_asset_id string",
        ));
    }
    Ok(data.clone())
}

fn search_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(data) = value.get("data") else {
        return Err(invalid_success_envelope(
            request,
            "data object with results array",
        ));
    };
    if !data.is_object() || !matches!(data.get("results"), Some(Value::Array(_))) {
        return Err(invalid_success_envelope(request, "data.results array"));
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

fn action_job_id(action: &Value) -> Option<String> {
    action
        .get("job_handle")
        .and_then(Value::as_str)
        .or_else(|| {
            action
                .get("job")
                .and_then(|job| job.get("job_id"))
                .and_then(Value::as_str)
        })
        .map(ToString::to_string)
}

fn action_rejected(action: &Value) -> bool {
    action
        .get("rejected")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn prepare_error_details(
    library_id: &str,
    source_root_id: &str,
    action: &str,
    job_id: &str,
    job: &Value,
) -> Value {
    json!({
        "library_id": library_id,
        "source_root_id": source_root_id,
        "action": action,
        "job_id": job_id,
        "last_job": job,
    })
}

fn required_string_field(value: &Value, field: &str) -> Result<String, CliError> {
    string_field(value, field).ok_or_else(|| {
        CliError::new(
            "invalid_response",
            format!("Response object is missing `{field}` string."),
        )
    })
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn print_json_output(base_url: &str, base_url_source: &str, response: &FindResponse, debug: bool) {
    let mut data = Map::new();
    data.insert("base_url".to_string(), Value::String(base_url.to_string()));
    data.insert("scope".to_string(), response.scope.clone());
    let folder = match (&response.folder_input, &response.folder_path) {
        (Some(input), Some(path)) => json!({
            "input": input,
            "path": path,
        }),
        _ => Value::Null,
    };
    data.insert("folder".to_string(), folder);
    if let Some(library_id) = response.library_id.as_ref() {
        let mut library = Map::new();
        library.insert(
            "library_id".to_string(),
            Value::String(library_id.to_string()),
        );
        if let Some(source_root_id) = response.source_root_id.as_ref() {
            library.insert(
                "source_root_id".to_string(),
                Value::String(source_root_id.to_string()),
            );
        }
        if let Some(reused_library) = response.reused_library {
            library.insert("reused_library".to_string(), Value::Bool(reused_library));
        }
        if let Some(reused_source_root) = response.reused_source_root {
            library.insert(
                "reused_source_root".to_string(),
                Value::Bool(reused_source_root),
            );
        }
        data.insert("library".to_string(), Value::Object(library));
    }
    data.insert(
        "prepare".to_string(),
        json!({
            "status": response.prepare_status,
            "action": response.prepare_action,
            "job_id": response.job_id,
            "wait_mode": response.wait_mode,
        }),
    );
    data.insert("results".to_string(), response.results.clone());
    if let Some(query_asset) = response.query_asset.as_ref() {
        data.insert("query_asset".to_string(), query_asset.clone());
    }
    if let Some(search_debug) = response.search_debug.as_ref() {
        data.insert("debug".to_string(), search_debug.clone());
    }

    let mut payload = json!({
        "status": "ok",
        "data": Value::Object(data),
    });
    if debug {
        payload["debug"] = json!({
            "base_url_source": base_url_source,
            "job_poll_count": response.job_poll_count,
            "last_job": response.last_job,
            "requests": response.requests.iter().map(RequestTrace::to_json).collect::<Vec<_>>(),
        });
    }
    println!(
        "{}",
        serde_json::to_string(&payload).expect("find JSON should serialize")
    );
}

fn print_human_output(response: FindResponse) {
    let results = response
        .results
        .as_array()
        .expect("wrap_find_results returns an array");
    match response.folder_path.as_deref() {
        Some(folder_path) => println!("folder={folder_path}"),
        None => println!("scope={}", response.scope),
    }
    let library = response.library_id.as_deref().unwrap_or("none");
    let source_root = response.source_root_id.as_deref().unwrap_or("none");
    println!(
        "library={library}\tsource_root={source_root}\tprepare={}\tjob={}",
        response.prepare_status,
        response.job_id.as_deref().unwrap_or("none")
    );
    println!("results={}", results.len());
    if results.is_empty() {
        println!("No results.");
        return;
    }
    for (index, result) in results.iter().enumerate() {
        println!(
            "{}\tsource={}\tasset_type={}\tscore={}\tlocator={}\tpreview={}",
            index + 1,
            string_field(result, "source_uri").unwrap_or_else(|| "unknown".to_string()),
            string_field(result, "asset_type").unwrap_or_else(|| "unknown".to_string()),
            score_field(result),
            result
                .get("locator")
                .map(Value::to_string)
                .unwrap_or_else(|| "unknown".to_string()),
            result
                .get("preview")
                .and_then(|preview| preview.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("none")
        );
    }
}

fn score_field(value: &Value) -> String {
    value
        .get("score")
        .and_then(Value::as_f64)
        .map(|score| format!("{score:.4}"))
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug)]
struct FindCommand {
    scope: FindScope,
    input: FindInput,
    top_k: Option<usize>,
    target_content_types: Vec<String>,
}

impl FindCommand {
    fn from_args(args: FindArgs) -> Result<Self, CliError> {
        if matches!(args.top_k, Some(0)) {
            return Err(validation_error("`--top-k` must be greater than 0."));
        }
        if matches!(args.wait_timeout_ms, Some(0)) {
            return Err(validation_error(
                "`--wait-timeout-ms` must be greater than 0.",
            ));
        }
        if matches!(args.poll_interval_ms, Some(0)) {
            return Err(validation_error(
                "`--poll-interval-ms` must be greater than 0.",
            ));
        }

        let input_count = [args.text.is_some(), args.image.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count();
        if input_count == 0 {
            return Err(validation_error(
                "Find query input is required; pass `--text` or `--image`.",
            ));
        }
        if input_count > 1 {
            return Err(CliError::new(
                "not_supported",
                "Combined find inputs are not supported by the current App API.",
            )
            .with_hint("Use exactly one query input for now."));
        }

        let input = if let Some(text) = args.text {
            if text.trim().is_empty() {
                return Err(validation_error("`--text` must not be empty."));
            }
            FindInput::Text(text)
        } else if let Some(path) = args.image {
            FindInput::Image(path)
        } else {
            unreachable!("input_count already verified")
        };

        let scope = match args.folder {
            Some(folder) => {
                if args.all_libraries {
                    return Err(validation_error(
                        "`<folder>` cannot be combined with `--all-libraries`.",
                    ));
                }
                FolderFindScope {
                    folder_input: folder.to_string_lossy().into_owned(),
                    folder,
                    library_id: args.library_id,
                    rescan: args.rescan,
                    wait_mode: args.wait_mode.unwrap_or(WaitMode::Complete),
                    wait_timeout: Duration::from_millis(
                        args.wait_timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS),
                    ),
                    poll_interval: Duration::from_millis(
                        args.poll_interval_ms.unwrap_or(DEFAULT_POLL_INTERVAL_MS),
                    ),
                }
                .into()
            }
            None => {
                if args.rescan {
                    return Err(validation_error(
                        "`--rescan` requires a folder prepare workflow.",
                    ));
                }
                if args.wait_mode.is_some() {
                    return Err(validation_error(
                        "`--wait-mode` requires a folder prepare workflow.",
                    ));
                }
                if args.wait_timeout_ms.is_some() {
                    return Err(validation_error(
                        "`--wait-timeout-ms` requires a folder prepare workflow.",
                    ));
                }
                if args.poll_interval_ms.is_some() {
                    return Err(validation_error(
                        "`--poll-interval-ms` requires a folder prepare workflow.",
                    ));
                }
                match (args.all_libraries, args.library_id) {
                    (true, Some(_)) => {
                        return Err(validation_error(
                            "`--all-libraries` cannot be combined with `--library-id` in scope-only mode.",
                        ));
                    }
                    (true, None) => {
                        if matches!(input, FindInput::Image(_)) {
                            return Err(CliError::new(
                                "not_supported",
                                "`faus find --all-libraries --image` is not supported because query image upload is library-scoped.",
                            )
                            .with_hint("Use `--library-id <library_id>` for image find, or use `--text` with `--all-libraries`."));
                        }
                        FindScope::AllLibraries
                    }
                    (false, Some(library_id)) => FindScope::Library { library_id },
                    (false, None) => {
                        return Err(validation_error(
                            "Pass a folder, `--library-id <library_id>`, or `--all-libraries`.",
                        ));
                    }
                }
            }
        };

        Ok(Self {
            scope,
            input,
            top_k: args.top_k,
            target_content_types: args.target_content_types,
        })
    }
}

fn validation_error(message: impl Into<String>) -> CliError {
    CliError::new("validation_failed", message)
}

#[derive(Debug)]
enum FindInput {
    Text(String),
    Image(PathBuf),
}

#[derive(Debug)]
enum FindScope {
    Folder(FolderFindScope),
    Library { library_id: String },
    AllLibraries,
}

impl From<FolderFindScope> for FindScope {
    fn from(folder: FolderFindScope) -> Self {
        Self::Folder(folder)
    }
}

#[derive(Debug)]
struct FolderFindScope {
    folder: PathBuf,
    folder_input: String,
    library_id: Option<String>,
    rescan: bool,
    wait_mode: WaitMode,
    wait_timeout: Duration,
    poll_interval: Duration,
}

#[derive(Clone, Copy)]
enum SearchRequestScope<'a> {
    Library(&'a str),
    AllLibraries,
}

impl<'a> SearchRequestScope<'a> {
    fn to_json(self) -> Value {
        match self {
            Self::Library(library_id) => {
                json!({ "kind": "library", "library_id": library_id })
            }
            Self::AllLibraries => json!({ "kind": "all_libraries" }),
        }
    }

    fn library_id(self) -> Option<&'a str> {
        match self {
            Self::Library(library_id) => Some(library_id),
            Self::AllLibraries => None,
        }
    }
}

struct LibrarySelection {
    library_id: String,
    display_name: String,
    managed: bool,
}

struct FindSearchResponse {
    query_asset: Option<Value>,
    search: Value,
}

struct WaitedJob {
    job: Value,
    poll_count: usize,
}

struct PartialWaitedSearch {
    search: FindSearchResponse,
    job: Option<Value>,
    poll_count: usize,
}

struct FindResponse {
    scope: Value,
    folder_input: Option<String>,
    folder_path: Option<String>,
    library_id: Option<String>,
    source_root_id: Option<String>,
    reused_library: Option<bool>,
    reused_source_root: Option<bool>,
    prepare_status: String,
    prepare_action: String,
    wait_mode: String,
    job_id: Option<String>,
    results: Value,
    search_debug: Option<Value>,
    query_asset: Option<Value>,
    requests: Vec<RequestTrace>,
    job_poll_count: usize,
    last_job: Option<Value>,
}

struct RequestTrace {
    label: &'static str,
    method: &'static str,
    url: String,
    status: u16,
}

impl RequestTrace {
    fn new(label: &'static str, method: &'static str, request: &AppRequest, status: u16) -> Self {
        Self {
            label,
            method,
            url: request.url.clone(),
            status,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "label": self.label,
            "method": self.method,
            "url": self.url,
            "http_status": self.status,
        })
    }
}
