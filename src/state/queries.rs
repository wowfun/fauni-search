use crate::{
    api::{
        ApiError, QueryHistoryDeleteData, QueryHistoryDetailData, QueryHistoryListData,
        QueryHistorySummaryData,
    },
    model::{QueryHistoryDraft, QueryHistoryRecord},
    query_assets::remove_temp_query_asset_file,
    state::AppState,
};
use serde_json::Value;
use std::path::Path as FsPath;

const DEFAULT_QUERY_HISTORY_LIMIT: usize = 50;
const MAX_QUERY_HISTORY_LIMIT: usize = 200;
const QUERY_HISTORY_RETENTION_LIMIT: usize = 1000;

impl AppState {
    pub(crate) fn list_query_history(
        &self,
        limit: Option<usize>,
        cursor: Option<&str>,
        query_kind: Option<&str>,
        source: Option<&str>,
        status: Option<&str>,
    ) -> Result<QueryHistoryListData, ApiError> {
        let limit = limit
            .unwrap_or(DEFAULT_QUERY_HISTORY_LIMIT)
            .clamp(1, MAX_QUERY_HISTORY_LIMIT);
        let offset = decode_query_history_cursor(cursor)?;
        let filtered = self
            .query_history_order
            .iter()
            .rev()
            .filter_map(|query_id| self.query_history.get(query_id))
            .filter(|record| {
                query_kind
                    .map(|value| record.query_kind == value)
                    .unwrap_or(true)
                    && source.map(|value| record.source == value).unwrap_or(true)
                    && status.map(|value| record.status == value).unwrap_or(true)
            })
            .collect::<Vec<_>>();
        let items = filtered
            .iter()
            .skip(offset)
            .take(limit)
            .map(|record| self.query_history_summary(record))
            .collect::<Vec<_>>();
        let next_offset = offset + items.len();
        let next_cursor =
            (next_offset < filtered.len()).then(|| encode_query_history_cursor(next_offset));
        Ok(QueryHistoryListData { items, next_cursor })
    }

    pub(crate) fn get_query_history(
        &self,
        query_id: &str,
    ) -> Result<QueryHistoryDetailData, ApiError> {
        let record = self
            .query_history
            .get(query_id)
            .ok_or_else(|| ApiError::not_found("Query history entry was not found."))?;
        Ok(QueryHistoryDetailData {
            summary: self.query_history_summary(record),
            input: record.input_json.clone(),
            search_scope: record.search_scope_json.clone(),
            filters: record.filters_json.clone(),
            target_content_types: record.target_content_types_json.clone(),
            top_k: record.top_k,
            error_code: record.error_code.clone(),
            error_message: record.error_message.clone(),
            duration_ms: record.duration_ms,
        })
    }

    pub(crate) fn delete_query_history(
        &mut self,
        query_id: &str,
    ) -> Result<QueryHistoryDeleteData, ApiError> {
        if !self.query_history.contains_key(query_id) {
            return Err(ApiError::not_found("Query history entry was not found."));
        }
        let before_history = self.query_history.clone();
        let before_order = self.query_history_order.clone();
        let before_assets = self.temp_query_assets.clone();

        let removed_record = self.query_history.remove(query_id);
        self.query_history_order.retain(|id| id != query_id);
        let removed_paths = self.remove_query_assets_referenced_by(removed_record.iter());
        let query_assets_deleted = removed_paths.len();
        if let Err(message) = self.persist_durable_state() {
            self.query_history = before_history;
            self.query_history_order = before_order;
            self.temp_query_assets = before_assets;
            return Err(ApiError::runtime_unavailable(
                format!("Query history could not be deleted: {message}"),
                None,
            ));
        }
        remove_query_asset_files(removed_paths);
        Ok(QueryHistoryDeleteData {
            deleted: 1,
            query_assets_deleted,
        })
    }

