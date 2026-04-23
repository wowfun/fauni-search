use crate::{
    api::*,
    indexing::{build_search_response, ExecutedSearchGroup},
    model::{
        IncomingQueryImageUpload, MaintenanceActionKind, ResolvedImageQueryInput,
        ResumeJobDispatch, RetryJobDispatch, SourceActionKind, SourceActionScope,
        SourceActionTrigger, StagedSettingsModelTestFile,
    },
    provider::{provider_context_payload, QUERY_KIND_IMAGE, QUERY_KIND_TEXT},
    qdrant::{cleanup_retired_vector_space_namespace, query_qdrant},
    query_assets::*,
    runtime::{run_import_job, run_maintenance_action_job, run_source_action_job},
    sidecar::{embed_query_document, embed_query_image, embed_query_text, embed_query_video},
    state::SharedState,
    APP_BODY_LIMIT_BYTES,
};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{collections::BTreeMap, fs};

struct ParsedModelTestForm {
    provider_id: String,
    model_id: String,
    input_modality: String,
    comparison_input_modality: Option<String>,
    provider_enabled: Option<bool>,
    provider_base_url: Option<String>,
    text: Option<String>,
    comparison_text: Option<String>,
    file: Option<PendingModelTestFile>,
    comparison_file: Option<PendingModelTestFile>,
}

struct PendingModelTestFile {
    original_filename: Option<String>,
    content_type: String,
    bytes: Vec<u8>,
}

pub fn build_app(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/runtime-health", get(get_runtime_health))
        .route("/settings/providers", get(list_provider_configs))
        .route(
            "/settings/providers/:provider_id",
            axum::routing::patch(update_provider_config),
        )
        .route("/settings/model-catalog", get(get_model_catalog))
        .route(
            "/settings/content-types",
            get(get_global_content_types).patch(update_global_content_types),
        )
        .route("/settings/model-tests", post(test_model_selection))
        .route("/libraries", get(list_libraries).post(create_library))
        .route(
            "/libraries/:library_id",
            get(get_library)
                .patch(update_library)
                .delete(delete_library),
        )
        .route("/libraries/:library_id/archive", post(archive_library))
        .route("/libraries/:library_id/restore", post(restore_library))
        .route(
            "/libraries/:library_id/content-types",
            get(get_library_content_types).patch(update_library_content_types),
        )
        .route(
            "/libraries/:library_id/resolved-content-models",
            get(get_resolved_content_models),
        )
        .route(
            "/libraries/:library_id/vector-space-diagnostics",
            get(get_vector_space_diagnostics),
        )
        .route("/libraries/:library_id/imports", post(import_paths))
        .route(
            "/libraries/:library_id/source-roots",
            get(list_source_roots).post(create_source_root),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id",
            get(get_source_root)
                .patch(update_source_root)
                .delete(delete_source_root),
        )
        .route("/libraries/:library_id/sources", get(list_sources))
        .route(
            "/libraries/:library_id/refresh",
            post(refresh_library_sources),
        )
        .route(
            "/libraries/:library_id/rescan",
            post(rescan_library_sources),
        )
        .route(
            "/libraries/:library_id/rebuild",
            post(rebuild_library_sources),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id/refresh",
            post(refresh_source_root),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id/rescan",
            post(rescan_source_root),
        )
        .route(
            "/libraries/:library_id/maintenance",
            post(run_library_maintenance_action),
        )
        .route(
            "/libraries/:library_id/video-sources",
            get(list_video_sources),
        )
        .route(
            "/libraries/:library_id/query-assets/images",
            post(upload_query_image),
        )
        .route(
            "/libraries/:library_id/query-assets/videos",
            post(upload_query_video),
        )
        .route(
            "/libraries/:library_id/query-assets/documents",
            post(upload_query_document),
        )
        .route(
            "/libraries/:library_id/query-assets/images/:temp_asset_id/preview",
            get(get_query_image_preview),
        )
        .route(
            "/libraries/:library_id/query-assets/videos/:temp_asset_id/preview",
            get(get_query_video_preview),
        )
        .route(
            "/libraries/:library_id/query-assets/documents/:temp_asset_id/preview",
            get(get_query_document_preview),
        )
        .route(
            "/libraries/:library_id/video-sources/:source_id/preview",
            get(get_video_source_preview),
        )
        .route(
            "/libraries/:library_id/visual-units/:visual_unit_id",
            get(get_visual_unit),
        )
        .route(
            "/libraries/:library_id/visual-units/:visual_unit_id/preview",
            get(get_visual_unit_preview),
        )
        .route("/jobs", get(list_jobs))
        .route("/jobs/:job_id", get(get_job))
        .route("/jobs/:job_id/cancel", post(cancel_job))
        .route("/jobs/:job_id/resume", post(resume_job))
        .route("/jobs/:job_id/retry", post(retry_job))
        .route("/search/text", post(search_text))
        .route("/search/image", post(search_image))
        .route("/search/video", post(search_video))
        .route("/search/document", post(search_document))
        .layer(DefaultBodyLimit::max(APP_BODY_LIMIT_BYTES))
        .with_state(state)
}

