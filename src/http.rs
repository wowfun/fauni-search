use crate::{
    api::*,
    indexing::build_search_response,
    model::{ResolvedImageQueryInput, SourceActionKind, SourceActionScope, SourceActionTrigger},
    qdrant::query_qdrant,
    query_assets::*,
    runtime::{run_import_job, run_source_action_job},
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

pub fn build_app(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/libraries", get(list_libraries).post(create_library))
        .route("/libraries/:library_id", get(get_library))
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
            "/libraries/:library_id/source-roots/:source_root_id/refresh",
            post(refresh_source_root),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id/rescan",
            post(rescan_source_root),
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
            "GET /libraries",
            "POST /libraries",
            "GET /libraries/{library_id}/source-roots",
            "POST /libraries/{library_id}/source-roots",
            "GET /libraries/{library_id}/source-roots/{source_root_id}",
            "PATCH /libraries/{library_id}/source-roots/{source_root_id}",
            "DELETE /libraries/{library_id}/source-roots/{source_root_id}",
            "GET /libraries/{library_id}/sources",
            "POST /libraries/{library_id}/imports",
            "POST /libraries/{library_id}/refresh",
            "POST /libraries/{library_id}/rescan",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/refresh",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/rescan",
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

async fn create_library(
    State(state): State<SharedState>,
    Json(request): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<SuccessEnvelope<LibrarySnapshot>>), ApiError> {
    let mut state = state.write().await;
    let snapshot = state.create_library(request)?;
    Ok((
        StatusCode::CREATED,
        Json(SuccessEnvelope { data: snapshot }),
    ))
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

async fn search_text(
    State(state): State<SharedState>,
    Json(request): Json<TextSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let plan = {
        let state = state.read().await;
        state.prepare_text_search(&request)?
    };

    let query_embedding = embed_query_text(request.text.trim()).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_image(
    State(state): State<SharedState>,
    Json(request): Json<ImageSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_image_search(&request)?
    };

    let (query_path, query_locator) = match &query_input {
        ResolvedImageQueryInput::TempAsset(asset) => (asset.path.as_str(), None),
        ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => (
            visual_unit.source_path.as_str(),
            Some(visual_unit.locator.clone()),
        ),
    };
    let query_embedding = embed_query_image(query_path, query_locator).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_video(
    State(state): State<SharedState>,
    Json(request): Json<VideoSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_video_search(&request)?
    };

    let query_embedding =
        embed_query_video(query_input.path.as_str(), query_input.locator.clone()).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_document(
    State(state): State<SharedState>,
    Json(request): Json<DocumentSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_document_search(&request)?
    };

    let query_embedding =
        embed_query_document(query_input.path.as_str(), query_input.locator).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}
