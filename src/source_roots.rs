use crate::{
    api::{ApiError, SourceRootRulesPayload},
    model::{
        LibraryRecord, ObservedSourceFile, SourceActionKind, SourceActionPlan, SourceRecord,
        SourceRootRecord, SourceRootScanResult,
    },
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path as FsPath,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn normalize_source_root_rules(rules: SourceRootRulesPayload) -> SourceRootRulesPayload {
    SourceRootRulesPayload {
        include_globs: normalize_rule_globs(rules.include_globs),
        exclude_globs: normalize_rule_globs(rules.exclude_globs),
        include_extensions: normalize_rule_extensions(rules.include_extensions),
    }
}

pub(crate) fn normalize_rule_globs(globs: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for glob in globs {
        let normalized = glob.trim().replace('\\', "/");
        let normalized = normalized.trim_start_matches("./").trim_matches('/');
        if !normalized.is_empty() {
            unique.insert(normalized.to_string());
        }
    }
    unique.into_iter().collect()
}

pub(crate) fn normalize_rule_extensions(extensions: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for extension in extensions {
        let normalized = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}

pub(crate) fn normalize_source_root_path(root_path: &str) -> Result<String, ApiError> {
    let trimmed = root_path.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation_failed(
            "Source root path must not be empty.",
            Some(json!({ "field": "root_path" })),
        ));
    }

    let path = FsPath::new(trimmed);
    if path.exists() {
        let metadata = fs::metadata(path).map_err(|error| {
            ApiError::validation_failed(
                format!("Source root metadata could not be read: {error}"),
                Some(json!({ "field": "root_path", "root_path": trimmed })),
            )
        })?;
        if !metadata.is_dir() {
            return Err(ApiError::validation_failed(
                "Current 140-library-source-management implementation only accepts local directory source roots.",
                Some(json!({ "field": "root_path", "root_path": trimmed })),
            ));
        }
        return Ok(fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string());
    }

    Ok(trimmed.to_string())
}

pub(crate) fn source_root_status_from_scan(enabled: bool, scan: &SourceRootScanResult) -> String {
    if !enabled {
        "disabled".to_string()
    } else if scan.status == "degraded" {
        "degraded".to_string()
    } else {
        "ready".to_string()
    }
}

pub(crate) fn source_root_watch_state(
    enabled: bool,
    scan: &SourceRootScanResult,
    queued: bool,
) -> String {
    if !enabled {
        "disabled".to_string()
    } else if queued {
        "queued_refresh".to_string()
    } else if scan.status == "degraded" {
        "error".to_string()
    } else {
        "watching".to_string()
    }
}

pub(crate) fn queued_watch_state_for_action(action: SourceActionKind) -> &'static str {
    match action {
        SourceActionKind::Refresh => "queued_refresh",
        SourceActionKind::Rescan => "queued_rescan",
    }
}

pub(crate) fn running_watch_state_for_action(action: SourceActionKind) -> &'static str {
    match action {
        SourceActionKind::Refresh => "refreshing",
        SourceActionKind::Rescan => "rescanning",
    }
}

pub(crate) fn source_root_action_in_flight(root: &SourceRootRecord) -> bool {
    matches!(
        root.watch_state.as_str(),
        "queued_refresh" | "queued_rescan" | "refreshing" | "rescanning"
    )
}

pub(crate) fn count_sources_for_root(
    library: &LibraryRecord,
    source_root_id: &str,
) -> (usize, usize) {
    library
        .sources
        .values()
        .filter(|source| source.source_root_id.as_deref() == Some(source_root_id))
        .fold((0usize, 0usize), |(active, inactive), source| {
            if source.status == "active" {
                (active + 1, inactive)
            } else {
                (active, inactive + 1)
            }
        })
}

pub(crate) fn mark_source_root_sources_state(
    library: &mut LibraryRecord,
    source_root_id: &str,
    status: &str,
    reason: Option<String>,
) {
    let affected_source_ids = library
        .source_order
        .iter()
        .filter_map(|source_id| {
            library
                .sources
                .get(source_id)
                .filter(|source| source.source_root_id.as_deref() == Some(source_root_id))
                .map(|source| source.id.clone())
        })
        .collect::<Vec<_>>();
    let mut removed_visual_unit_ids = BTreeSet::new();

    for source_id in affected_source_ids {
        if let Some(source) = library.sources.get_mut(&source_id) {
            removed_visual_unit_ids.extend(source.visual_unit_ids.iter().cloned());
            source.status = status.to_string();
            source.status_reason = reason.clone();
            source.visual_unit_ids.clear();
            source.observed_size_bytes = None;
            source.observed_modified_at_ms = None;
        }
    }

    if !removed_visual_unit_ids.is_empty() {
        library
            .visual_unit_order
            .retain(|visual_unit_id| !removed_visual_unit_ids.contains(visual_unit_id));
        for visual_unit_id in removed_visual_unit_ids {
            library.visual_units.remove(&visual_unit_id);
        }
    }
}

