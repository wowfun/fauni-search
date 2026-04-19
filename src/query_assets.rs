use crate::{
    api::{ApiError, PreviewReference},
    model::{
        IncomingQueryDocumentUpload, IncomingQueryImageUpload, IncomingQueryVideoUpload,
        SourceRecord, StagedQueryAsset, StagedSettingsModelTestFile, VisualUnitRecord,
    },
    VIDEO_SEGMENT_OVERLAP_MS, VIDEO_SEGMENT_WINDOW_MS,
};
use axum::extract::Multipart;
use lopdf::Document as PdfDocument;
use serde_json::{json, Value};
use std::{
    env, fs,
    path::Path as FsPath,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) async fn read_single_query_image_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryImageUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryImageUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query image upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_image_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only common image files are accepted as query images right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query image upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query image upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 110-image-search implementation accepts exactly one query image per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryImageUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query image upload requires one image file part.",
            Some(json!({ "field": "file" })),
        )
    })
}

pub(crate) async fn read_single_query_video_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryVideoUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryVideoUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query video upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_video_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only mp4, mov, or m4v files are accepted as query videos right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query video upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query video upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 120-video-search implementation accepts exactly one query video per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryVideoUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query video upload requires one video file part.",
            Some(json!({ "field": "file" })),
        )
    })
}

pub(crate) async fn read_single_query_document_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryDocumentUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryDocumentUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query document upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_document_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only PDF files are accepted as query documents right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query document upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query document upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 130-document-search implementation accepts exactly one query document per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryDocumentUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query document upload requires one document file part.",
            Some(json!({ "field": "file" })),
        )
    })
}

pub(crate) fn infer_query_image_extension(
    filename: Option<&str>,
    content_type: &str,
) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_image_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "image/png" => Some("png".to_string()),
        "image/jpeg" => Some("jpg".to_string()),
        "image/webp" => Some("webp".to_string()),
        "image/bmp" => Some("bmp".to_string()),
        "image/gif" => Some("gif".to_string()),
        _ => None,
    }
}

pub(crate) fn infer_query_video_extension(
    filename: Option<&str>,
    content_type: &str,
) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_video_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "video/mp4" | "video/quicktime" => Some("mp4".to_string()),
        "video/x-m4v" => Some("m4v".to_string()),
        _ => None,
    }
}

pub(crate) fn infer_query_document_extension(
    filename: Option<&str>,
    content_type: &str,
) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_document_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "application/pdf" => Some("pdf".to_string()),
        _ => None,
    }
}

pub(crate) fn is_supported_query_image_extension(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif")
}

pub(crate) fn is_supported_query_video_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "mov" | "m4v")
}

pub(crate) fn is_supported_query_document_extension(extension: &str) -> bool {
    matches!(extension, "pdf")
}

pub(crate) fn persist_query_image_asset(
    upload: IncomingQueryImageUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir).join("temp-assets").join("images");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query image asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-image-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query image asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        page_count: None,
        duration_ms: None,
    })
}

pub(crate) fn persist_settings_model_test_image(
    upload: IncomingQueryImageUpload,
) -> Result<StagedSettingsModelTestFile, ApiError> {
    let path = persist_settings_model_test_file("image", &upload.extension, &upload.bytes)?;
    Ok(StagedSettingsModelTestFile {
        path,
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        size_bytes: upload.bytes.len(),
    })
}

pub(crate) fn persist_query_video_asset(
    upload: IncomingQueryVideoUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir).join("temp-assets").join("videos");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query video asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-video-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query video asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    let duration_ms = video_duration_ms(&path).map_err(|message| {
        remove_temp_query_asset_file(path.to_string_lossy().as_ref());
        ApiError::validation_failed(message, Some(json!({ "field": "file" })))
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        page_count: None,
        duration_ms: Some(duration_ms),
    })
}