    pub(crate) fn clear_query_history(&mut self) -> Result<QueryHistoryDeleteData, ApiError> {
        let deleted = self.query_history.len();
        let before_history = self.query_history.clone();
        let before_order = self.query_history_order.clone();
        let before_assets = self.temp_query_assets.clone();

        let removed_records = self.query_history.values().cloned().collect::<Vec<_>>();
        self.query_history.clear();
        self.query_history_order.clear();
        let removed_paths = self.remove_query_assets_referenced_by(removed_records.iter());
        let query_assets_deleted = removed_paths.len();
        if let Err(message) = self.persist_durable_state() {
            self.query_history = before_history;
            self.query_history_order = before_order;
            self.temp_query_assets = before_assets;
            return Err(ApiError::runtime_unavailable(
                format!("Query history could not be cleared: {message}"),
                None,
            ));
        }
        remove_query_asset_files(removed_paths);
        Ok(QueryHistoryDeleteData {
            deleted,
            query_assets_deleted,
        })
    }

    pub(crate) fn record_query_history(
        &mut self,
        draft: QueryHistoryDraft,
    ) -> Result<(), ApiError> {
        let before_history = self.query_history.clone();
        let before_order = self.query_history_order.clone();
        let before_assets = self.temp_query_assets.clone();
        let query_id = self.next_query_id();
        let record = QueryHistoryRecord {
            id: query_id.clone(),
            created_at_ms: crate::state::current_unix_ms(),
            source: draft.source,
            query_kind: draft.query_kind,
            input_kind: draft.input_kind,
            input_summary: truncate_summary(&draft.input_summary),
            input_json: draft.input_json,
            search_scope_json: draft.search_scope_json,
            filters_json: draft.filters_json,
            target_content_types_json: draft.target_content_types_json,
            top_k: draft.top_k,
            status: draft.status,
            result_count: draft.result_count,
            error_code: draft.error_code,
            error_message: draft.error_message,
            duration_ms: draft.duration_ms,
        };
        self.query_history.insert(query_id.clone(), record);
        self.query_history_order.push(query_id);
        let removed_paths = self.enforce_query_history_retention();
        if let Err(message) = self.persist_durable_state() {
            self.query_history = before_history;
            self.query_history_order = before_order;
            self.temp_query_assets = before_assets;
            return Err(ApiError::runtime_unavailable(
                format!("Query history could not be recorded: {message}"),
                None,
            ));
        }
        remove_query_asset_files(removed_paths);
        Ok(())
    }

    fn query_history_summary(&self, record: &QueryHistoryRecord) -> QueryHistorySummaryData {
        QueryHistorySummaryData {
            query_id: record.id.clone(),
            created_at_ms: record.created_at_ms,
            source: record.source.clone(),
            query_kind: record.query_kind.clone(),
            input_kind: record.input_kind.clone(),
            input_summary: truncate_summary(&record.input_summary),
            scope_summary: scope_summary(&record.search_scope_json),
            status: record.status.clone(),
            result_count: record.result_count,
            input_available: self.query_history_input_available(record),
        }
    }

    fn query_history_input_available(&self, record: &QueryHistoryRecord) -> bool {
        match record.input_kind.as_str() {
            "inline_text" | "library_object" => true,
            "query_asset" => record
                .input_json
                .get("query_asset_id")
                .and_then(Value::as_str)
                .and_then(|asset_id| self.temp_query_assets.get(asset_id))
                .map(|asset| {
                    asset.expires_at_ms > crate::state::current_unix_ms()
                        && FsPath::new(&asset.path).exists()
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    fn enforce_query_history_retention(&mut self) -> Vec<String> {
        let mut removed_records = Vec::new();
        while self.query_history_order.len() > QUERY_HISTORY_RETENTION_LIMIT {
            if let Some(query_id) = self.query_history_order.first().cloned() {
                self.query_history_order.remove(0);
                if let Some(record) = self.query_history.remove(&query_id) {
                    removed_records.push(record);
                }
            }
        }
        self.remove_query_assets_referenced_by(removed_records.iter())
    }

    fn remove_query_assets_referenced_by<'a>(
        &mut self,
        records: impl IntoIterator<Item = &'a QueryHistoryRecord>,
    ) -> Vec<String> {
        let remaining_references = self
            .query_history
            .values()
            .filter_map(query_asset_id_from_history)
            .collect::<std::collections::BTreeSet<_>>();
        let remove_ids = records
            .into_iter()
            .filter_map(query_asset_id_from_history)
            .filter(|asset_id| !remaining_references.contains(asset_id))
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut removed_paths = Vec::new();
        for asset_id in &remove_ids {
            if let Some(asset) = self.temp_query_assets.remove(asset_id) {
                removed_paths.push(asset.path);
            }
        }
        removed_paths
    }
}

fn query_asset_id_from_history(record: &QueryHistoryRecord) -> Option<&str> {
    (record.input_kind == "query_asset")
        .then(|| {
            record
                .input_json
                .get("query_asset_id")
                .and_then(Value::as_str)
        })
        .flatten()
}

fn remove_query_asset_files(paths: Vec<String>) {
    for path in paths {
        remove_temp_query_asset_file(&path);
    }
}

fn encode_query_history_cursor(offset: usize) -> String {
    format!("queries:v1:{offset}")
}

fn decode_query_history_cursor(cursor: Option<&str>) -> Result<usize, ApiError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    cursor
        .strip_prefix("queries:v1:")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Query history cursor is invalid.",
                Some(serde_json::json!({ "field": "cursor" })),
            )
        })
}

