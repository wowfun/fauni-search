use super::*;
use crate::*;
use serde_json::Value;
use std::{fs, path::Path as FsPath};

impl AppState {
    pub(crate) fn list_libraries(&self) -> LibrariesListData {
        let libraries = self
            .library_order
            .iter()
            .filter_map(|id| self.libraries.get(id))
            .map(|record| self.library_snapshot(record))
            .collect();

        LibrariesListData { libraries }
    }

    pub(crate) fn get_library(&self, library_id: &str) -> Result<LibrarySnapshot, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        Ok(self.library_snapshot(library))
    }

    pub(crate) fn list_source_roots(
        &self,
        library_id: &str,
    ) -> Result<SourceRootsListData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let source_roots = library
            .source_root_order
            .iter()
            .filter_map(|source_root_id| library.source_roots.get(source_root_id))
            .map(Self::source_root_snapshot)
            .collect();

        Ok(SourceRootsListData { source_roots })
    }

    pub(crate) fn get_source_root(
        &self,
        library_id: &str,
        source_root_id: &str,
    ) -> Result<SourceRootDetailData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let source_root = library
            .source_roots
            .get(source_root_id)
            .ok_or_else(|| ApiError::not_found("Source root was not found."))?;

        Ok(SourceRootDetailData {
            source_root: Self::source_root_snapshot(source_root),
        })
    }

    pub(crate) fn create_source_root(
        &mut self,
        library_id: &str,
        request: CreateSourceRootRequest,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            if !state.libraries.contains_key(library_id) {
                return Err(ApiError::not_found("Library was not found."));
            }

            let root_path = normalize_source_root_path(&request.root_path)?;
            let rules = normalize_source_root_rules(request.rules.unwrap_or_default());
            let enabled = request.enabled.unwrap_or(true);
            let root_id = state.next_source_root_id();
            let scan = if enabled {
                scan_source_root_directory(&root_path)
            } else {
                SourceRootScanResult::disabled()
            };
            let coverage_summary = SourceRootCoverageSummary {
                observed_file_count: scan.observed_entries.len(),
                matched_file_count: count_matched_observed_entries(&scan.observed_entries, &rules),
                active_source_count: 0,
                inactive_source_count: 0,
                last_scan_at_ms: None,
            };
            let record = SourceRootRecord {
                id: root_id.clone(),
                root_path,
                enabled,
                status: source_root_status_from_scan(enabled, &scan),
                watch_state: source_root_watch_state(enabled, &scan, false),
                rules,
                coverage_summary,
                observed_entries: scan.observed_entries,
                pending_watch_paths: BTreeSet::new(),
                pending_watch_deadline_ms: None,
                pending_watch_error: scan.error,
                last_action: None,
            };

            let snapshot = Self::source_root_snapshot(&record);
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            library.source_root_order.push(root_id.clone());
            library.source_roots.insert(root_id, record);
            Ok(snapshot)
        })
    }

    pub(crate) fn update_source_root(
        &mut self,
        library_id: &str,
        source_root_id: &str,
        request: UpdateSourceRootRequest,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            if !library.source_roots.contains_key(source_root_id) {
                return Err(ApiError::not_found("Source root was not found."));
            }

            if let Some(false) = request.enabled {
                mark_source_root_sources_state(
                    library,
                    source_root_id,
                    "out_of_scope",
                    Some("source_root_disabled".to_string()),
                );
            }

            {
                let root = library
                    .source_roots
                    .get_mut(source_root_id)
                    .ok_or_else(|| ApiError::not_found("Source root was not found."))?;

                if let Some(root_path) = request.root_path.as_ref() {
                    root.root_path = normalize_source_root_path(root_path)?;
                    root.pending_watch_paths.clear();
                    root.pending_watch_deadline_ms = None;
                }
                if let Some(enabled) = request.enabled {
                    root.enabled = enabled;
                }
                if let Some(rules) = request.rules {
                    root.rules = normalize_source_root_rules(rules);
                }

                let scan = if root.enabled {
                    scan_source_root_directory(&root.root_path)
                } else {
                    SourceRootScanResult::disabled()
                };
                root.status = source_root_status_from_scan(root.enabled, &scan);
                root.watch_state = source_root_watch_state(root.enabled, &scan, false);
                root.pending_watch_error = scan.error.clone();
                root.observed_entries = scan.observed_entries;
                root.coverage_summary.observed_file_count = root.observed_entries.len();
                root.coverage_summary.matched_file_count =
                    count_matched_observed_entries(&root.observed_entries, &root.rules);
            }

            let (active_source_count, inactive_source_count) =
                count_sources_for_root(library, source_root_id);
            let root = library
                .source_roots
                .get_mut(source_root_id)
                .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
            root.coverage_summary.active_source_count = active_source_count;
            root.coverage_summary.inactive_source_count = inactive_source_count;

            Ok(Self::source_root_snapshot(root))
        })
    }

    pub(crate) fn delete_source_root(
        &mut self,
        library_id: &str,
        source_root_id: &str,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            if !library.source_roots.contains_key(source_root_id) {
                return Err(ApiError::not_found("Source root was not found."));
            }

            mark_source_root_sources_state(
                library,
                source_root_id,
                "out_of_scope",
                Some("source_root_deleted".to_string()),
            );

            let root = library
                .source_roots
                .remove(source_root_id)
                .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
            library
                .source_root_order
                .retain(|candidate| candidate != source_root_id);
            Ok(Self::source_root_snapshot(&root))
        })
    }

    pub(crate) fn list_sources(
        &self,
        library_id: &str,
        query: SourcesQuery,
    ) -> Result<SourcesListData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let sources = library
            .source_order
            .iter()
            .filter_map(|source_id| library.sources.get(source_id))
            .filter(|source| {
                query
                    .source_root_id
                    .as_ref()
                    .map(|expected| source.source_root_id.as_ref() == Some(expected))
                    .unwrap_or(true)
                    && query
                        .source_type
                        .as_ref()
                        .map(|expected| &source.source_type == expected)
                        .unwrap_or(true)
                    && query
                        .status
                        .as_ref()
                        .map(|expected| &source.status == expected)
                        .unwrap_or(true)
            })
            .map(|source| Self::source_inventory_item(library, source))
            .collect();

        Ok(SourcesListData { sources })
    }

    pub(crate) fn queue_source_action(
        &mut self,
        library_id: &str,
        scope: SourceActionScope,
        action: SourceActionKind,
        trigger: SourceActionTrigger,
        changed_paths_by_root: BTreeMap<String, BTreeSet<String>>,
    ) -> Result<(SourceActionData, Option<QueuedSourceAction>), ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut accepted_root_ids = Vec::new();

        match &scope {
            SourceActionScope::Library => {
                for source_root_id in &library.source_root_order {
                    let root = library
                        .source_roots
                        .get(source_root_id)
                        .expect("source_root_order should reference a valid source root");
                    if !root.enabled {
                        rejected.push(SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "not_enabled".to_string(),
                            message: "Source root is disabled.".to_string(),
                        });
                        continue;
                    }
                    if source_root_action_in_flight(root) {
                        rejected.push(SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "job_in_progress".to_string(),
                            message:
                                "Source root already has an in-flight source-management action."
                                    .to_string(),
                        });
                        continue;
                    }
                    accepted_root_ids.push(root.id.clone());
                    accepted.push(SourceActionAcceptedItem {
                        source_root_id: root.id.clone(),
                        root_path: root.root_path.clone(),
                        action: action.as_str().to_string(),
                    });
                }
            }
            SourceActionScope::SourceRoot(source_root_id) => {
                let root = library
                    .source_roots
                    .get(source_root_id)
                    .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
                if !root.enabled {
                    return Ok((
                        SourceActionData {
                            accepted,
                            rejected: vec![SourceActionRejectedItem {
                                source_root_id: Some(root.id.clone()),
                                root_path: Some(root.root_path.clone()),
                                reason_code: "not_enabled".to_string(),
                                message: "Source root is disabled.".to_string(),
                            }],
                            job_handle: None,
                            job: None,
                        },
                        None,
                    ));
                }
                if source_root_action_in_flight(root) {
                    return Ok((
                        SourceActionData {
                            accepted,
                            rejected: vec![SourceActionRejectedItem {
                                source_root_id: Some(root.id.clone()),
                                root_path: Some(root.root_path.clone()),
                                reason_code: "job_in_progress".to_string(),
                                message:
                                    "Source root already has an in-flight source-management action."
                                        .to_string(),
                            }],
                            job_handle: None,
                            job: None,
                        },
                        None,
                    ));
                }
                accepted_root_ids.push(root.id.clone());
                accepted.push(SourceActionAcceptedItem {
                    source_root_id: root.id.clone(),
                    root_path: root.root_path.clone(),
                    action: action.as_str().to_string(),
                });
            }
        }

        if accepted_root_ids.is_empty() {
            return Ok((
                SourceActionData {
                    accepted,
                    rejected,
                    job_handle: None,
                    job: None,
                },
                None,
            ));
        }

        let job_id = self.next_job_id();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            library_id: library_id.to_string(),
            kind: action.as_str().to_string(),
            status: "queued".to_string(),
            phase: "intake".to_string(),
            progress: JobProgress {
                completed: 0,
                total: accepted_root_ids.len(),
                unit: "source_root".to_string(),
            },
            cancelable: false,
            current_attempt: JobAttemptSnapshot {
                attempt: 1,
                status: "queued".to_string(),
                summary: format!(
                    "Queued {} across {} source root(s) via {} trigger.",
                    action.as_str(),
                    accepted_root_ids.len(),
                    trigger.as_str(),
                ),
            },
        };

        let plan = SourceActionPlan {
            library_id: library_id.to_string(),
            action,
            target_root_ids: accepted_root_ids.clone(),
            changed_paths_by_root,
        };

        let library = self
            .libraries
            .get_mut(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        for source_root_id in &accepted_root_ids {
            if let Some(root) = library.source_roots.get_mut(source_root_id) {
                root.watch_state = queued_watch_state_for_action(action).to_string();
            }
        }
        library.latest_job_id = Some(job_id.clone());

        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                snapshot: snapshot.clone(),
            },
        );
        self.job_order.push(job_id.clone());

        Ok((
            SourceActionData {
                accepted,
                rejected,
                job_handle: Some(job_id.clone()),
                job: Some(snapshot),
            },
            Some(QueuedSourceAction { job_id, plan }),
        ))
    }

    pub(crate) fn poll_source_root_watchers(&mut self) -> Vec<QueuedSourceAction> {
        let now = current_unix_ms();
        let library_ids = self.library_order.clone();
        let mut due_actions = Vec::new();

        for library_id in &library_ids {
            let Some(library) = self.libraries.get_mut(library_id) else {
                continue;
            };
            let source_root_ids = library.source_root_order.clone();

            for source_root_id in source_root_ids {
                let Some(root) = library.source_roots.get_mut(&source_root_id) else {
                    continue;
                };
                if !root.enabled {
                    root.status = "disabled".to_string();
                    root.watch_state = "disabled".to_string();
                    continue;
                }
                let watcher_refresh_pending = root.watch_state == "queued_refresh"
                    && root.pending_watch_deadline_ms.is_some()
                    && !root.pending_watch_paths.is_empty();
                if source_root_action_in_flight(root) && !watcher_refresh_pending {
                    continue;
                }

                let scan = scan_source_root_directory(&root.root_path);
                let changed_paths =
                    diff_observed_entries(&root.observed_entries, &scan.observed_entries);
                if !changed_paths.is_empty() {
                    root.pending_watch_paths.extend(changed_paths);
                    if root.pending_watch_deadline_ms.is_none() {
                        root.pending_watch_deadline_ms =
                            Some(now.saturating_add(SOURCE_WATCHER_DEBOUNCE_MS));
                    }
                }
                root.observed_entries = scan.observed_entries.clone();
                root.pending_watch_error = scan.error.clone();
                root.status = source_root_status_from_scan(true, &scan);
                root.watch_state = if root.pending_watch_paths.is_empty() {
                    source_root_watch_state(true, &scan, false)
                } else {
                    "queued_refresh".to_string()
                };

                if root
                    .pending_watch_deadline_ms
                    .map(|deadline| now >= deadline)
                    .unwrap_or(false)
                    && !root.pending_watch_paths.is_empty()
                {
                    root.watch_state = source_root_watch_state(true, &scan, false);
                    due_actions.push((
                        library_id.clone(),
                        source_root_id.clone(),
                        std::mem::take(&mut root.pending_watch_paths),
                    ));
                    root.pending_watch_deadline_ms = None;
                }
            }
        }

        let mut queued = Vec::new();
        for (library_id, source_root_id, changed_paths) in due_actions {
            let mut changed_paths_by_root = BTreeMap::new();
            changed_paths_by_root.insert(source_root_id.clone(), changed_paths);
            if let Ok((_, Some(queued_action))) = self.queue_source_action(
                &library_id,
                SourceActionScope::SourceRoot(source_root_id),
                SourceActionKind::Refresh,
                SourceActionTrigger::Watcher,
                changed_paths_by_root,
            ) {
                queued.push(queued_action);
            }
        }

        queued
    }

    pub(crate) fn list_video_sources(
        &self,
        library_id: &str,
    ) -> Result<VideoSourcesData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let sources = library
            .source_order
            .iter()
            .filter_map(|source_id| library.sources.get(source_id))
            .filter(|source| source.source_type == "video" && source.status == "active")
            .map(|source| {
                Ok(VideoSourceSummary {
                    source_id: source.id.clone(),
                    source_path: source.source_path.clone(),
                    source_type: source.source_type.clone(),
                    duration_ms: source.duration_ms,
                    preview: video_source_preview_reference(library_id, &source.id)?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        Ok(VideoSourcesData { sources })
    }

    pub(crate) fn create_library(
        &mut self,
        request: CreateLibraryRequest,
    ) -> Result<LibrarySnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let name = request.name.trim();
            if name.is_empty() {
                return Err(ApiError::validation_failed(
                    "Library name must not be empty.",
                    Some(json!({ "field": "name" })),
                ));
            }

            let enabled_index_lines =
                normalize_index_lines(request.config.map(|config| config.enabled_index_lines));

            if enabled_index_lines != [MULTIVECTOR_INDEX_LINE.to_string()] {
                return Err(ApiError::validation_failed(
                    "Current 100-text-search implementation requires config.enabled_index_lines to be exactly [\"multivector\"].",
                    Some(json!({
                        "field": "config.enabled_index_lines",
                        "expected": [MULTIVECTOR_INDEX_LINE],
                        "received": enabled_index_lines,
                    })),
                ));
            }

            let library_id = state.next_library_id();
            let record = LibraryRecord {
                id: library_id.clone(),
                name: name.to_string(),
                collection_name: stable_collection_name(&library_id, MULTIVECTOR_INDEX_LINE),
                config: LibraryConfigPayload {
                    enabled_index_lines,
                },
                model_overrides: default_library_model_overrides(),
                source_roots: BTreeMap::new(),
                source_root_order: Vec::new(),
                sources: BTreeMap::new(),
                source_order: Vec::new(),
                visual_units: BTreeMap::new(),
                visual_unit_order: Vec::new(),
                latest_job_id: None,
                active_index_lines: BTreeSet::new(),
            };

            let snapshot = state.library_snapshot(&record);
            state.library_order.push(library_id.clone());
            state.libraries.insert(library_id, record);
            Ok(snapshot)
        })
    }

    pub(crate) fn prepare_import(
        &mut self,
        library_id: &str,
        request: ImportPathsRequest,
    ) -> Result<PreparedImport, ApiError> {
        let (collection_name, had_existing_visual_units) = self
            .libraries
            .get(library_id)
            .map(|library| {
                (
                    library.collection_name.clone(),
                    !library.visual_units.is_empty(),
                )
            })
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut new_sources = Vec::new();
        let mut new_visual_units = Vec::new();

        for original in request.paths {
            match self.inspect_import_path(&original) {
                Ok(classification) => {
                    let visual_units = self.new_visual_units_from_classification(&classification);
                    let source = self.source_record_from_classification(
                        &classification,
                        visual_units.iter().map(|item| item.id.clone()).collect(),
                    );
                    let visual_unit_summaries =
                        visual_units.iter().map(VisualUnitRecord::summary).collect();
                    new_sources.push(source);
                    new_visual_units.extend(visual_units);

                    accepted.push(ImportAcceptedItem {
                        original_path: original,
                        normalized_path: Some(classification.normalized_path),
                        reason_code: "accepted".to_string(),
                        message: format!(
                            "Accepted as {} input for the pending multivector index.",
                            classification.source_type
                        ),
                        source_id: Some(classification.source_id),
                        source_type: classification.source_type,
                        kind: classification.kind,
                        visual_units: visual_unit_summaries,
                    });
                }
                Err(rejection) => rejected.push(ImportRejectedItem {
                    original_path: original,
                    normalized_path: rejection.normalized_path,
                    reason_code: rejection.reason_code,
                    message: rejection.message,
                }),
            }
        }

        Ok(PreparedImport {
            library_id: library_id.to_string(),
            collection_name,
            had_existing_visual_units,
            accepted,
            rejected,
            sources: new_sources,
            visual_units: new_visual_units,
        })
    }

    pub(crate) fn source_root_snapshot(root: &SourceRootRecord) -> SourceRootSnapshot {
        SourceRootSnapshot {
            source_root_id: root.id.clone(),
            root_path: root.root_path.clone(),
            enabled: root.enabled,
            status: root.status.clone(),
            watch_state: root.watch_state.clone(),
            coverage_summary: root.coverage_summary.clone(),
            rules: root.rules.clone(),
            last_action: root.last_action.clone(),
        }
    }

    pub(crate) fn source_inventory_item(
        library: &LibraryRecord,
        source: &SourceRecord,
    ) -> SourceInventoryItem {
        let source_root_path = source
            .source_root_id
            .as_ref()
            .and_then(|source_root_id| {
                library
                    .source_roots
                    .get(source_root_id)
                    .map(|root| root.root_path.clone())
            })
            .or_else(|| source.source_root_path.clone());
        let source_root_label = match (&source.source_root_id, &source_root_path) {
            (Some(_), Some(root_path)) => root_path.clone(),
            (Some(source_root_id), None) => format!("deleted:{source_root_id}"),
            (None, _) => "manual import".to_string(),
        };

        SourceInventoryItem {
            source_id: source.id.clone(),
            source_path: source.source_path.clone(),
            source_type: source.source_type.clone(),
            kind: source.kind.clone(),
            status: source.status.clone(),
            status_reason: source.status_reason.clone(),
            relative_path: source.relative_path.clone(),
            source_root_id: source.source_root_id.clone(),
            source_root_path,
            source_root_label,
            visual_unit_count: source.visual_unit_ids.len(),
        }
    }

    pub(crate) fn inspect_import_path(
        &mut self,
        original_path: &str,
    ) -> Result<PathClassification, ImportRejection> {
        let trimmed = original_path.trim();
        if trimmed.is_empty() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "empty_path".to_string(),
                message: "Path must not be empty.".to_string(),
            });
        }

        let path = FsPath::new(trimmed);
        if !path.exists() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "not_found".to_string(),
                message: "Path does not exist.".to_string(),
            });
        }

        let metadata = fs::metadata(path).map_err(|_| ImportRejection {
            normalized_path: None,
            reason_code: "not_readable".to_string(),
            message: "Path metadata could not be read.".to_string(),
        })?;

        if !metadata.is_file() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "not_file".to_string(),
                message: "Only file paths are accepted in the current implementation.".to_string(),
            });
        }

        let normalized_path = fs::canonicalize(path)
            .map(|resolved| resolved.to_string_lossy().to_string())
            .unwrap_or_else(|_| trimmed.to_string());

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        match extension.as_deref() {
            Some("pdf") => {
                let page_count = pdf_page_count(path).map_err(|message| ImportRejection {
                    normalized_path: Some(normalized_path.clone()),
                    reason_code: "invalid_pdf".to_string(),
                    message,
                })?;
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "pdf".to_string(),
                    kind: "document_page".to_string(),
                    page_count: Some(page_count),
                    duration_ms: None,
                })
            }
            Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("bmp") | Some("gif") => {
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    page_count: None,
                    duration_ms: None,
                })
            }
            Some("mp4") | Some("mov") | Some("m4v") => {
                let duration_ms = video_duration_ms(path).map_err(|message| ImportRejection {
                    normalized_path: Some(normalized_path.clone()),
                    reason_code: "invalid_video".to_string(),
                    message,
                })?;
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    page_count: None,
                    duration_ms: Some(duration_ms),
                })
            }
            _ => Err(ImportRejection {
                normalized_path: Some(normalized_path),
                reason_code: "unsupported_type".to_string(),
                message:
                    "Only PDF, common image files, and mp4/mov video files are accepted right now."
                        .to_string(),
            }),
        }
    }

    pub(crate) fn library_snapshot(&self, library: &LibraryRecord) -> LibrarySnapshot {
        let pending_jobs = self
            .jobs
            .values()
            .filter(|job| {
                job.snapshot.library_id == library.id
                    && !matches!(
                        job.snapshot.status.as_str(),
                        "completed" | "failed" | "canceled"
                    )
            })
            .count();

        let index_lines = library
            .config
            .enabled_index_lines
            .iter()
            .map(|index_line| LibraryIndexLineStatus {
                index_line: index_line.clone(),
                status: if library.active_index_lines.contains(index_line) {
                    "ready".to_string()
                } else {
                    "not_ready".to_string()
                },
            })
            .collect();

        LibrarySnapshot {
            id: library.id.clone(),
            name: library.name.clone(),
            config: library.config.clone(),
            index_lines,
            counts: LibraryCounts {
                accepted_items: library
                    .sources
                    .values()
                    .filter(|source| source.status == "active")
                    .map(|source| source.visual_unit_ids.len())
                    .sum(),
                pending_jobs,
            },
            latest_job_id: library.latest_job_id.clone(),
        }
    }

    pub(crate) fn source_record_from_classification(
        &self,
        classification: &PathClassification,
        visual_unit_ids: Vec<String>,
    ) -> SourceRecord {
        SourceRecord {
            id: classification.source_id.clone(),
            source_root_id: None,
            source_root_path: None,
            source_path: classification.normalized_path.clone(),
            relative_path: None,
            source_type: classification.source_type.clone(),
            kind: classification.kind.clone(),
            status: "active".to_string(),
            status_reason: None,
            page_count: classification.page_count,
            duration_ms: classification.duration_ms,
            observed_size_bytes: None,
            observed_modified_at_ms: None,
            visual_unit_ids,
        }
    }

    pub(crate) fn new_visual_units_from_classification(
        &mut self,
        classification: &PathClassification,
    ) -> Vec<VisualUnitRecord> {
        if classification.kind == "document_page" {
            let page_count = classification.page_count.unwrap_or(1);
            return (1..=page_count)
                .map(|page_number| {
                    self.new_visual_unit_record(
                        classification,
                        json!({
                            "page": page_number,
                            "page_label": page_number.to_string(),
                        }),
                        json!({
                            "previous_page": (page_number > 1).then_some(page_number - 1),
                            "current_page": page_number,
                            "next_page": (page_number < page_count).then_some(page_number + 1),
                            "total_pages": page_count,
                            "source_path": classification.normalized_path,
                            "source_type": classification.source_type,
                        }),
                    )
                })
                .collect();
        }

        if classification.kind == "video_segment" {
            let duration_ms = classification.duration_ms.unwrap_or(1);
            let segments = build_video_segment_ranges(duration_ms);
            return segments
                .iter()
                .enumerate()
                .map(|(segment_index, (start_ms, end_ms))| {
                    let previous = segment_index
                        .checked_sub(1)
                        .and_then(|index| segments.get(index))
                        .map(
                            |(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }),
                        );
                    let next = segments.get(segment_index + 1).map(
                        |(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }),
                    );
                    self.new_visual_unit_record(
                        classification,
                        json!({
                            "start_ms": start_ms,
                            "end_ms": end_ms,
                            "duration_ms": duration_ms,
                        }),
                        json!({
                            "previous_segment": previous,
                            "current_segment": {
                                "start_ms": start_ms,
                                "end_ms": end_ms,
                            },
                            "next_segment": next,
                            "total_segments": segments.len(),
                            "source_path": classification.normalized_path,
                            "source_type": classification.source_type,
                            "duration_ms": duration_ms,
                        }),
                    )
                })
                .collect();
        }

        vec![self.new_visual_unit_record(
            classification,
            json!({
                "path": classification.normalized_path,
            }),
            json!({
                "source_type": classification.source_type,
                "source_path": classification.normalized_path,
            }),
        )]
    }

    pub(crate) fn new_visual_unit_record(
        &mut self,
        classification: &PathClassification,
        locator: Value,
        neighbor_context: Value,
    ) -> VisualUnitRecord {
        let point_id = self.next_visual_unit_seq + 1;
        let visual_unit_id = self.next_visual_unit_id();

        VisualUnitRecord {
            id: visual_unit_id,
            point_id,
            source_id: classification.source_id.clone(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            kind: classification.kind.clone(),
            locator,
            neighbor_context,
        }
    }
}
