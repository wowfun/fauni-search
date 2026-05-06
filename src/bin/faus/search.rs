use crate::{
    client::{
        app_client, post_json, post_multipart_file, resolve_base_url, AppRequest, ResolvedBaseUrl,
    },
    error::{CliError, CliFailure},
};
use clap::{ArgAction, Args};
use reqwest::Client;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(
    about = "Search with text or one local query file through the App API",
    long_about = "Search through the FauniSearch App API. This command does not start local processes. The flag-based input shape reserves future combined queries, but the current App API accepts one query input at a time.",
    after_help = "Examples:\n  faus search --library-id demo --text \"terminal screen\"\n  faus search --all-libraries --text \"quarterly report\"\n  faus search --library-id demo --image ./query.png\n  faus search --library-id demo --video ./clip.mp4 --video-start-ms 42000 --video-end-ms 50000\n  faus search --library-id demo --document ./report.pdf --document-start-page 1 --document-end-page 3"
)]
pub(crate) struct SearchArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Search only one library")]
    library_id: Option<String>,
    #[arg(
        long,
        help = "Search all libraries; currently supported for --text only"
    )]
    all_libraries: bool,
    #[arg(long, value_name = "TEXT", help = "Text query")]
    text: Option<String>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Local image file to upload and search with"
    )]
    image: Option<PathBuf>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Local video file to upload and search with"
    )]
    video: Option<PathBuf>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Local document file to upload and search with"
    )]
    document: Option<PathBuf>,
    #[arg(long, value_name = "N", help = "Maximum number of search results")]
    top_k: Option<usize>,
    #[arg(long, value_name = "CURSOR", help = "Search pagination cursor")]
    cursor: Option<String>,
    #[arg(
        long = "target-content-type",
        value_name = "TYPE",
        action = ArgAction::Append,
        help = "Restrict search target content type; can be repeated"
    )]
    target_content_types: Vec<String>,
    #[arg(long, value_name = "MS", help = "Start time for --video query locator")]
    video_start_ms: Option<u64>,
    #[arg(long, value_name = "MS", help = "End time for --video query locator")]
    video_end_ms: Option<u64>,
    #[arg(
        long,
        value_name = "N",
        help = "Start page for --document query locator"
    )]
    document_start_page: Option<u64>,
    #[arg(long, value_name = "N", help = "End page for --document query locator")]
    document_end_page: Option<u64>,
}

pub(crate) async fn run_search(
    args: SearchArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let command =
        SearchCommand::from_args(args).map_err(|error| CliFailure::client(error, json_output))?;
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_search_command(&client, &base, command, debug)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_search_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: SearchCommand,
    debug: bool,
) -> Result<SearchResponse, CliError> {
    let SearchCommand {
        scope,
        input,
        common,
    } = command;
    match input {
        SearchInput::Text(text) => {
            let request = base.request("/search/text");
            let body = text_search_body(text, &scope, &common, debug);
            let fetched = post_json(client, &request, &body).await?;
            let search = search_from_envelope(&fetched.value, &request)?;
            Ok(SearchResponse {
                upload_request_url: None,
                upload_http_status: None,
                search_request_url: request.url,
                search_http_status: fetched.status,
                query_asset: None,
                search,
            })
        }
        SearchInput::Image(path) => {
            execute_file_search(
                client,
                base,
                FileSearchKind::Image,
                path,
                &scope,
                &common,
                debug,
            )
            .await
        }
        SearchInput::Video(path) => {
            execute_file_search(
                client,
                base,
                FileSearchKind::Video,
                path,
                &scope,
                &common,
                debug,
            )
            .await
        }
        SearchInput::Document(path) => {
            execute_file_search(
                client,
                base,
                FileSearchKind::Document,
                path,
                &scope,
                &common,
                debug,
            )
            .await
        }
    }
}

async fn execute_file_search(
    client: &Client,
    base: &ResolvedBaseUrl,
    kind: FileSearchKind,
    path: PathBuf,
    scope: &SearchScope,
    common: &SearchCommon,
    debug: bool,
) -> Result<SearchResponse, CliError> {
    let upload_request = match scope {
        SearchScope::Library(library_id) => base.request(format!(
            "/libraries/{library_id}/query-assets/{}",
            kind.query_asset_path_segment()
        )),
        SearchScope::AllLibraries => {
            base.request(format!("/query-assets/{}", kind.query_asset_path_segment()))
        }
    };
    let uploaded = post_multipart_file(client, &upload_request, "file", &path).await?;
    let query_asset = query_asset_from_envelope(&uploaded.value, &upload_request)?;
    let temp_asset_id = query_asset
        .get("temp_asset_id")
        .and_then(Value::as_str)
        .expect("query_asset_from_envelope ensures temp_asset_id exists")
        .to_string();

    let search_request = base.request(format!("/search/{}", kind.search_path_segment()));
    let body = file_search_body(kind, scope, temp_asset_id, common, debug);
    let searched = post_json(client, &search_request, &body).await?;
    let search = search_from_envelope(&searched.value, &search_request)?;

    Ok(SearchResponse {
        upload_request_url: Some(upload_request.url),
        upload_http_status: Some(uploaded.status),
        search_request_url: search_request.url,
        search_http_status: searched.status,
        query_asset: Some(query_asset),
        search,
    })
}