async fn root() -> Json<RootPayload> {
    Json(RootPayload {
        name: "fauni-search",
        status: "workspace",
        stage: "search workspace",
        routes: vec![
            "GET /health",
            "GET /runtime-health",
            "GET /settings/providers",
            "PATCH /settings/providers/{provider_id}",
            "GET /settings/model-catalog",
            "GET /settings/content-types",
            "PATCH /settings/content-types",
            "POST /settings/model-tests",
            "GET /libraries",
            "POST /libraries",
            "GET /libraries/{library_id}",
            "PATCH /libraries/{library_id}",
            "DELETE /libraries/{library_id}",
            "GET /libraries/{library_id}/content-types",
            "PATCH /libraries/{library_id}/content-types",
            "GET /libraries/{library_id}/resolved-content-models",
            "GET /libraries/{library_id}/vector-space-diagnostics",
            "GET /libraries/{library_id}/source-roots",
            "POST /libraries/{library_id}/source-roots",
            "GET /libraries/{library_id}/source-roots/{source_root_id}",
            "PATCH /libraries/{library_id}/source-roots/{source_root_id}",
            "DELETE /libraries/{library_id}/source-roots/{source_root_id}",
            "GET /libraries/{library_id}/sources",
            "POST /libraries/{library_id}/imports",
            "POST /libraries/{library_id}/refresh",
            "POST /libraries/{library_id}/rescan",
            "POST /libraries/{library_id}/rebuild",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/refresh",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/rescan",
            "POST /libraries/{library_id}/maintenance",
            "GET /libraries/{library_id}/video-sources",
            "POST /libraries/{library_id}/query-assets/images",
            "POST /libraries/{library_id}/query-assets/videos",
            "POST /libraries/{library_id}/query-assets/documents",
            "GET /libraries/{library_id}/video-sources/{source_id}/preview",
            "GET /libraries/{library_id}/visual-units/{visual_unit_id}",
            "GET /libraries/{library_id}/query-assets/images/{temp_asset_id}/preview",
            "GET /libraries/{library_id}/query-assets/videos/{temp_asset_id}/preview",
            "GET /libraries/{library_id}/query-assets/documents/{temp_asset_id}/preview",
            "GET /jobs",
            "GET /jobs/{job_id}",
            "POST /jobs/{job_id}/cancel",
            "POST /jobs/{job_id}/resume",
            "POST /jobs/{job_id}/retry",
            "POST /search/text",
            "POST /search/image",
            "POST /search/video",
            "POST /search/document",
        ],
    })
}

async fn health(State(state): State<SharedState>) -> Json<HealthPayload> {
    let state = state.read().await;
    Json(HealthPayload {
        service: "app",
        status: "ok",
        env: std::env::var("FAUNI_ENV").unwrap_or_else(|_| "development".to_string()),
        libraries: state.list_libraries().libraries.len(),
        jobs: state.list_jobs(None).jobs.len(),
    })
}