pub(crate) fn diff_observed_entries(
    previous: &BTreeMap<String, ObservedSourceFile>,
    current: &BTreeMap<String, ObservedSourceFile>,
) -> BTreeSet<String> {
    let mut changed = BTreeSet::new();
    for relative_path in previous.keys().chain(current.keys()) {
        let before = previous.get(relative_path);
        let after = current.get(relative_path);
        if before.map(observed_signature) != after.map(observed_signature) {
            changed.insert(relative_path.clone());
        }
    }
    changed
}

pub(crate) fn observed_signature(entry: &ObservedSourceFile) -> (u64, Option<u128>) {
    (entry.size_bytes, entry.modified_at_ms)
}

pub(crate) fn count_matched_observed_entries(
    observed_entries: &BTreeMap<String, ObservedSourceFile>,
    rules: &SourceRootRulesPayload,
) -> usize {
    observed_entries
        .values()
        .filter(|entry| observed_entry_is_in_scope(entry, rules))
        .count()
}

pub(crate) fn observed_entry_is_in_scope(
    entry: &ObservedSourceFile,
    rules: &SourceRootRulesPayload,
) -> bool {
    source_root_rules_allow_path(&entry.relative_path, rules)
        && observed_entry_extension_allowed(entry, rules)
}

pub(crate) fn observed_entry_extension_allowed(
    entry: &ObservedSourceFile,
    rules: &SourceRootRulesPayload,
) -> bool {
    let extension = FsPath::new(&entry.absolute_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(extension) = extension else {
        return false;
    };
    if !is_supported_source_extension(&extension) {
        return false;
    }
    rules.include_extensions.is_empty() || rules.include_extensions.contains(&extension)
}

pub(crate) fn source_root_rules_allow_path(
    relative_path: &str,
    rules: &SourceRootRulesPayload,
) -> bool {
    let normalized_path = relative_path.replace('\\', "/");
    let included = rules.include_globs.is_empty()
        || rules
            .include_globs
            .iter()
            .any(|pattern| glob_pattern_matches(pattern, &normalized_path));
    if !included {
        return false;
    }

    !rules
        .exclude_globs
        .iter()
        .any(|pattern| glob_pattern_matches(pattern, &normalized_path))
}

pub(crate) fn glob_pattern_matches(pattern: &str, relative_path: &str) -> bool {
    let pattern_segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let path_segments = relative_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    glob_segments_match(&pattern_segments, &path_segments)
}

pub(crate) fn glob_segments_match(pattern_segments: &[&str], path_segments: &[&str]) -> bool {
    if pattern_segments.is_empty() {
        return path_segments.is_empty();
    }
    if pattern_segments[0] == "**" {
        return glob_segments_match(&pattern_segments[1..], path_segments)
            || (!path_segments.is_empty()
                && glob_segments_match(pattern_segments, &path_segments[1..]));
    }
    !path_segments.is_empty()
        && wildcard_segment_matches(pattern_segments[0], path_segments[0])
        && glob_segments_match(&pattern_segments[1..], &path_segments[1..])
}

pub(crate) fn wildcard_segment_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut dp = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for pattern_index in 0..pattern.len() {
        match pattern[pattern_index] {
            '*' => {
                for value_index in 0..=value.len() {
                    if dp[pattern_index][value_index] {
                        dp[pattern_index + 1][value_index] = true;
                        if value_index < value.len() {
                            dp[pattern_index][value_index + 1] = true;
                        }
                    }
                }
            }
            '?' => {
                for value_index in 0..value.len() {
                    if dp[pattern_index][value_index] {
                        dp[pattern_index + 1][value_index + 1] = true;
                    }
                }
            }
            expected => {
                for value_index in 0..value.len() {
                    if dp[pattern_index][value_index] && value[value_index] == expected {
                        dp[pattern_index + 1][value_index + 1] = true;
                    }
                }
            }
        }
    }

    dp[pattern.len()][value.len()]
}

pub(crate) fn planned_source_action_paths(
    plan: &SourceActionPlan,
    root: &SourceRootRecord,
    candidate_by_relative_path: &BTreeMap<String, ObservedSourceFile>,
    existing_by_relative_path: &BTreeMap<String, SourceRecord>,
) -> BTreeSet<String> {
    if plan.action.is_rescan() {
        return candidate_by_relative_path
            .keys()
            .chain(existing_by_relative_path.keys())
            .cloned()
            .collect();
    }

    if let Some(paths) = plan.changed_paths_by_root.get(&root.id) {
        return paths.clone();
    }

    let mut affected = BTreeSet::new();
    for (relative_path, entry) in candidate_by_relative_path {
        let current_source = existing_by_relative_path.get(relative_path);
        let unchanged = current_source
            .map(|source| {
                source.status == "active"
                    && source.observed_size_bytes == Some(entry.size_bytes)
                    && source.observed_modified_at_ms == entry.modified_at_ms
            })
            .unwrap_or(false);
        if !unchanged {
            affected.insert(relative_path.clone());
        }
    }

    for relative_path in existing_by_relative_path.keys() {
        if !candidate_by_relative_path.contains_key(relative_path) {
            affected.insert(relative_path.clone());
        }
    }

    affected
}