fn text_search_body(
    text: String,
    scope: &SearchScope,
    common: &SearchCommon,
    debug: bool,
) -> Value {
    let mut body = Map::new();
    body.insert("text".to_string(), Value::String(text));
    body.insert("search_scope".to_string(), scope.to_json());
    insert_common_search_fields(&mut body, common, debug);
    Value::Object(body)
}

fn file_search_body(
    kind: FileSearchKind,
    scope: &SearchScope,
    temp_asset_id: String,
    common: &SearchCommon,
    debug: bool,
) -> Value {
    let mut body = Map::new();
    if let SearchScope::Library(library_id) = scope {
        body.insert(
            "library_id".to_string(),
            Value::String(library_id.to_string()),
        );
    }
    body.insert("search_scope".to_string(), scope.to_json());

    let mut input = Map::new();
    input.insert("kind".to_string(), Value::String("temp_asset".to_string()));
    input.insert("temp_asset_id".to_string(), Value::String(temp_asset_id));
    match kind {
        FileSearchKind::Image => {
            body.insert("image_input".to_string(), Value::Object(input));
        }
        FileSearchKind::Video => {
            if let Some(locator) = common.video_locator.as_ref() {
                input.insert("locator".to_string(), locator.clone());
            }
            body.insert("video_input".to_string(), Value::Object(input));
        }
        FileSearchKind::Document => {
            if let Some(locator) = common.document_locator.as_ref() {
                input.insert("locator".to_string(), locator.clone());
            }
            body.insert("document_input".to_string(), Value::Object(input));
        }
    }
    insert_common_search_fields(&mut body, common, debug);
    Value::Object(body)
}

