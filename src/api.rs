use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateLibraryRequest {
    pub(crate) name: String,
    pub(crate) config: Option<CreateLibraryConfigRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateLibraryConfigRequest {
    pub(crate) enabled_index_lines: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct LibraryConfigPayload {
    pub(crate) enabled_index_lines: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibrariesListData {
    pub(crate) libraries: Vec<LibrarySnapshot>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibrarySnapshot {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) config: LibraryConfigPayload,
    pub(crate) index_lines: Vec<LibraryIndexLineStatus>,
    pub(crate) counts: LibraryCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_job_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibraryIndexLineStatus {
    pub(crate) index_line: String,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct LibraryCounts {
    pub(crate) accepted_items: usize,
    pub(crate) pending_jobs: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSourceRootRequest {
    pub(crate) root_path: String,
    pub(crate) enabled: Option<bool>,
    pub(crate) rules: Option<SourceRootRulesPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSourceRootRequest {
    pub(crate) root_path: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) rules: Option<SourceRootRulesPayload>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct SourceRootRulesPayload {
    #[serde(default)]
    pub(crate) include_globs: Vec<String>,
    #[serde(default)]
    pub(crate) exclude_globs: Vec<String>,
    #[serde(default)]
    pub(crate) include_extensions: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct SourceRootCoverageSummary {
    pub(crate) observed_file_count: usize,
    pub(crate) matched_file_count: usize,
    pub(crate) active_source_count: usize,
    pub(crate) inactive_source_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_scan_at_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SourceRootLastAction {
    pub(crate) action: String,
    pub(crate) status: String,
    pub(crate) summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) job_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SourceRootSnapshot {
    pub(crate) source_root_id: String,
    pub(crate) root_path: String,
    pub(crate) enabled: bool,
    pub(crate) status: String,
    pub(crate) watch_state: String,
    pub(crate) coverage_summary: SourceRootCoverageSummary,
    pub(crate) rules: SourceRootRulesPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_action: Option<SourceRootLastAction>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceRootsListData {
    pub(crate) source_roots: Vec<SourceRootSnapshot>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceRootDetailData {
    pub(crate) source_root: SourceRootSnapshot,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourcesQuery {
    pub(crate) source_root_id: Option<String>,
    pub(crate) source_type: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourcesListData {
    pub(crate) sources: Vec<SourceInventoryItem>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceInventoryItem {
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_root_path: Option<String>,
    pub(crate) source_root_label: String,
    pub(crate) visual_unit_count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct SourceActionAcceptedItem {
    pub(crate) source_root_id: String,
    pub(crate) root_path: String,
    pub(crate) action: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct SourceActionRejectedItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) root_path: Option<String>,
    pub(crate) reason_code: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceActionData {
    pub(crate) accepted: Vec<SourceActionAcceptedItem>,
    pub(crate) rejected: Vec<SourceActionRejectedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) job_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) job: Option<JobSnapshot>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImportPathsRequest {
    pub(crate) paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ImportPathsData {
    pub(crate) accepted: Vec<ImportAcceptedItem>,
    pub(crate) rejected: Vec<ImportRejectedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) job_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) job: Option<JobSnapshot>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ImportAcceptedItem {
    pub(crate) original_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) normalized_path: Option<String>,
    pub(crate) reason_code: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_id: Option<String>,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) visual_units: Vec<VisualUnitSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ImportRejectedItem {
    pub(crate) original_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) normalized_path: Option<String>,
    pub(crate) reason_code: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct JobSnapshot {
    pub(crate) job_id: String,
    pub(crate) library_id: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) phase: String,
    pub(crate) progress: JobProgress,
    pub(crate) cancelable: bool,
    pub(crate) current_attempt: JobAttemptSnapshot,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct JobProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) unit: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct JobAttemptSnapshot {
    pub(crate) attempt: u32,
    pub(crate) status: String,
    pub(crate) summary: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct JobsListData {
    pub(crate) jobs: Vec<JobSnapshot>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct VisualUnitSummary {
    pub(crate) visual_unit_id: String,
    pub(crate) source_id: String,
    pub(crate) kind: String,
    pub(crate) source_type: String,
    pub(crate) locator: Value,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct VisualUnitSnapshot {
    pub(crate) visual_unit_id: String,
    pub(crate) source_id: String,
    pub(crate) kind: String,
    pub(crate) source_type: String,
    pub(crate) source_path: String,
    pub(crate) locator: Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct VisualUnitDetailData {
    pub(crate) visual_unit: VisualUnitSnapshot,
    pub(crate) preview: PreviewReference,
    pub(crate) neighbor_context: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JobsQuery {
    pub(crate) library_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TextSearchRequest {
    pub(crate) library_id: String,
    pub(crate) text: String,
    pub(crate) filters: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) cursor: Option<String>,
    pub(crate) debug: Option<bool>,
    pub(crate) target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImageSearchRequest {
    pub(crate) library_id: String,
    pub(crate) image_input: QueryImageInputRequest,
    pub(crate) filters: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) cursor: Option<String>,
    pub(crate) debug: Option<bool>,
    pub(crate) target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VideoSearchRequest {
    pub(crate) library_id: String,
    pub(crate) video_input: QueryVideoInputRequest,
    pub(crate) filters: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) cursor: Option<String>,
    pub(crate) debug: Option<bool>,
    pub(crate) target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DocumentSearchRequest {
    pub(crate) library_id: String,
    pub(crate) document_input: QueryDocumentInputRequest,
    pub(crate) filters: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) cursor: Option<String>,
    pub(crate) debug: Option<bool>,
    pub(crate) target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryImageInputRequest {
    pub(crate) kind: String,
    pub(crate) temp_asset_id: Option<String>,
    pub(crate) visual_unit_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryVideoInputRequest {
    pub(crate) kind: String,
    pub(crate) temp_asset_id: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) visual_unit_id: Option<String>,
    pub(crate) locator: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryDocumentInputRequest {
    pub(crate) kind: String,
    pub(crate) temp_asset_id: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) locator: Option<Value>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TextSearchData {
    pub(crate) results: Vec<SearchResultItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) debug: Option<Value>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchResultItem {
    pub(crate) visual_unit_id: String,
    pub(crate) source_id: String,
    pub(crate) preview: PreviewReference,
    pub(crate) source_path: String,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) locator: Value,
    pub(crate) cursor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) score: Option<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct PreviewReference {
    pub(crate) url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) handle: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QueryImageAssetData {
    pub(crate) temp_asset_id: String,
    pub(crate) preview: PreviewReference,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_filename: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QueryVideoAssetData {
    pub(crate) temp_asset_id: String,
    pub(crate) preview: PreviewReference,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QueryDocumentAssetData {
    pub(crate) temp_asset_id: String,
    pub(crate) preview: PreviewReference,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) page_count: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct VideoSourcesData {
    pub(crate) sources: Vec<VideoSourceSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct VideoSourceSummary {
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u64>,
    pub(crate) preview: PreviewReference,
}

#[derive(Serialize)]
pub(crate) struct RootPayload {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) stage: &'static str,
    pub(crate) routes: Vec<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct HealthPayload {
    pub(crate) service: &'static str,
    pub(crate) status: &'static str,
    pub(crate) env: String,
    pub(crate) libraries: usize,
    pub(crate) jobs: usize,
}

#[derive(Serialize)]
pub(crate) struct SuccessEnvelope<T> {
    pub(crate) data: T,
}

#[derive(Serialize)]
pub(crate) struct ErrorEnvelope {
    pub(crate) error: ErrorPayload,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorPayload {
    pub(crate) code: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retryable: Option<bool>,
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) payload: ErrorPayload,
}

impl ApiError {
    pub(crate) fn validation_failed(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            payload: ErrorPayload {
                code: "validation_failed".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            payload: ErrorPayload {
                code: "not_found".to_string(),
                message: message.into(),
                details: None,
                retryable: Some(false),
            },
        }
    }

    pub(crate) fn not_enabled(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            payload: ErrorPayload {
                code: "not_enabled".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    pub(crate) fn not_supported(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            payload: ErrorPayload {
                code: "not_supported".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    pub(crate) fn not_ready(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            payload: ErrorPayload {
                code: "not_ready".to_string(),
                message: message.into(),
                details,
                retryable: Some(true),
            },
        }
    }

    pub(crate) fn runtime_unavailable(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            payload: ErrorPayload {
                code: "runtime_unavailable".to_string(),
                message: message.into(),
                details,
                retryable: Some(true),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: self.payload,
            }),
        )
            .into_response()
    }
}