pub(crate) fn persist_query_document_asset(
    upload: IncomingQueryDocumentUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir)
        .join("temp-assets")
        .join("documents");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query document asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-document-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query document asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    let page_count = pdf_page_count(&path).map_err(|message| {
        remove_temp_query_asset_file(path.to_string_lossy().as_ref());
        ApiError::validation_failed(message, Some(json!({ "field": "file" })))
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "pdf".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        page_count: Some(page_count),
        duration_ms: None,
    })
}

fn persist_settings_model_test_file(
    modality: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<String, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir)
        .join("settings-model-tests")
        .join(modality);
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Settings model test directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("settings-model-test-{}.{}", runtime_token(), extension);
    let path = target_dir.join(filename);
    fs::write(&path, bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Settings model test file could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    Ok(path.to_string_lossy().to_string())
}

pub(crate) fn pdf_page_count(path: &FsPath) -> Result<usize, String> {
    let document =
        PdfDocument::load(path).map_err(|error| format!("PDF could not be opened: {error}"))?;
    let page_count = document.get_pages().len();
    if page_count == 0 {
        return Err("PDF has no pages.".to_string());
    }
    Ok(page_count)
}

pub(crate) fn video_duration_ms(path: &FsPath) -> Result<u64, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("Video metadata could not be probed: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown ffprobe error".to_string()
        } else {
            stderr
        };
        return Err(format!("Video metadata could not be probed: {detail}"));
    }

    let duration_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let duration_secs = duration_text
        .parse::<f64>()
        .map_err(|_| format!("Video duration was invalid: {duration_text}"))?;
    Ok((duration_secs * 1000.0).round().max(1.0) as u64)
}

pub(crate) fn build_video_segment_ranges(duration_ms: u64) -> Vec<(u64, u64)> {
    let duration_ms = duration_ms.max(1);
    let mut ranges = Vec::new();
    let step_ms = VIDEO_SEGMENT_WINDOW_MS
        .saturating_sub(VIDEO_SEGMENT_OVERLAP_MS)
        .max(1);
    let mut start_ms = 0;

    loop {
        let end_ms = (start_ms + VIDEO_SEGMENT_WINDOW_MS).min(duration_ms);
        ranges.push((start_ms, end_ms.max(start_ms + 1)));
        if end_ms >= duration_ms {
            break;
        }
        start_ms += step_ms;
    }

    ranges
}

pub(crate) fn resolve_video_query_locator(
    locator: Option<&Value>,
    duration_ms: Option<u64>,
    field_name: &str,
) -> Result<Option<Value>, ApiError> {
    let duration_ms = duration_ms.ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Video duration is unavailable for the selected query input.",
            Some(json!({ "field": field_name })),
        )
    })?;

    let Some(locator) = locator else {
        return Ok(None);
    };
    let start_ms = locator
        .get("start_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Video locator must include integer start_ms.",
                Some(json!({ "field": format!("{field_name}.start_ms") })),
            )
        })?;
    let end_ms = locator
        .get("end_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Video locator must include integer end_ms.",
                Some(json!({ "field": format!("{field_name}.end_ms") })),
            )
        })?;
    if start_ms >= end_ms || end_ms > duration_ms {
        return Err(ApiError::validation_failed(
            "Video locator must satisfy 0 <= start_ms < end_ms <= duration_ms.",
            Some(json!({
                "field": field_name,
                "start_ms": start_ms,
                "end_ms": end_ms,
                "duration_ms": duration_ms,
            })),
        ));
    }

    Ok(Some(json!({
        "start_ms": start_ms,
        "end_ms": end_ms,
        "duration_ms": duration_ms,
    })))
}