fn truncate_summary(value: &str) -> String {
    const MAX_CHARS: usize = 160;
    let trimmed = value.trim();
    let mut summary = trimmed.chars().take(MAX_CHARS).collect::<String>();
    if trimmed.chars().count() > MAX_CHARS {
        summary.push('…');
    }
    summary
}

fn scope_summary(scope: &Value) -> String {
    match scope
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "library" => scope
            .get("library_id")
            .and_then(Value::as_str)
            .map(|library_id| format!("library:{library_id}"))
            .unwrap_or_else(|| "library".to_string()),
        "all_libraries" => "all_libraries".to_string(),
        "library_set" => "library_set".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StagedQueryAsset;
    use serde_json::json;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn deleting_history_only_removes_assets_referenced_by_deleted_history() {
        let mut state = AppState::default();
        let retained_path = temp_query_file("retained");
        let referenced_path = temp_query_file("referenced");

        let retained = state
            .register_temp_query_asset_record(None, staged_asset(retained_path.clone()))
            .expect("retained query asset should register");
        let referenced = state
            .register_temp_query_asset_record(None, staged_asset(referenced_path.clone()))
            .expect("referenced query asset should register");

        state
            .record_query_history(QueryHistoryDraft {
                source: "api".to_string(),
                query_kind: "image".to_string(),
                input_kind: "query_asset".to_string(),
                input_summary: referenced.id.clone(),
                input_json: json!({
                    "kind": "temp_asset",
                    "temp_asset_id": referenced.id.clone(),
                    "query_asset_id": referenced.id.clone(),
                }),
                search_scope_json: json!({ "kind": "all_libraries" }),
                filters_json: None,
                target_content_types_json: None,
                top_k: None,
                status: "completed".to_string(),
                result_count: Some(1),
                error_code: None,
                error_message: None,
                duration_ms: 1,
            })
            .expect("query history should record");
        let query_id = state.query_history_order[0].clone();

        let deleted = state
            .delete_query_history(&query_id)
            .expect("query history should delete");
        assert_eq!(deleted.deleted, 1);
        assert_eq!(deleted.query_assets_deleted, 1);
        assert!(!state.temp_query_assets.contains_key(&referenced.id));
        assert!(state.temp_query_assets.contains_key(&retained.id));
        assert!(!referenced_path.exists());
        assert!(retained_path.exists());

        let _ = fs::remove_file(retained_path);
    }

    fn staged_asset(path: std::path::PathBuf) -> StagedQueryAsset {
        StagedQueryAsset {
            path: path.to_string_lossy().to_string(),
            source_type: "image".to_string(),
            content_type: "image/png".to_string(),
            original_filename: Some("query.png".to_string()),
            page_count: None,
            duration_ms: None,
            size_bytes: 4,
        }
    }

    fn temp_query_file(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fauni-query-history-{name}-{unique}.png"));
        fs::write(&path, b"test").expect("temp query file should be writable");
        path
    }
}