fn insert_common_search_fields(body: &mut Map<String, Value>, common: &SearchCommon, debug: bool) {
    if let Some(top_k) = common.top_k {
        body.insert("top_k".to_string(), json!(top_k));
    }
    if let Some(cursor) = common.cursor.as_ref() {
        body.insert("cursor".to_string(), Value::String(cursor.clone()));
    }
    if !common.target_content_types.is_empty() {
        body.insert(
            "target_content_types".to_string(),
            json!(common.target_content_types),
        );
    }
    if debug {
        body.insert("debug".to_string(), Value::Bool(true));
    }
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

fn print_json_output(
    base_url: &str,
    base_url_source: &str,
    response: &SearchResponse,
    debug: bool,
) {
    let mut data = json!({
        "base_url": base_url,
        "search": response.search,
    });
    if let Some(query_asset) = response.query_asset.as_ref() {
        data["query_asset"] = query_asset.clone();
    }

    let mut payload = json!({
        "status": "ok",
        "data": data,
    });
    if debug {
        let mut debug_payload = json!({
            "base_url_source": base_url_source,
            "search_request_url": response.search_request_url,
            "search_http_status": response.search_http_status,
        });
        if let Some(upload_request_url) = response.upload_request_url.as_ref() {
            debug_payload["upload_request_url"] = json!(upload_request_url);
        }
        if let Some(upload_http_status) = response.upload_http_status {
            debug_payload["upload_http_status"] = json!(upload_http_status);
        }
        payload["debug"] = debug_payload;
    }
    println!(
        "{}",
        serde_json::to_string(&payload).expect("search JSON should serialize")
    );
}

fn print_human_output(response: SearchResponse) {
    if let Some(query_asset) = response.query_asset.as_ref() {
        println!("query_asset={}", string_field(query_asset, "temp_asset_id"));
    }
    let results = response
        .search
        .get("results")
        .and_then(Value::as_array)
        .expect("search_from_envelope ensures results array exists");
    println!("results={}", results.len());
    if let Some(next_cursor) = response.search.get("next_cursor").and_then(Value::as_str) {
        println!("next_cursor={next_cursor}");
    }
    if results.is_empty() {
        println!("No results.");
        return;
    }
    for (index, result) in results.iter().enumerate() {
        println!(
            "{}\tlibrary={}\tkind={}\tscore={}\tlocator={}\tsource={}\tpreview={}",
            index + 1,
            string_field(result, "library_id"),
            string_field(result, "asset_type"),
            score_field(result),
            locator_field(result),
            string_field(result, "source_uri"),
            result
                .get("preview")
                .and_then(|preview| preview.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("none")
        );
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn score_field(value: &Value) -> String {
    value
        .get("score")
        .and_then(Value::as_f64)
        .map(|score| format!("{score:.4}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn locator_field(value: &Value) -> String {
    value
        .get("locator")
        .map(|locator| locator.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug)]
struct SearchCommand {
    scope: SearchScope,
    input: SearchInput,
    common: SearchCommon,
}

impl SearchCommand {
    fn from_args(args: SearchArgs) -> Result<Self, CliError> {
        if args.library_id.is_some() && args.all_libraries {
            return Err(validation_error(
                "Use either `--library-id` or `--all-libraries`, not both.",
            ));
        }
        if matches!(args.top_k, Some(0)) {
            return Err(validation_error("`--top-k` must be greater than 0."));
        }

        let scope = match (args.library_id, args.all_libraries) {
            (Some(library_id), false) => SearchScope::Library(library_id),
            (None, true) => SearchScope::AllLibraries,
            (None, false) => {
                return Err(validation_error(
                    "Search scope is required; pass `--library-id <id>` or `--all-libraries`.",
                ));
            }
            (Some(_), true) => unreachable!("both scope inputs are handled above"),
        };

        let input_count = [
            args.text.is_some(),
            args.image.is_some(),
            args.video.is_some(),
            args.document.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if input_count == 0 {
            return Err(validation_error(
                "Search query input is required; pass one of `--text`, `--image`, `--video`, or `--document`.",
            ));
        }
        if input_count > 1 {
            return Err(CliError::new(
                "not_supported",
                "Combined search inputs are not supported by the current App API.",
            )
            .with_hint(
                "Use exactly one query input for now. The flag-based command shape is reserved for future combined search.",
            ));
        }

        let input = if let Some(text) = args.text {
            SearchInput::Text(text)
        } else if let Some(path) = args.image {
            SearchInput::Image(path)
        } else if let Some(path) = args.video {
            SearchInput::Video(path)
        } else if let Some(path) = args.document {
            SearchInput::Document(path)
        } else {
            unreachable!("input_count already verified")
        };

        let video_locator = paired_locator(
            args.video_start_ms,
            args.video_end_ms,
            "--video-start-ms",
            "--video-end-ms",
            |start, end| json!({ "start_ms": start, "end_ms": end }),
        )?;
        let document_locator = paired_locator(
            args.document_start_page,
            args.document_end_page,
            "--document-start-page",
            "--document-end-page",
            |start, end| json!({ "start_page": start, "end_page": end }),
        )?;
        if video_locator.is_some() && !matches!(input, SearchInput::Video(_)) {
            return Err(validation_error(
                "`--video-start-ms` and `--video-end-ms` can only be used with `--video`.",
            ));
        }
        if document_locator.is_some() && !matches!(input, SearchInput::Document(_)) {
            return Err(validation_error(
                "`--document-start-page` and `--document-end-page` can only be used with `--document`.",
            ));
        }

        Ok(Self {
            scope,
            input,
            common: SearchCommon {
                top_k: args.top_k,
                cursor: args.cursor,
                target_content_types: args.target_content_types,
                video_locator,
                document_locator,
            },
        })
    }
}

fn paired_locator(
    start: Option<u64>,
    end: Option<u64>,
    start_flag: &'static str,
    end_flag: &'static str,
    build: impl FnOnce(u64, u64) -> Value,
) -> Result<Option<Value>, CliError> {
    match (start, end) {
        (Some(start), Some(end)) => Ok(Some(build(start, end))),
        (None, None) => Ok(None),
        _ => Err(validation_error(format!(
            "`{start_flag}` and `{end_flag}` must be passed together."
        ))),
    }
}

fn validation_error(message: impl Into<String>) -> CliError {
    CliError::new("validation_failed", message)
}

#[derive(Debug)]
enum SearchScope {
    Library(String),
    AllLibraries,
}

impl SearchScope {
    fn to_json(&self) -> Value {
        match self {
            Self::Library(library_id) => json!({
                "kind": "library",
                "library_id": library_id,
            }),
            Self::AllLibraries => json!({ "kind": "all_libraries" }),
        }
    }
}

#[derive(Debug)]
enum SearchInput {
    Text(String),
    Image(PathBuf),
    Video(PathBuf),
    Document(PathBuf),
}

#[derive(Debug)]
struct SearchCommon {
    top_k: Option<usize>,
    cursor: Option<String>,
    target_content_types: Vec<String>,
    video_locator: Option<Value>,
    document_locator: Option<Value>,
}

#[derive(Clone, Copy, Debug)]
enum FileSearchKind {
    Image,
    Video,
    Document,
}

impl FileSearchKind {
    fn query_asset_path_segment(self) -> &'static str {
        match self {
            Self::Image => "images",
            Self::Video => "videos",
            Self::Document => "documents",
        }
    }

    fn search_path_segment(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Document => "document",
        }
    }
}

struct SearchResponse {
    upload_request_url: Option<String>,
    upload_http_status: Option<u16>,
    search_request_url: String,
    search_http_status: u16,
    query_asset: Option<Value>,
    search: Value,
}