pub(crate) fn resolve_document_query_locator(
    locator: Option<&Value>,
    page_count: Option<usize>,
    field_name: &str,
) -> Result<Option<Value>, ApiError> {
    let page_count = page_count.ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Document page count is unavailable for the selected query input.",
            Some(json!({ "field": field_name })),
        )
    })?;

    let Some(locator) = locator else {
        return Ok(None);
    };

    let start_page = locator
        .get("start_page")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Document locator must include integer start_page.",
                Some(json!({ "field": format!("{field_name}.start_page") })),
            )
        })?;
    let end_page = locator
        .get("end_page")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Document locator must include integer end_page.",
                Some(json!({ "field": format!("{field_name}.end_page") })),
            )
        })?;
    if start_page < 1 || end_page < start_page || end_page > page_count as u64 {
        return Err(ApiError::validation_failed(
            "Document locator must satisfy 1 <= start_page <= end_page <= page_count.",
            Some(json!({
                "field": field_name,
                "start_page": start_page,
                "end_page": end_page,
                "page_count": page_count,
            })),
        ));
    }

    Ok(Some(json!({
        "start_page": start_page,
        "end_page": end_page,
        "page_count": page_count,
    })))
}

pub(crate) fn visual_unit_preview_reference(
    library_id: &str,
    visual_unit_id: &str,
    kind: &str,
    locator: &Value,
) -> Result<PreviewReference, ApiError> {
    let base = format!(
        "{}/libraries/{}/visual-units/{}/preview",
        app_base_url()?.trim_end_matches('/'),
        library_id,
        visual_unit_id
    );
    let url = if kind == "document_page" {
        let page = locator.get("page").and_then(Value::as_u64).unwrap_or(1);
        format!("{base}#page={page}&view=FitH")
    } else if kind == "video_segment" {
        let start_seconds =
            locator.get("start_ms").and_then(Value::as_u64).unwrap_or(0) as f64 / 1000.0;
        let end_seconds =
            locator.get("end_ms").and_then(Value::as_u64).unwrap_or(0) as f64 / 1000.0;
        format!("{base}#t={start_seconds:.3},{end_seconds:.3}")
    } else {
        base
    };
    Ok(PreviewReference {
        url,
        handle: Some(format!("preview:{visual_unit_id}")),
    })
}

pub(crate) fn query_video_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/videos/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-video-preview:{temp_asset_id}")),
    })
}

pub(crate) fn query_document_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/documents/{}/preview#page=1&view=FitH",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-document-preview:{temp_asset_id}")),
    })
}

pub(crate) fn video_source_preview_reference(
    library_id: &str,
    source_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/video-sources/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            source_id
        ),
        handle: Some(format!("video-source-preview:{source_id}")),
    })
}

pub(crate) fn query_image_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/images/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-image-preview:{temp_asset_id}")),
    })
}

pub(crate) fn content_type_for_visual_unit(visual_unit: &VisualUnitRecord) -> &'static str {
    content_type_for_source_type_and_path(&visual_unit.source_type, &visual_unit.source_path)
}

pub(crate) fn content_type_for_source(source: &SourceRecord) -> &'static str {
    content_type_for_source_type_and_path(&source.source_type, &source.source_path)
}

pub(crate) fn content_type_for_source_type_and_path(
    source_type: &str,
    source_path: &str,
) -> &'static str {
    match source_type {
        "pdf" => "application/pdf",
        "video" => match FsPath::new(source_path)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("mov") => "video/quicktime",
            Some("m4v") => "video/x-m4v",
            _ => "video/mp4",
        },
        _ => match FsPath::new(source_path)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("bmp") => "image/bmp",
            _ => "image/png",
        },
    }
}

pub(crate) fn remove_temp_query_asset_file(path: &str) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to remove expired query asset file {path}: {error}");
        }
    }
}

pub(crate) fn read_required_env(name: &'static str) -> Result<String, ApiError> {
    env::var(name).map_err(|_| {
        ApiError::runtime_unavailable(
            format!(
                "Missing required environment variable {name}; source .env or use scripts/local/run.sh"
            ),
            Some(json!({ "field": name })),
        )
    })
}

pub(crate) fn app_base_url() -> Result<String, ApiError> {
    Ok(format!(
        "http://{}:{}",
        read_required_env("APP_HOST")?,
        read_required_env("APP_PORT")?,
    ))
}

pub(crate) fn runtime_token() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
