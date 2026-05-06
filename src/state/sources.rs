use super::*;
use crate::*;
use serde_json::Value;
use std::{fs, path::Path as FsPath};

pub(crate) struct DeletedLibraryCleanupPlan {
    pub(crate) snapshot: LibrarySnapshot,
    pub(crate) vector_space_ids: Vec<String>,
    pub(crate) temp_asset_paths: Vec<String>,
}

impl AppState {
    fn resolve_source_action_targets(
        &self,
        library_id: &str,
        scope: &SourceActionScope,
        action: SourceActionKind,
    ) -> Result<
        (
            Vec<SourceActionAcceptedItem>,
            Vec<SourceActionRejectedItem>,
            Vec<String>,
        ),
        ApiError,
    > {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut accepted_root_ids = Vec::new();

        match scope {
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
                        accepted,
                        vec![SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "not_enabled".to_string(),
                            message: "Source root is disabled.".to_string(),
                        }],
                        accepted_root_ids,
                    ));
                }
                if source_root_action_in_flight(root) {
                    return Ok((
                        accepted,
                        vec![SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "job_in_progress".to_string(),
                            message:
                                "Source root already has an in-flight source-management action."
                                    .to_string(),
                        }],
                        accepted_root_ids,
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

        Ok((accepted, rejected, accepted_root_ids))
    }

    pub(crate) fn plan_source_action_replay(
        &self,
        library_id: &str,
        scope: SourceActionScope,
        action: SourceActionKind,
    ) -> Result<SourceActionPlan, ApiError> {
        let (_, rejected, accepted_root_ids) =
            self.resolve_source_action_targets(library_id, &scope, action)?;
        if accepted_root_ids.is_empty() {
            let message = rejected
                .first()
                .map(|item| item.message.clone())
                .unwrap_or_else(|| {
                    "Source-action replay did not target any source roots.".to_string()
                });
            return Err(ApiError::conflict(
                &message,
                Some(json!({ "library_id": library_id, "action": action.as_str() })),
            ));
        }

        Ok(SourceActionPlan {
            library_id: library_id.to_string(),
            action,
            target_root_ids: accepted_root_ids,
            changed_paths_by_root: BTreeMap::new(),
        })
    }

    pub(crate) fn list_libraries(&self) -> LibrariesListData {
        let (mut active, archived): (Vec<_>, Vec<_>) = self
            .library_order
            .iter()
            .filter_map(|id| self.libraries.get(id))
            .map(|record| self.library_snapshot(record))
            .partition(|snapshot| snapshot.lifecycle_state != "archived");
        active.extend(archived);

        LibrariesListData { libraries: active }
    }

    pub(crate) fn get_library(&self, library_id: &str) -> Result<LibrarySnapshot, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        Ok(self.library_snapshot(library))
    }

    pub(crate) fn update_library(
        &mut self,
        library_id: &str,
        request: UpdateLibraryRequest,
    ) -> Result<LibrarySnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let display_name = request.display_name.trim().to_string();
            if display_name.is_empty() {
                return Err(ApiError::validation_failed(
                    "Library display_name must not be empty.",
                    Some(json!({ "field": "display_name" })),
                ));
            }

            {
                let library = state
                    .libraries
                    .get_mut(library_id)
                    .ok_or_else(|| ApiError::not_found("Library was not found."))?;
                library.display_name = display_name;
            }
            let library = state
                .libraries
                .get(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            Ok(state.library_snapshot(library))
        })
    }

    pub(crate) fn archive_library(
        &mut self,
        library_id: &str,
    ) -> Result<LibrarySnapshot, ApiError> {
        self.commit_durable_api(|state| {
            {
                let library = state
                    .libraries
                    .get_mut(library_id)
                    .ok_or_else(|| ApiError::not_found("Library was not found."))?;
                if library.lifecycle_state != "archived" {
                    library.lifecycle_state = "archived".to_string();
                    library.archived_at_ms = Some(current_unix_ms());
                }
            }
            let library = state
                .libraries
                .get(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            Ok(state.library_snapshot(library))
        })
    }

    pub(crate) fn restore_library(
        &mut self,
        library_id: &str,
    ) -> Result<LibrarySnapshot, ApiError> {
        self.commit_durable_api(|state| {
            {
                let library = state
                    .libraries
                    .get_mut(library_id)
                    .ok_or_else(|| ApiError::not_found("Library was not found."))?;
                if library.lifecycle_state != "active" {
                    library.lifecycle_state = "active".to_string();
                    library.archived_at_ms = None;
                }
            }
            let library = state
                .libraries
                .get(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            Ok(state.library_snapshot(library))
        })
    }

    pub(crate) fn delete_library(
        &mut self,
        library_id: &str,
    ) -> Result<DeletedLibraryCleanupPlan, ApiError> {
        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .remove(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;

            state
                .library_order
                .retain(|candidate| candidate != library_id);

            let job_ids = state
                .jobs
                .iter()
                .filter_map(|(job_id, job)| {
                    (job.snapshot.library_id == library_id).then_some(job_id.clone())
                })
                .collect::<BTreeSet<_>>();
            for job_id in &job_ids {
                state.jobs.remove(job_id);
            }
            state
                .job_order
                .retain(|candidate| !job_ids.contains(candidate));

            let temp_asset_ids = state
                .temp_query_assets
                .iter()
                .filter_map(|(temp_asset_id, asset)| {
                    asset
                        .is_library_scoped_to(library_id)
                        .then_some(temp_asset_id.clone())
                })
                .collect::<BTreeSet<_>>();
            let mut temp_asset_paths = Vec::with_capacity(temp_asset_ids.len());
            for temp_asset_id in &temp_asset_ids {
                if let Some(asset) = state.temp_query_assets.remove(temp_asset_id) {
                    temp_asset_paths.push(asset.path);
                }
            }

            Ok(DeletedLibraryCleanupPlan {
                snapshot: deleted_library_snapshot(&library),
                vector_space_ids: Vec::new(),
                temp_asset_paths,
            })
        })
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
            .map(|source| Self::source_inventory_item(library_id, library, source))
            .collect::<Result<Vec<_>, _>>()?;

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
        self.queue_source_action_with_context(
            library_id,
            scope,
            action,
            trigger,
            changed_paths_by_root,
            JobQueueContext::default(),
        )
    }

    pub(crate) fn queue_retried_source_action(
        &mut self,
        library_id: &str,
        scope: SourceActionScope,
        action: SourceActionKind,
        retried_from_job_id: String,
        attempt: u32,
    ) -> Result<(SourceActionData, Option<QueuedSourceAction>), ApiError> {
        self.queue_source_action_with_context(
            library_id,
            scope,
            action,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
            JobQueueContext {
                attempt,
                retried_from_job_id: Some(retried_from_job_id),
            },
        )
    }

    fn queue_source_action_with_context(
        &mut self,
        library_id: &str,
        scope: SourceActionScope,
        action: SourceActionKind,
        trigger: SourceActionTrigger,
        changed_paths_by_root: BTreeMap<String, BTreeSet<String>>,
        queue_context: JobQueueContext,
    ) -> Result<(SourceActionData, Option<QueuedSourceAction>), ApiError> {
        let (accepted, rejected, accepted_root_ids) =
            self.resolve_source_action_targets(library_id, &scope, action)?;

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
            cancelable: true,
            retryable: trigger == SourceActionTrigger::Manual,
            retried_from_job_id: queue_context.retried_from_job_id.clone(),
            current_attempt: JobAttemptSnapshot {
                attempt: queue_context.attempt,
                status: "queued".to_string(),
                summary: match queue_context.retried_from_job_id.as_deref() {
                    Some(retried_from_job_id) => format!(
                        "Retry attempt {} for {} after {}; queued across {} source root(s) via {} trigger.",
                        queue_context.attempt,
                        action.as_str(),
                        retried_from_job_id,
                        accepted_root_ids.len(),
                        trigger.as_str(),
                    ),
                    None => format!(
                        "Queued {} across {} source root(s) via {} trigger.",
                        action.as_str(),
                        accepted_root_ids.len(),
                        trigger.as_str(),
                    ),
                },
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
                cancellation_requested: false,
                replay: if trigger == SourceActionTrigger::Manual {
                    Some(JobReplayAction::SourceAction {
                        scope: scope.clone(),
                        action,
                    })
                } else {
                    None
                },
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
                    source_uri: source.source_uri.clone(),
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
            let display_name = request
                .display_name
                .as_deref()
                .or(Some(request.name.as_str()))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    request
                        .library_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .ok_or_else(|| {
                    ApiError::validation_failed(
                        "Library display_name must not be empty.",
                        Some(json!({ "field": "display_name" })),
                    )
                })?;

            let library_id = match request.library_id.as_deref() {
                Some(value) => normalize_library_id(value)?,
                None => state.generate_library_slug(&display_name),
            };
            if state.libraries.contains_key(&library_id) {
                return Err(ApiError::validation_failed(
                    "library_id is already in use.",
                    Some(json!({
                        "field": "library_id",
                        "library_id": library_id,
                    })),
                ));
            }
            let record = LibraryRecord {
                id: library_id.clone(),
                display_name,
                lifecycle_state: "active".to_string(),
                archived_at_ms: None,
                content_type_overrides: BTreeMap::new(),
                source_roots: BTreeMap::new(),
                source_root_order: Vec::new(),
                contents: BTreeMap::new(),
                sources: BTreeMap::new(),
                source_order: Vec::new(),
                source_asset_locations: BTreeMap::new(),
                source_asset_location_order: Vec::new(),
                assets: BTreeMap::new(),
                asset_order: Vec::new(),
                units: BTreeMap::new(),
                unit_order: Vec::new(),
                vector_spaces: BTreeMap::new(),
                unit_indexes: BTreeMap::new(),
                content_e2e_index_states: BTreeMap::new(),
                latest_job_id: None,
            };

            let snapshot = state.library_snapshot(&record);
            state.library_order.push(library_id.clone());
            state.libraries.insert(library_id.clone(), record);
            let vector_spaces = state.configured_vector_space_records_for_library(&library_id)?;
            if let Some(library) = state.libraries.get_mut(&library_id) {
                for vector_space in vector_spaces {
                    library
                        .vector_spaces
                        .insert(vector_space.id.clone(), vector_space);
                }
            }
            Ok(snapshot)
        })
    }

    pub(crate) fn prepare_import(
        &mut self,
        library_id: &str,
        request: ImportPathsRequest,
    ) -> Result<PreparedImport, ApiError> {
        let existing_manual_sources_by_path = self
            .libraries
            .get(library_id)
            .map(|library| {
                library
                    .sources
                    .values()
                    .filter(|source| source.source_root_id.is_none())
                    .cloned()
                    .map(|source| (source.source_uri.clone(), source))
                    .collect::<BTreeMap<_, _>>()
            })
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let vector_space_bindings =
            self.configured_vector_space_bindings_for_library(library_id)?;

        let request_paths = request.paths.clone();
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut new_contents = Vec::new();
        let mut new_sources = Vec::new();
        let mut new_source_asset_locations = Vec::new();
        let mut new_assets = Vec::new();
        let mut new_units = Vec::new();

        for original in request.paths {
            match self.inspect_import_path(&original) {
                Ok(mut classification) => {
                    let source_uri = file_source_uri(&classification.normalized_path);
                    if let Some(existing_source) = existing_manual_sources_by_path.get(&source_uri)
                    {
                        classification.source_id = existing_source.id.clone();
                    }
                    let (contents, source_asset_locations, assets, units) = self
                        .new_assets_and_units_from_classification(
                            &classification,
                            Some(library_id),
                        );
                    let source = self.source_record_from_classification(
                        &classification,
                        source_asset_locations
                            .iter()
                            .map(|item| item.asset_id.clone())
                            .collect(),
                        contents
                            .first()
                            .map(|content| content.id.clone())
                            .expect("generated source content should exist"),
                    );
                    let asset_summaries = assets
                        .iter()
                        .map(|asset| asset.summary(&source.id, &source.source_type))
                        .collect();
                    new_contents.extend(contents);
                    new_sources.push(source);
                    new_source_asset_locations.extend(source_asset_locations);
                    new_assets.extend(assets);
                    new_units.extend(units);

                    accepted.push(ImportAcceptedItem {
                        original_path: original,
                        normalized_path: Some(classification.normalized_path),
                        reason_code: "accepted".to_string(),
                        message: format!(
                            "Accepted as {} input for the library.",
                            classification.source_type
                        ),
                        source_id: Some(classification.source_id),
                        source_type: classification.source_type,
                        kind: classification.kind,
                        assets: asset_summaries,
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

        let vector_space_batches = vector_space_bindings
            .into_iter()
            .filter_map(|binding| {
                let units = new_units
                    .iter()
                    .filter(|unit| {
                        binding.content_types.iter().any(|content_type| {
                            import_content_type_matches_asset(content_type, &unit.asset_type)
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if units.is_empty() {
                    return None;
                }
                Some(PreparedImportVectorSpaceBatch {
                    vector_space_id: binding.vector_space_id.clone(),
                    units,
                })
            })
            .collect::<Vec<_>>();

        Ok(PreparedImport {
            library_id: library_id.to_string(),
            request: ImportPathsRequest {
                paths: request_paths,
            },
            accepted,
            rejected,
            contents: new_contents,
            sources: new_sources,
            source_asset_locations: new_source_asset_locations,
            assets: new_assets,
            units: new_units,
            vector_space_batches,
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
        library_id: &str,
        library: &LibraryRecord,
        source: &SourceRecord,
    ) -> Result<SourceInventoryItem, ApiError> {
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
        let representative_record = source
            .asset_ids
            .first()
            .and_then(|asset_id| library.assets.get(asset_id));
        let representative_asset =
            representative_record.map(|asset| asset.summary(&source.id, &source.source_type));
        let representative_preview = representative_record
            .map(|asset| {
                asset_preview_reference(library_id, &asset.id, &asset.asset_type, &asset.locator)
            })
            .transpose()?;

        Ok(SourceInventoryItem {
            source_id: source.id.clone(),
            source_uri: source.source_uri.clone(),
            source_type: source.source_type.clone(),
            media_type: source.media_type.clone(),
            kind: source.kind.clone(),
            status: source.status.clone(),
            status_reason: source.status_reason.clone(),
            relative_path: source.relative_path.clone(),
            source_root_id: source.source_root_id.clone(),
            source_root_path,
            source_root_label,
            asset_count: source.asset_ids.len(),
            representative_asset,
            representative_preview,
        })
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
        LibrarySnapshot {
            id: library.id.clone(),
            display_name: library.display_name.clone(),
            lifecycle_state: library.lifecycle_state.clone(),
            archived_at_ms: library.archived_at_ms,
            counts: LibraryCounts {
                accepted_items: accepted_item_count(library),
                pending_jobs: self
                    .jobs
                    .values()
                    .filter(|job| {
                        job.snapshot.library_id == library.id
                            && !matches!(
                                job.snapshot.status.as_str(),
                                "completed" | "failed" | "canceled"
                            )
                    })
                    .count(),
            },
            latest_job_id: library.latest_job_id.clone(),
        }
    }

    fn generate_library_slug(&self, display_name: &str) -> String {
        let base = slugify_library_id(display_name);
        if !self.libraries.contains_key(&base) {
            return base;
        }

        let mut suffix = 2_u64;
        loop {
            let candidate = format!("{base}-{suffix}");
            if !self.libraries.contains_key(&candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    pub(crate) fn source_record_from_classification(
        &self,
        classification: &PathClassification,
        asset_ids: Vec<String>,
        source_content_id: String,
    ) -> SourceRecord {
        SourceRecord {
            id: classification.source_id.clone(),
            source_root_id: None,
            source_root_path: None,
            source_path: classification.normalized_path.clone(),
            source_uri: file_source_uri(&classification.normalized_path),
            relative_path: None,
            source_type: classification.source_type.clone(),
            media_type: source_media_type(
                &classification.source_type,
                &classification.normalized_path,
            )
            .to_string(),
            kind: classification.kind.clone(),
            status: "active".to_string(),
            status_reason: None,
            page_count: classification.page_count,
            duration_ms: classification.duration_ms,
            observed_size_bytes: None,
            observed_modified_at_ms: None,
            source_content_id,
            asset_ids,
        }
    }

    pub(crate) fn new_assets_and_units_from_classification(
        &mut self,
        classification: &PathClassification,
        library_id: Option<&str>,
    ) -> (
        Vec<ContentRecord>,
        Vec<SourceAssetLocationRecord>,
        Vec<AssetRecord>,
        Vec<UnitRecord>,
    ) {
        let source_content = self
            .find_completed_source_content(library_id, &classification.normalized_path)
            .unwrap_or_else(|| self.new_content_record(&classification.normalized_path));
        let source_content_id = source_content.id.clone();
        let contents = vec![source_content];
        let mut source_asset_locations = Vec::new();
        let mut assets = Vec::new();
        let mut units = Vec::new();
        if let Some((reused_locations, reused_assets)) = self
            .reused_locations_for_completed_content(library_id, classification, &source_content_id)
        {
            return (contents, reused_locations, reused_assets, units);
        }
        if classification.kind == "document_page" {
            let page_count = classification.page_count.unwrap_or(1);
            for page_number in 1..=page_count {
                let (asset, location, unit) = self.new_asset_and_unit_record(
                    classification,
                    "document_page",
                    "page_image",
                    &source_content_id,
                    json!({
                        "page": page_number,
                        "page_label": page_number.to_string(),
                    }),
                    json!({
                        "previous_page": (page_number > 1).then_some(page_number - 1),
                        "current_page": page_number,
                        "next_page": (page_number < page_count).then_some(page_number + 1),
                        "total_pages": page_count,
                        "source_uri": file_source_uri(&classification.normalized_path),
                        "source_type": classification.source_type,
                    }),
                );
                source_asset_locations.push(location);
                assets.push(asset);
                units.push(unit);
            }
            return (contents, source_asset_locations, assets, units);
        }

        if classification.kind == "video_segment" {
            let duration_ms = classification.duration_ms.unwrap_or(1);
            let segments = build_video_segment_ranges(duration_ms);
            for (segment_index, (start_ms, end_ms)) in segments.iter().enumerate() {
                let previous = segment_index
                    .checked_sub(1)
                    .and_then(|index| segments.get(index))
                    .map(|(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }));
                let next = segments
                    .get(segment_index + 1)
                    .map(|(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }));
                let (asset, location, unit) = self.new_asset_and_unit_record(
                    classification,
                    "video_segment",
                    "keyframe_image",
                    &source_content_id,
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
                        "source_uri": file_source_uri(&classification.normalized_path),
                        "source_type": classification.source_type,
                        "duration_ms": duration_ms,
                    }),
                );
                source_asset_locations.push(location);
                assets.push(asset);
                units.push(unit);
            }
            return (contents, source_asset_locations, assets, units);
        }

        let (asset, location, unit) = self.new_asset_and_reused_unit_record(
            classification,
            &source_content_id,
            json!({
                "source_uri": file_source_uri(&classification.normalized_path),
            }),
            json!({
                "source_type": classification.source_type,
                "source_uri": file_source_uri(&classification.normalized_path),
            }),
        );
        source_asset_locations.push(location);
        assets.push(asset);
        units.push(unit);
        (contents, source_asset_locations, assets, units)
    }

    fn new_content_record(&mut self, source_path: &str) -> ContentRecord {
        let metadata = fs::metadata(source_path).ok();
        ContentRecord {
            id: self.next_content_id(),
            size_bytes: metadata.as_ref().map(|metadata| metadata.len()),
            fast_fingerprint: fast_file_fingerprint(source_path).ok(),
            sha256: None,
            created_at_ms: current_unix_ms(),
        }
    }

    fn find_completed_source_content(
        &self,
        library_id: Option<&str>,
        source_path: &str,
    ) -> Option<ContentRecord> {
        let _ = library_id?;
        let metadata = fs::metadata(source_path).ok()?;
        let fast_fingerprint = fast_file_fingerprint(source_path).ok()?;
        self.libraries.values().find_map(|library| {
            library
                .contents
                .values()
                .find(|content| {
                    content.size_bytes == Some(metadata.len())
                        && content.fast_fingerprint.as_deref() == Some(fast_fingerprint.as_str())
                        && library
                            .content_e2e_index_states
                            .values()
                            .any(|state| state.content_id == content.id)
                })
                .cloned()
        })
    }

    fn reused_locations_for_completed_content(
        &self,
        library_id: Option<&str>,
        classification: &PathClassification,
        source_content_id: &str,
    ) -> Option<(Vec<SourceAssetLocationRecord>, Vec<AssetRecord>)> {
        let _ = library_id?;
        let assets = self
            .libraries
            .values()
            .flat_map(|library| library.assets.values())
            .filter(|asset| asset.source_content_id == source_content_id)
            .map(|asset| (asset.id.clone(), asset.clone()))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect::<Vec<_>>();
        if assets.is_empty() {
            return None;
        }

        let locations = assets
            .iter()
            .map(|asset| SourceAssetLocationRecord {
                id: SourceAssetLocationRecord::key(&classification.source_id, &asset.id),
                source_id: classification.source_id.clone(),
                asset_id: asset.id.clone(),
                locator: asset.locator.clone(),
                visibility: ACTIVE_INDEX_VISIBILITY.to_string(),
            })
            .collect();
        Some((locations, assets))
    }

    fn new_asset_and_reused_unit_record(
        &mut self,
        classification: &PathClassification,
        content_id: &str,
        locator: Value,
        neighbor_context: Value,
    ) -> (AssetRecord, SourceAssetLocationRecord, UnitRecord) {
        let asset_id = self.next_asset_id();
        let unit_id = self.next_unit_id();
        let point_id = self.next_unit_seq;
        let asset = AssetRecord {
            id: asset_id.clone(),
            source_id: classification.source_id.clone(),
            content_id: content_id.to_string(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            source_content_id: content_id.to_string(),
            asset_type: classification.kind.clone(),
            locator: locator.clone(),
            derivation_signature: "source_asset:v1".to_string(),
            neighbor_context: neighbor_context.clone(),
            unit_ids: vec![unit_id.clone()],
        };
        let location = SourceAssetLocationRecord {
            id: SourceAssetLocationRecord::key(&classification.source_id, &asset_id),
            source_id: classification.source_id.clone(),
            asset_id: asset_id.clone(),
            locator: locator.clone(),
            visibility: ACTIVE_INDEX_VISIBILITY.to_string(),
        };
        let unit = UnitRecord {
            id: unit_id.clone(),
            asset_id: asset_id.clone(),
            content_id: content_id.to_string(),
            point_id,
            source_id: classification.source_id.clone(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            asset_type: classification.kind.clone(),
            unit_type: classification.kind.clone(),
            derivation_signature: "source_unit:v1".to_string(),
            locator,
            neighbor_context,
        };
        (asset, location, unit)
    }

    pub(crate) fn new_asset_and_unit_record(
        &mut self,
        classification: &PathClassification,
        asset_type: &str,
        unit_type: &str,
        source_content_id: &str,
        locator: Value,
        neighbor_context: Value,
    ) -> (AssetRecord, SourceAssetLocationRecord, UnitRecord) {
        let asset_id = self.next_asset_id();
        let unit_id = self.next_unit_id();
        let point_id = self.next_unit_seq;
        let asset = AssetRecord {
            id: asset_id.clone(),
            source_id: classification.source_id.clone(),
            content_id: source_content_id.to_string(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            source_content_id: source_content_id.to_string(),
            asset_type: asset_type.to_string(),
            locator: locator.clone(),
            derivation_signature: format!("{asset_type}:v1"),
            neighbor_context: neighbor_context.clone(),
            unit_ids: vec![unit_id.clone()],
        };
        let location = SourceAssetLocationRecord {
            id: SourceAssetLocationRecord::key(&classification.source_id, &asset_id),
            source_id: classification.source_id.clone(),
            asset_id: asset_id.clone(),
            locator: locator.clone(),
            visibility: ACTIVE_INDEX_VISIBILITY.to_string(),
        };
        let unit = UnitRecord {
            id: unit_id.clone(),
            asset_id: asset_id.clone(),
            content_id: source_content_id.to_string(),
            point_id,
            source_id: classification.source_id.clone(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            asset_type: asset_type.to_string(),
            unit_type: unit_type.to_string(),
            derivation_signature: format!("{unit_type}:v1"),
            locator: locator.clone(),
            neighbor_context: neighbor_context.clone(),
        };
        (asset, location, unit)
    }
}

fn accepted_item_count(library: &LibraryRecord) -> usize {
    library
        .sources
        .values()
        .filter(|source| source.status == "active")
        .map(|source| source.asset_ids.len())
        .sum()
}

pub(crate) fn file_source_uri(path: &str) -> String {
    if path.starts_with("file://") {
        return path.to_string();
    }
    format!("file://{path}")
}

fn fast_file_fingerprint(path: &str) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek, SeekFrom};

    const SAMPLE_BYTES: usize = 64 * 1024;

    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());

    let mut head = vec![0_u8; SAMPLE_BYTES.min(size as usize)];
    let head_len = file.read(&mut head)?;
    hasher.update(&head[..head_len]);

    if size > SAMPLE_BYTES as u64 {
        let tail_len = SAMPLE_BYTES.min(size as usize);
        file.seek(SeekFrom::End(-(tail_len as i64)))?;
        let mut tail = vec![0_u8; tail_len];
        let read_len = file.read(&mut tail)?;
        hasher.update(&tail[..read_len]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn deleted_library_snapshot(library: &LibraryRecord) -> LibrarySnapshot {
    LibrarySnapshot {
        id: library.id.clone(),
        display_name: library.display_name.clone(),
        lifecycle_state: library.lifecycle_state.clone(),
        archived_at_ms: library.archived_at_ms,
        counts: LibraryCounts {
            accepted_items: accepted_item_count(library),
            pending_jobs: 0,
        },
        latest_job_id: library.latest_job_id.clone(),
    }
}

fn import_content_type_matches_asset(content_type: &str, asset_type: &str) -> bool {
    match content_type {
        "image" => asset_type == "image",
        "document" => asset_type == "document_page",
        "video" => asset_type == "video_segment",
        "text" => asset_type == "text",
        _ => false,
    }
}

fn source_media_type(source_type: &str, source_path: &str) -> &'static str {
    crate::query_assets::content_type_for_source_type_and_path(source_type, source_path)
}

fn normalize_library_id(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation_failed(
            "library_id must not be empty.",
            Some(json!({ "field": "library_id" })),
        ));
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    {
        return Err(ApiError::validation_failed(
            "library_id must contain only lowercase letters, digits, '-' or '_'.",
            Some(json!({ "field": "library_id" })),
        ));
    }

    Ok(trimmed.to_string())
}

fn slugify_library_id(display_name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for ch in display_name.trim().chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            slug.push(lowered);
            last_was_separator = false;
        } else if !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    let slug = slug.trim_start_matches('-').to_string();
    if slug.is_empty() {
        "library".to_string()
    } else {
        slug
    }
}