pub(crate) fn invalidated_source_record(
    mut source: SourceRecord,
    status: &str,
    reason: Option<String>,
    observed_size_bytes: Option<u64>,
    observed_modified_at_ms: Option<u128>,
) -> SourceRecord {
    source.status = status.to_string();
    source.status_reason = reason;
    source.visual_unit_ids.clear();
    source.observed_size_bytes = observed_size_bytes;
    source.observed_modified_at_ms = observed_modified_at_ms;
    source
}

pub(crate) fn out_of_scope_status_reason(
    observed: &ObservedSourceFile,
    rules: &SourceRootRulesPayload,
) -> (String, Option<String>) {
    if !source_root_rules_allow_path(&observed.relative_path, rules) {
        return (
            "out_of_scope".to_string(),
            Some("rule_excluded".to_string()),
        );
    }

    let extension = FsPath::new(&observed.absolute_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(extension) = extension else {
        return (
            "out_of_scope".to_string(),
            Some("unsupported_type".to_string()),
        );
    };
    if !is_supported_source_extension(&extension) {
        return (
            "out_of_scope".to_string(),
            Some("unsupported_type".to_string()),
        );
    }
    if !rules.include_extensions.is_empty() && !rules.include_extensions.contains(&extension) {
        return (
            "out_of_scope".to_string(),
            Some("extension_filtered".to_string()),
        );
    }

    (
        "out_of_scope".to_string(),
        Some("outside_coverage".to_string()),
    )
}

pub(crate) fn is_supported_source_extension(extension: &str) -> bool {
    matches!(
        extension,
        "pdf" | "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "mp4" | "mov" | "m4v"
    )
}

pub(crate) fn scan_source_root_directory(root_path: &str) -> SourceRootScanResult {
    let trimmed = root_path.trim();
    if trimmed.is_empty() {
        return SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some("Source root path must not be empty.".to_string()),
        };
    }

    let root = FsPath::new(trimmed);
    let metadata = match fs::metadata(root) {
        Ok(metadata) => metadata,
        Err(error) => {
            return SourceRootScanResult {
                status: "degraded".to_string(),
                observed_entries: BTreeMap::new(),
                error: Some(format!("Source root metadata could not be read: {error}")),
            };
        }
    };
    if !metadata.is_dir() {
        return SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some("Source root path is not a directory.".to_string()),
        };
    }

    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut observed_entries = BTreeMap::new();
    match collect_source_root_files(&canonical_root, &canonical_root, &mut observed_entries) {
        Ok(()) => SourceRootScanResult {
            status: "ready".to_string(),
            observed_entries,
            error: None,
        },
        Err(error) => SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some(error),
        },
    }
}

pub(crate) fn collect_source_root_files(
    root: &FsPath,
    current: &FsPath,
    observed_entries: &mut BTreeMap<String, ObservedSourceFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("Source root directory could not be read: {error}"))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Source root entry could not be read: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Source root entry type could not be read: {error}"))?;

        if file_type.is_dir() {
            collect_source_root_files(root, &path, observed_entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let metadata = entry
            .metadata()
            .map_err(|error| format!("Source root file metadata could not be read: {error}"))?;
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        observed_entries.insert(
            relative_path.clone(),
            ObservedSourceFile {
                absolute_path: fs::canonicalize(&path)
                    .unwrap_or(path.clone())
                    .to_string_lossy()
                    .to_string(),
                relative_path,
                size_bytes: metadata.len(),
                modified_at_ms: metadata.modified().ok().and_then(system_time_to_unix_ms),
            },
        );
    }

    Ok(())
}

pub(crate) fn system_time_to_unix_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

pub(crate) fn read_string_filter(filters: Option<&Value>, key: &str) -> Option<BTreeSet<String>> {
    let value = filters?.get(key)?;
    match value {
        Value::String(item) => Some(BTreeSet::from([item.clone()])),
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(|item| item.as_str().map(|text| text.to_string()))
                .collect::<BTreeSet<_>>();
            (!values.is_empty()).then_some(values)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::glob_pattern_matches;

    #[test]
    fn glob_pattern_matches_double_star_and_wildcards() {
        assert!(glob_pattern_matches(
            "reports/**/*.pdf",
            "reports/2025/q2/report.pdf"
        ));
        assert!(glob_pattern_matches("*.png", "chart.png"));
        assert!(!glob_pattern_matches("*.png", "chart.jpg"));
    }
}