async fn get_runtime_health(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<RuntimeHealthData>> {
    let mut state = state.write().await;
    Json(SuccessEnvelope {
        data: state.get_runtime_health().await,
    })
}

async fn list_provider_configs(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<ProvidersListData>> {
    let mut state = state.write().await;
    state.refresh_boot_provider_probe_cache().await;
    Json(SuccessEnvelope {
        data: state.list_provider_configs(),
    })
}

async fn update_provider_config(
    State(state): State<SharedState>,
    Path(provider_id): Path<String>,
    Json(request): Json<UpdateProviderConfigRequest>,
) -> Result<Json<SuccessEnvelope<ProviderConfigSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.update_provider_config(&provider_id, request).await?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn get_model_catalog(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<ModelCatalogData>> {
    let mut state = state.write().await;
    Json(SuccessEnvelope {
        data: state.list_model_catalog().await,
    })
}

async fn get_global_content_types(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<GlobalContentTypesData>> {
    let state = state.read().await;
    Json(SuccessEnvelope {
        data: state.get_global_content_types(),
    })
}

async fn update_global_content_types(
    State(state): State<SharedState>,
    Json(request): Json<ContentTypesPayload>,
) -> Result<Json<SuccessEnvelope<GlobalContentTypesData>>, ApiError> {
    let mut state = state.write().await;
    let data = state.update_global_content_types(request).await?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn test_model_selection(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> Result<Json<SuccessEnvelope<ModelTestData>>, ApiError> {
    let form = parse_model_test_form(&mut multipart).await?;
    let staged_file = stage_model_test_file(form.file.as_ref(), &form.input_modality, "file")?;
    let comparison_staged_file = stage_model_test_file(
        form.comparison_file.as_ref(),
        form.comparison_input_modality
            .as_deref()
            .unwrap_or_default(),
        "comparison_file",
    )?;

    let result = {
        let mut state = state.write().await;
        state
            .test_model_selection(
                &form.provider_id,
                &form.model_id,
                &form.input_modality,
                form.provider_enabled,
                form.provider_base_url.clone(),
                form.text.as_deref(),
                staged_file.as_ref(),
                form.comparison_input_modality.as_deref(),
                form.comparison_text.as_deref(),
                comparison_staged_file.as_ref(),
            )
            .await
    };

    if let Some(file) = &staged_file {
        remove_temp_query_asset_file(&file.path);
    }
    if let Some(file) = &comparison_staged_file {
        remove_temp_query_asset_file(&file.path);
    }

    Ok(Json(SuccessEnvelope { data: result? }))
}

async fn list_libraries(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<LibrariesListData>> {
    let state = state.read().await;
    Json(SuccessEnvelope {
        data: state.list_libraries(),
    })
}

async fn get_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_library(&library_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn update_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<UpdateLibraryApiRequest>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let request = normalize_update_library_request(request)?;
    let mut state = state.write().await;
    let snapshot = state.update_library(&library_id, request)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn archive_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.archive_library(&library_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn restore_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.restore_library(&library_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn delete_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let cleanup_plan = {
        let mut state = state.write().await;
        state.delete_library(&library_id)?
    };

    for temp_asset_path in cleanup_plan.temp_asset_paths {
        remove_temp_query_asset_file(&temp_asset_path);
    }

    for vector_space_id in cleanup_plan.vector_space_ids {
        if let Err(error) =
            cleanup_retired_vector_space_namespace(&library_id, &vector_space_id).await
        {
            tracing::warn!(
                library_id = %library_id,
                vector_space_id = %vector_space_id,
                "Failed to cleanup deleted library namespace: {error}"
            );
        }
    }

    Ok(Json(SuccessEnvelope {
        data: cleanup_plan.snapshot,
    }))
}

async fn parse_model_test_form(multipart: &mut Multipart) -> Result<ParsedModelTestForm, ApiError> {
    let mut provider_id = None;
    let mut model_id = None;
    let mut input_modality = None;
    let mut comparison_input_modality = None;
    let mut provider_enabled = None;
    let mut provider_base_url = None;
    let mut text = None;
    let mut comparison_text = None;
    let mut file = None;
    let mut comparison_file = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Settings model test form could not be parsed: {error}"),
            Some(json!({ "field": "multipart" })),
        )
    })? {
        let field_name = field.name().map(str::to_string).unwrap_or_default();
        match field_name.as_str() {
            "provider_id" => provider_id = Some(read_text_multipart_field(field).await?),
            "model_id" => model_id = Some(read_text_multipart_field(field).await?),
            "input_modality" => input_modality = Some(read_text_multipart_field(field).await?),
            "comparison_input_modality" => {
                comparison_input_modality = Some(read_text_multipart_field(field).await?)
            }
            "provider_enabled" => {
                provider_enabled = Some(parse_bool_form_field(
                    &field_name,
                    &read_text_multipart_field(field).await?,
                )?)
            }
            "provider_base_url" => {
                provider_base_url = Some(read_text_multipart_field(field).await?)
            }
            "text" => text = Some(read_text_multipart_field(field).await?),
            "comparison_text" => comparison_text = Some(read_text_multipart_field(field).await?),
            "file" => {
                if file.is_some() {
                    return Err(ApiError::validation_failed(
                        "Settings model test accepts exactly one file input.",
                        Some(json!({ "field": "file" })),
                    ));
                }
                let filename = field.file_name().map(str::to_string);
                let content_type = field
                    .content_type()
                    .map(str::to_string)
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let bytes = field.bytes().await.map_err(|error| {
                    ApiError::validation_failed(
                        format!("Settings model test file could not be read: {error}"),
                        Some(json!({ "field": "file" })),
                    )
                })?;
                file = Some(PendingModelTestFile {
                    original_filename: filename,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            "comparison_file" => {
                if comparison_file.is_some() {
                    return Err(ApiError::validation_failed(
                        "Settings model test accepts exactly one comparison file input.",
                        Some(json!({ "field": "comparison_file" })),
                    ));
                }
                let filename = field.file_name().map(str::to_string);
                let content_type = field
                    .content_type()
                    .map(str::to_string)
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let bytes = field.bytes().await.map_err(|error| {
                    ApiError::validation_failed(
                        format!("Settings comparison model test file could not be read: {error}"),
                        Some(json!({ "field": "comparison_file" })),
                    )
                })?;
                comparison_file = Some(PendingModelTestFile {
                    original_filename: filename,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            _ => {
                if field.file_name().is_some() {
                    return Err(ApiError::validation_failed(
                        "Unexpected file field in settings model test form.",
                        Some(json!({ "field": field_name })),
                    ));
                }
            }
        }
    }

    Ok(ParsedModelTestForm {
        provider_id: provider_id.ok_or_else(|| {
            ApiError::validation_failed(
                "Settings model test requires provider_id.",
                Some(json!({ "field": "provider_id" })),
            )
        })?,
        model_id: model_id.ok_or_else(|| {
            ApiError::validation_failed(
                "Settings model test requires model_id.",
                Some(json!({ "field": "model_id" })),
            )
        })?,
        input_modality: input_modality.ok_or_else(|| {
            ApiError::validation_failed(
                "Settings model test requires input_modality.",
                Some(json!({ "field": "input_modality" })),
            )
        })?,
        comparison_input_modality,
        provider_enabled,
        provider_base_url,
        text,
        comparison_text,
        file,
        comparison_file,
    })
}

async fn read_text_multipart_field(
    field: axum::extract::multipart::Field<'_>,
) -> Result<String, ApiError> {
    field.text().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Multipart text field could not be read: {error}"),
            Some(json!({ "field": "multipart" })),
        )
    })
}

fn parse_bool_form_field(field: &str, value: &str) -> Result<bool, ApiError> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ApiError::validation_failed(
            "Boolean multipart field must be true or false.",
            Some(json!({ "field": field, "received": other })),
        )),
    }
}

fn stage_model_test_file(
    pending_file: Option<&PendingModelTestFile>,
    input_modality: &str,
    file_field: &str,
) -> Result<Option<StagedSettingsModelTestFile>, ApiError> {
    match input_modality {
        "" => {
            if pending_file.is_some() {
                return Err(ApiError::validation_failed(
                    "comparison_file requires comparison_input_modality.",
                    Some(json!({ "field": file_field })),
                ));
            }
            Ok(None)
        }
        QUERY_KIND_TEXT => {
            if pending_file.is_some() {
                return Err(ApiError::validation_failed(
                    "text model test does not accept a file input.",
                    Some(json!({ "field": file_field })),
                ));
            }
            Ok(None)
        }
        QUERY_KIND_IMAGE => {
            let file = pending_file.ok_or_else(|| {
                ApiError::validation_failed(
                    "image model test requires one file input.",
                    Some(json!({ "field": file_field })),
                )
            })?;
            let extension =
                infer_query_image_extension(file.original_filename.as_deref(), &file.content_type)
                    .ok_or_else(|| {
                        ApiError::validation_failed(
                    "Only common image files are accepted as settings model test images right now.",
                    Some(json!({
                        "field": file_field,
                        "content_type": file.content_type,
                        "filename": file.original_filename,
                    })),
                )
                    })?;
            Ok(Some(persist_settings_model_test_image(
                IncomingQueryImageUpload {
                    bytes: file.bytes.clone(),
                    content_type: file.content_type.clone(),
                    original_filename: file.original_filename.clone(),
                    extension,
                },
            )?))
        }
        _ => Err(ApiError::validation_failed(
            "input_modality must be one of the supported settings model input types.",
            Some(json!({
                "field": "input_modality",
                "received": input_modality,
                "supported": [QUERY_KIND_TEXT, QUERY_KIND_IMAGE],
            })),
        )),
    }
}

async fn create_library(
    State(state): State<SharedState>,
    Json(request): Json<CreateLibraryApiRequest>,
) -> Result<(StatusCode, Json<SuccessEnvelope<LibrarySnapshot>>), ApiError> {
    let request = normalize_create_library_request(request)?;
    let mut state = state.write().await;
    let snapshot = state.create_library(request)?;
    Ok((
        StatusCode::CREATED,
        Json(SuccessEnvelope { data: snapshot }),
    ))
}

fn normalize_create_library_request(
    request: CreateLibraryApiRequest,
) -> Result<CreateLibraryRequest, ApiError> {
    if request.extra.contains_key("name") {
        return Err(ApiError::validation_failed(
            "Library name must be provided via display_name; the legacy name field is no longer accepted.",
            Some(json!({ "field": "name" })),
        ));
    }

    if let Some(field) = request.extra.keys().next() {
        return Err(ApiError::validation_failed(
            "Create library request contains an unsupported field.",
            Some(json!({ "field": field })),
        ));
    }

    Ok(CreateLibraryRequest {
        library_id: request.library_id,
        display_name: request.display_name,
        name: String::new(),
    })
}

fn normalize_update_library_request(
    request: UpdateLibraryApiRequest,
) -> Result<UpdateLibraryRequest, ApiError> {
    if request.extra.contains_key("library_id") {
        return Err(ApiError::validation_failed(
            "Library identity is stable; PATCH /libraries/{library_id} only accepts display_name changes.",
            Some(json!({ "field": "library_id" })),
        ));
    }

    if let Some(field) = request.extra.keys().next() {
        return Err(ApiError::validation_failed(
            "Update library request contains an unsupported field.",
            Some(json!({ "field": field })),
        ));
    }

    let Some(display_name) = request.display_name else {
        return Err(ApiError::validation_failed(
            "Update library request requires display_name.",
            Some(json!({ "field": "display_name" })),
        ));
    };

    Ok(UpdateLibraryRequest { display_name })
}

fn normalize_maintenance_action_request(
    request: MaintenanceActionRequest,
) -> Result<MaintenanceActionKind, ApiError> {
    let action = request.action.trim();
    if action.is_empty() {
        return Err(ApiError::validation_failed(
            "Maintenance action request requires action.",
            Some(json!({ "field": "action" })),
        ));
    }

    match action {
        "cleanup_retired_vector_spaces" => Ok(MaintenanceActionKind::CleanupRetiredVectorSpaces),
        _ => Err(ApiError::validation_failed(
            "Maintenance action is not supported.",
            Some(json!({ "field": "action", "action": action })),
        )),
    }
}

async fn get_library_content_types(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibraryContentTypesData>>, ApiError> {
    let state = state.read().await;
    Ok(Json(SuccessEnvelope {
        data: state.get_library_content_types(&library_id)?,
    }))
}

async fn update_library_content_types(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<ContentTypesPayload>,
) -> Result<Json<SuccessEnvelope<LibraryContentTypesData>>, ApiError> {
    let mut state = state.write().await;
    let data = state
        .update_library_content_types(&library_id, request)
        .await?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn get_resolved_content_models(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<ResolvedContentModelsData>>, ApiError> {
    let mut state = state.write().await;
    let data = state.get_resolved_content_models(&library_id).await?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn get_vector_space_diagnostics(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<VectorSpaceDiagnosticsData>>, ApiError> {
    let mut state = state.write().await;
    let data = state.get_vector_space_diagnostics(&library_id).await?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn list_source_roots(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceRootsListData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_source_roots(&library_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn get_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceRootDetailData>>, ApiError> {
    let state = state.read().await;
    let data = state.get_source_root(&library_id, &source_root_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn create_source_root(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<CreateSourceRootRequest>,
) -> Result<(StatusCode, Json<SuccessEnvelope<SourceRootSnapshot>>), ApiError> {
    let mut state = state.write().await;
    let snapshot = state.create_source_root(&library_id, request)?;
    Ok((
        StatusCode::CREATED,
        Json(SuccessEnvelope { data: snapshot }),
    ))
}

async fn update_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
    Json(request): Json<UpdateSourceRootRequest>,
) -> Result<Json<SuccessEnvelope<SourceRootSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.update_source_root(&library_id, &source_root_id, request)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn delete_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceRootSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.delete_source_root(&library_id, &source_root_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn list_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Query(query): Query<SourcesQuery>,
) -> Result<Json<SuccessEnvelope<SourcesListData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_sources(&library_id, query)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn refresh_library_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::Library,
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn rescan_library_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::Library,
            SourceActionKind::Rescan,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn rebuild_library_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::Library,
            SourceActionKind::Rebuild,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn refresh_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::SourceRoot(source_root_id),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn rescan_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::SourceRoot(source_root_id),
            SourceActionKind::Rescan,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn run_library_maintenance_action(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<MaintenanceActionRequest>,
) -> Result<Json<SuccessEnvelope<MaintenanceActionData>>, ApiError> {
    let action = normalize_maintenance_action_request(request)?;
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_maintenance_action(&library_id, action)?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_maintenance_action_job(background_state, queued_action.job_id, queued_action.plan)
                .await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn import_paths(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<ImportPathsRequest>,
) -> Result<Json<SuccessEnvelope<ImportPathsData>>, ApiError> {
    let (prepared, response) = {
        let mut state = state.write().await;
        let prepared = state.prepare_import(&library_id, request)?;
        let response = state.queue_import(&prepared)?;
        (prepared, response)
    };

    if let Some(job_id) = response.job_handle.clone() {
        let state = state.clone();
        tokio::spawn(async move {
            run_import_job(state, job_id, prepared).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn list_video_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<VideoSourcesData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_video_sources(&library_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn upload_query_image(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryImageAssetData>>), ApiError> {
    let file = read_single_query_image_part(&mut multipart).await?;
    let staged = persist_query_image_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_asset(&library_id, staged)?
    };

    Ok((StatusCode::CREATED, Json(SuccessEnvelope { data })))
}

async fn upload_query_video(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryVideoAssetData>>), ApiError> {
    let file = read_single_query_video_part(&mut multipart).await?;
    let staged = persist_query_video_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_video_asset(&library_id, staged)?
    };

    Ok((StatusCode::CREATED, Json(SuccessEnvelope { data })))
}

async fn upload_query_document(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryDocumentAssetData>>), ApiError> {
    let file = read_single_query_document_part(&mut multipart).await?;
    let staged = persist_query_document_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_document_asset(&library_id, staged)?
    };

    Ok((StatusCode::CREATED, Json(SuccessEnvelope { data })))
}

async fn get_visual_unit(
    State(state): State<SharedState>,
    Path((library_id, visual_unit_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<VisualUnitDetailData>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_visual_unit(&library_id, &visual_unit_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn get_visual_unit_preview(
    State(state): State<SharedState>,
    Path((library_id, visual_unit_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let visual_unit = {
        let state = state.read().await;
        state.get_library_visual_unit(&library_id, &visual_unit_id)?
    };

    let bytes = fs::read(&visual_unit.source_path)
        .map_err(|_| ApiError::not_found("Preview source file is not available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_visual_unit(&visual_unit)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_query_image_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query image file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query image preview content type is invalid.",
                Some(json!({ "temp_asset_id": temp_asset_id })),
            )
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_query_video_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_video_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query video file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query video preview content type is invalid.",
                Some(json!({ "temp_asset_id": temp_asset_id })),
            )
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_query_document_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_document_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query document file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query document preview content type is invalid.",
                Some(json!({ "temp_asset_id": temp_asset_id })),
            )
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_video_source_preview(
    State(state): State<SharedState>,
    Path((library_id, source_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let source = {
        let state = state.read().await;
        state.get_library_source(&library_id, &source_id)?
    };

    let bytes = fs::read(&source.source_path)
        .map_err(|_| ApiError::not_found("Video source file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_source(&source)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn list_jobs(
    State(state): State<SharedState>,
    Query(query): Query<JobsQuery>,
) -> Json<SuccessEnvelope<JobsListData>> {
    let state = state.read().await;
    Json(SuccessEnvelope {
        data: state.list_jobs(query.library_id.as_deref()),
    })
}

async fn get_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<Json<SuccessEnvelope<JobSnapshot>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_job(&job_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn cancel_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<Json<SuccessEnvelope<JobSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.request_job_cancellation(&job_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn retry_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<Json<SuccessEnvelope<JobSnapshot>>, ApiError> {
    let (snapshot, dispatch) = {
        let mut state = state.write().await;
        state.request_job_retry(&job_id)?
    };

    match dispatch {
        RetryJobDispatch::Import(prepared) => {
            let background_state = state.clone();
            let retry_job_id = snapshot.job_id.clone();
            tokio::spawn(async move {
                run_import_job(background_state, retry_job_id, prepared).await;
            });
        }
        RetryJobDispatch::SourceAction(queued_action) => {
            let background_state = state.clone();
            tokio::spawn(async move {
                run_source_action_job(background_state, queued_action.job_id, queued_action.plan)
                    .await;
            });
        }
        RetryJobDispatch::Maintenance(queued_action) => {
            let background_state = state.clone();
            tokio::spawn(async move {
                run_maintenance_action_job(
                    background_state,
                    queued_action.job_id,
                    queued_action.plan,
                )
                .await;
            });
        }
    }

    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn resume_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<Json<SuccessEnvelope<JobSnapshot>>, ApiError> {
    let (snapshot, dispatch) = {
        let mut state = state.write().await;
        state.request_job_resume(&job_id)?
    };

    match dispatch {
        ResumeJobDispatch::Import(prepared) => {
            let background_state = state.clone();
            let resumed_job_id = snapshot.job_id.clone();
            tokio::spawn(async move {
                run_import_job(background_state, resumed_job_id, prepared).await;
            });
        }
        ResumeJobDispatch::SourceAction(plan) => {
            let background_state = state.clone();
            let resumed_job_id = snapshot.job_id.clone();
            tokio::spawn(async move {
                run_source_action_job(background_state, resumed_job_id, plan).await;
            });
        }
        ResumeJobDispatch::Maintenance(plan) => {
            let background_state = state.clone();
            let resumed_job_id = snapshot.job_id.clone();
            tokio::spawn(async move {
                run_maintenance_action_job(background_state, resumed_job_id, plan).await;
            });
        }
    }

    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn search_text(
    State(state): State<SharedState>,
    Json(request): Json<TextSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let plan = {
        let mut state = state.write().await;
        state.prepare_text_search(&request).await?
    };
    let mut executed_groups = Vec::new();
    for group in &plan.execution_groups {
        let query_embedding = embed_query_text(
            request.text.trim(),
            Some(provider_context_payload(&group.resolved_model)),
        )
        .await?;
        let candidates = query_qdrant(
            &group.library_id,
            &group.vector_space_id,
            group.active_visual_unit_count,
            plan.cursor_offset.saturating_add(plan.top_k),
            &query_embedding,
        )
        .await?;
        executed_groups.push(ExecutedSearchGroup {
            library_id: group.library_id.clone(),
            query_embedding,
            candidates,
        });
    }
    let response = build_search_response(plan, executed_groups)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_image(
    State(state): State<SharedState>,
    Json(request): Json<ImageSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_image_search(&request).await?
    };
    let received_scope_kind = request
        .search_scope
        .as_ref()
        .map(|scope| scope.kind.clone())
        .unwrap_or_else(|| "library".to_string());
    let plan_library_id = (!plan.library_id.trim().is_empty())
        .then_some(plan.library_id.as_str())
        .ok_or_else(|| {
        ApiError::not_supported(
            "Current 110-image-search implementation only supports single-library search_scope.",
            Some(json!({
                "field": "search_scope.kind",
                "supported": ["library"],
                "received": received_scope_kind,
            })),
        )
    })?;

    let (query_path, query_locator) = match &query_input {
        ResolvedImageQueryInput::TempAsset(asset) => (asset.path.as_str(), None),
        ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => (
            visual_unit.source_path.as_str(),
            Some(visual_unit.locator.clone()),
        ),
    };
    let mut executed_groups = Vec::new();
    for group in &plan.execution_groups {
        let query_embedding = embed_query_image(
            query_path,
            query_locator.clone(),
            Some(provider_context_payload(&group.resolved_model)),
        )
        .await?;
        let candidates = query_qdrant(
            plan_library_id,
            &group.vector_space_id,
            group.active_visual_unit_count,
            plan.cursor_offset.saturating_add(plan.top_k),
            &query_embedding,
        )
        .await?;
        executed_groups.push(ExecutedSearchGroup {
            library_id: group.library_id.clone(),
            query_embedding,
            candidates,
        });
    }
    let response = build_search_response(plan, executed_groups)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_video(
    State(state): State<SharedState>,
    Json(request): Json<VideoSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_video_search(&request).await?
    };
    let received_scope_kind = request
        .search_scope
        .as_ref()
        .map(|scope| scope.kind.clone())
        .unwrap_or_else(|| "library".to_string());
    let plan_library_id = (!plan.library_id.trim().is_empty())
        .then_some(plan.library_id.as_str())
        .ok_or_else(|| {
        ApiError::not_supported(
            "Current 120-video-search implementation only supports single-library search_scope.",
            Some(json!({
                "field": "search_scope.kind",
                "supported": ["library"],
                "received": received_scope_kind,
            })),
        )
    })?;

    let mut executed_groups = Vec::new();
    for group in &plan.execution_groups {
        let query_embedding = embed_query_video(
            query_input.path.as_str(),
            query_input.locator.clone(),
            Some(provider_context_payload(&group.resolved_model)),
        )
        .await?;
        let candidates = query_qdrant(
            plan_library_id,
            &group.vector_space_id,
            group.active_visual_unit_count,
            plan.cursor_offset.saturating_add(plan.top_k),
            &query_embedding,
        )
        .await?;
        executed_groups.push(ExecutedSearchGroup {
            library_id: group.library_id.clone(),
            query_embedding,
            candidates,
        });
    }
    let response = build_search_response(plan, executed_groups)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_document(
    State(state): State<SharedState>,
    Json(request): Json<DocumentSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_document_search(&request).await?
    };
    let received_scope_kind = request
        .search_scope
        .as_ref()
        .map(|scope| scope.kind.clone())
        .unwrap_or_else(|| "library".to_string());
    let plan_library_id = (!plan.library_id.trim().is_empty())
        .then_some(plan.library_id.as_str())
        .ok_or_else(|| {
        ApiError::not_supported(
            "Current 130-document-search implementation only supports single-library search_scope.",
            Some(json!({
                "field": "search_scope.kind",
                "supported": ["library"],
                "received": received_scope_kind,
            })),
        )
    })?;

    let mut executed_groups = Vec::new();
    for group in &plan.execution_groups {
        let query_embedding = embed_query_document(
            query_input.path.as_str(),
            query_input.locator.clone(),
            Some(provider_context_payload(&group.resolved_model)),
        )
        .await?;
        let candidates = query_qdrant(
            plan_library_id,
            &group.vector_space_id,
            group.active_visual_unit_count,
            plan.cursor_offset.saturating_add(plan.top_k),
            &query_embedding,
        )
        .await?;
        executed_groups.push(ExecutedSearchGroup {
            library_id: group.library_id.clone(),
            query_embedding,
            candidates,
        });
    }
    let response = build_search_response(plan, executed_groups)?;
    Ok(Json(SuccessEnvelope { data: response }))
}
