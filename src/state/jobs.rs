use super::*;

impl AppState {
    pub(crate) fn queue_import(
        &mut self,
        prepared: &PreparedImport,
    ) -> Result<ImportPathsData, ApiError> {
        if prepared.accepted.is_empty() {
            return Ok(ImportPathsData {
                accepted: prepared.accepted.clone(),
                rejected: prepared.rejected.clone(),
                job_handle: None,
                job: None,
            });
        }

        let job_id = self.next_job_id();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            library_id: prepared.library_id.clone(),
            kind: "import".to_string(),
            status: "queued".to_string(),
            phase: "intake".to_string(),
            progress: JobProgress {
                completed: 0,
                total: prepared.accepted.len(),
                unit: "item".to_string(),
            },
            cancelable: false,
            current_attempt: JobAttemptSnapshot {
                attempt: 1,
                status: "queued".to_string(),
                summary: format!(
                    "Accepted {} path(s); queued for multivector indexing.",
                    prepared.accepted.len()
                ),
            },
        };

        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        library.latest_job_id = Some(job_id.clone());

        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                snapshot: snapshot.clone(),
            },
        );
        self.job_order.push(job_id.clone());

        Ok(ImportPathsData {
            accepted: prepared.accepted.clone(),
            rejected: prepared.rejected.clone(),
            job_handle: Some(job_id),
            job: Some(snapshot),
        })
    }

    pub(crate) fn update_job_snapshot(
        &mut self,
        job_id: &str,
        status: &str,
        phase: &str,
        completed: usize,
        summary: impl Into<String>,
    ) {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.snapshot.status = status.to_string();
            job.snapshot.phase = phase.to_string();
            job.snapshot.progress.completed = completed.min(job.snapshot.progress.total);
            job.snapshot.current_attempt.status = status.to_string();
            job.snapshot.current_attempt.summary = summary.into();
        }
    }

    pub(crate) fn finalize_import_job(
        &mut self,
        job_id: &str,
        prepared: PreparedImport,
        outcome: ImportJobOutcome,
    ) -> Result<(), String> {
        if !self.jobs.contains_key(job_id) {
            return Err("Job was not found.".to_string());
        }
        if !self.libraries.contains_key(&prepared.library_id) {
            return Err("Library was not found.".to_string());
        }

        let before = self.clone();
        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| "Library was not found.".to_string())?;

        for source in &prepared.sources {
            library.source_order.push(source.id.clone());
            library.sources.insert(source.id.clone(), source.clone());
        }

        for visual_unit in &prepared.visual_units {
            library.visual_unit_order.push(visual_unit.id.clone());
            library
                .visual_units
                .insert(visual_unit.id.clone(), visual_unit.clone());
        }

        if outcome.activate_index {
            library
                .active_index_lines
                .insert(MULTIVECTOR_INDEX_LINE.to_string());
        }

        if let Err(message) = self.persist_durable_state() {
            *self = before;
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.snapshot.status = "failed".to_string();
                job.snapshot.phase = "failed".to_string();
                job.snapshot.current_attempt.status = "failed".to_string();
                job.snapshot.current_attempt.summary =
                    format!("Persisting durable app state failed: {message}");
            }
            return Err(format!("Failed to persist durable app state: {message}"));
        }

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| "Job was not found.".to_string())?;
        job.snapshot.status = outcome.status.to_string();
        job.snapshot.phase = outcome.phase.to_string();
        job.snapshot.progress.completed = outcome.completed.min(job.snapshot.progress.total);
        job.snapshot.current_attempt.status = outcome.status.to_string();
        job.snapshot.current_attempt.summary = outcome.summary;

        Ok(())
    }

    pub(crate) fn mark_source_action_running(&mut self, plan: &SourceActionPlan, job_id: &str) {
        self.update_job_snapshot(
            job_id,
            "running",
            "scan",
            0,
            format!(
                "{} is evaluating {} source root(s).",
                plan.action.as_str(),
                plan.target_root_ids.len()
            ),
        );

        if let Some(library) = self.libraries.get_mut(&plan.library_id) {
            for source_root_id in &plan.target_root_ids {
                if let Some(root) = library.source_roots.get_mut(source_root_id) {
                    root.watch_state = running_watch_state_for_action(plan.action).to_string();
                }
            }
        }
    }

    pub(crate) fn prepare_source_action_execution(
        &mut self,
        plan: &SourceActionPlan,
    ) -> Result<PreparedSourceAction, String> {
        let (collection_name, had_existing_visual_units, can_rebuild_from_scratch) = self
            .libraries
            .get(&plan.library_id)
            .map(|library| {
                (
                    library.collection_name.clone(),
                    !library.visual_units.is_empty(),
                    plan.action.is_rescan()
                        && plan.target_root_ids.len() == library.source_root_order.len(),
                )
            })
            .ok_or_else(|| "Library was not found.".to_string())?;

        let mut root_updates = Vec::new();
        let mut source_mutations = Vec::new();
        let mut stale_point_ids = Vec::new();
        let mut visual_units_to_index = Vec::new();
        let mut summary = SourceActionSummary::default();

        for source_root_id in &plan.target_root_ids {
            let (root, existing_sources, existing_point_ids_by_source) = {
                let library = self
                    .libraries
                    .get(&plan.library_id)
                    .ok_or_else(|| "Library was not found.".to_string())?;
                let root = library
                    .source_roots
                    .get(source_root_id)
                    .cloned()
                    .ok_or_else(|| format!("Source root {source_root_id} was not found."))?;
                let existing_sources = library
                    .sources
                    .values()
                    .filter(|source| {
                        source.source_root_id.as_deref() == Some(source_root_id.as_str())
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let existing_point_ids_by_source = existing_sources
                    .iter()
                    .map(|source| {
                        let point_ids = source
                            .visual_unit_ids
                            .iter()
                            .filter_map(|visual_unit_id| {
                                library
                                    .visual_units
                                    .get(visual_unit_id)
                                    .map(|visual_unit| visual_unit.point_id)
                            })
                            .collect::<Vec<_>>();
                        (source.id.clone(), point_ids)
                    })
                    .collect::<BTreeMap<_, _>>();
                (root, existing_sources, existing_point_ids_by_source)
            };

            let scan = scan_source_root_directory(&root.root_path);
            let candidate_entries = scan
                .observed_entries
                .values()
                .filter(|entry| observed_entry_is_in_scope(entry, &root.rules))
                .cloned()
                .collect::<Vec<_>>();
            let candidate_by_relative_path = candidate_entries
                .iter()
                .map(|entry| (entry.relative_path.clone(), entry.clone()))
                .collect::<BTreeMap<_, _>>();
            let existing_by_relative_path = existing_sources
                .iter()
                .filter_map(|source| {
                    source
                        .relative_path
                        .as_ref()
                        .map(|relative_path| (relative_path.clone(), source.clone()))
                })
                .collect::<BTreeMap<_, _>>();

            let affected_paths = planned_source_action_paths(
                plan,
                &root,
                &candidate_by_relative_path,
                &existing_by_relative_path,
            );
            let mut root_status_by_source_id = existing_sources
                .iter()
                .map(|source| (source.id.clone(), source.status.clone()))
                .collect::<BTreeMap<_, _>>();

            for relative_path in affected_paths {
                let current_entry = candidate_by_relative_path.get(&relative_path);
                let existing_source = existing_by_relative_path.get(&relative_path);

                if let Some(entry) = current_entry {
                    match self.inspect_import_path(&entry.absolute_path) {
                        Ok(mut classification) => {
                            if let Some(existing_source) = existing_source {
                                classification.source_id = existing_source.id.clone();
                                stale_point_ids.extend(
                                    existing_point_ids_by_source
                                        .get(&existing_source.id)
                                        .into_iter()
                                        .flatten()
                                        .copied(),
                                );
                            }
                            let visual_units =
                                self.new_visual_units_from_classification(&classification);
                            let mut source = self.source_record_from_classification(
                                &classification,
                                visual_units.iter().map(|item| item.id.clone()).collect(),
                            );
                            source.source_root_id = Some(root.id.clone());
                            source.source_root_path = Some(root.root_path.clone());
                            source.relative_path = Some(relative_path.clone());
                            source.status = "active".to_string();
                            source.status_reason = None;
                            source.observed_size_bytes = Some(entry.size_bytes);
                            source.observed_modified_at_ms = entry.modified_at_ms;

                            root_status_by_source_id
                                .insert(source.id.clone(), source.status.clone());
                            summary.activated_sources += 1;
                            summary.indexing_visual_units += visual_units.len();
                            visual_units_to_index.extend(visual_units.iter().cloned());
                            source_mutations.push(PreparedSourceMutation {
                                source,
                                visual_units,
                            });
                        }
                        Err(rejection) => {
                            if let Some(existing_source) = existing_source.cloned() {
                                stale_point_ids.extend(
                                    existing_point_ids_by_source
                                        .get(&existing_source.id)
                                        .into_iter()
                                        .flatten()
                                        .copied(),
                                );
                                let mutated = invalidated_source_record(
                                    existing_source,
                                    "invalidated",
                                    Some(rejection.reason_code),
                                    Some(entry.size_bytes),
                                    entry.modified_at_ms,
                                );
                                root_status_by_source_id
                                    .insert(mutated.id.clone(), mutated.status.clone());
                                summary.invalidated_sources += 1;
                                source_mutations.push(PreparedSourceMutation {
                                    source: mutated,
                                    visual_units: Vec::new(),
                                });
                            }
                        }
                    }
                    continue;
                }

                let Some(existing_source) = existing_source.cloned() else {
                    continue;
                };

                let (status, reason) = match scan.observed_entries.get(&relative_path) {
                    Some(observed) => out_of_scope_status_reason(observed, &root.rules),
                    None if scan.error.is_some() => (
                        "invalidated".to_string(),
                        Some("source_root_unreachable".to_string()),
                    ),
                    None => ("invalidated".to_string(), Some("not_found".to_string())),
                };
                if status == "invalidated" {
                    summary.invalidated_sources += 1;
                } else {
                    summary.out_of_scope_sources += 1;
                }
                stale_point_ids.extend(
                    existing_point_ids_by_source
                        .get(&existing_source.id)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
                let mutated =
                    invalidated_source_record(existing_source, &status, reason, None, None);
                root_status_by_source_id.insert(mutated.id.clone(), mutated.status.clone());
                source_mutations.push(PreparedSourceMutation {
                    source: mutated,
                    visual_units: Vec::new(),
                });
            }

            summary.scanned_roots += 1;
            summary.observed_files += scan.observed_entries.len();
            summary.matched_files += candidate_entries.len();
            if scan.status == "degraded" {
                summary.degraded_roots += 1;
            }

            let active_source_count = root_status_by_source_id
                .values()
                .filter(|status| status.as_str() == "active")
                .count();
            root_updates.push(PreparedSourceRootUpdate {
                source_root_id: root.id.clone(),
                status: source_root_status_from_scan(root.enabled, &scan),
                watch_state: source_root_watch_state(root.enabled, &scan, false),
                coverage_summary: SourceRootCoverageSummary {
                    observed_file_count: scan.observed_entries.len(),
                    matched_file_count: candidate_entries.len(),
                    active_source_count,
                    inactive_source_count: root_status_by_source_id
                        .len()
                        .saturating_sub(active_source_count),
                    last_scan_at_ms: Some(current_unix_ms()),
                },
                observed_entries: scan.observed_entries,
            });
        }

        stale_point_ids.sort_unstable();
        stale_point_ids.dedup();

        Ok(PreparedSourceAction {
            library_id: plan.library_id.clone(),
            collection_name,
            action: plan.action,
            accepted_root_count: plan.target_root_ids.len(),
            can_rebuild_from_scratch,
            had_existing_visual_units,
            root_updates,
            source_mutations,
            stale_point_ids,
            visual_units_to_index,
            summary,
        })
    }

    pub(crate) fn finalize_source_action_job(
        &mut self,
        job_id: &str,
        prepared: PreparedSourceAction,
        outcome: SourceActionJobOutcome,
    ) -> Result<(), String> {
        if !self.jobs.contains_key(job_id) {
            return Err("Job was not found.".to_string());
        }
        if !self.libraries.contains_key(&prepared.library_id) {
            return Err("Library was not found.".to_string());
        }

        if outcome.status == "completed" {
            let before = self.clone();
            let library = self
                .libraries
                .get_mut(&prepared.library_id)
                .ok_or_else(|| "Library was not found.".to_string())?;

            for mutation in &prepared.source_mutations {
                let old_visual_unit_ids = library
                    .sources
                    .get(&mutation.source.id)
                    .map(|source| source.visual_unit_ids.clone())
                    .unwrap_or_default();
                if !old_visual_unit_ids.is_empty() {
                    let stale_ids = old_visual_unit_ids.iter().cloned().collect::<BTreeSet<_>>();
                    library
                        .visual_unit_order
                        .retain(|visual_unit_id| !stale_ids.contains(visual_unit_id));
                    for visual_unit_id in old_visual_unit_ids {
                        library.visual_units.remove(&visual_unit_id);
                    }
                }

                if !library.sources.contains_key(&mutation.source.id) {
                    library.source_order.push(mutation.source.id.clone());
                }
                library
                    .sources
                    .insert(mutation.source.id.clone(), mutation.source.clone());

                for visual_unit in &mutation.visual_units {
                    library.visual_unit_order.push(visual_unit.id.clone());
                    library
                        .visual_units
                        .insert(visual_unit.id.clone(), visual_unit.clone());
                }
            }

            for update in &prepared.root_updates {
                if let Some(root) = library.source_roots.get_mut(&update.source_root_id) {
                    root.status = update.status.clone();
                    root.watch_state = update.watch_state.clone();
                    root.coverage_summary = update.coverage_summary.clone();
                    root.observed_entries = update.observed_entries.clone();
                    root.pending_watch_error = None;
                }
            }

            if outcome.activate_index {
                library
                    .active_index_lines
                    .insert(MULTIVECTOR_INDEX_LINE.to_string());
            }

            if let Err(message) = self.persist_durable_state() {
                *self = before;
                if let Some(job) = self.jobs.get_mut(job_id) {
                    job.snapshot.status = "failed".to_string();
                    job.snapshot.phase = "failed".to_string();
                    job.snapshot.current_attempt.status = "failed".to_string();
                    job.snapshot.current_attempt.summary =
                        format!("Persisting durable app state failed: {message}");
                }
                return Err(format!("Failed to persist durable app state: {message}"));
            }
        }

        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| "Library was not found.".to_string())?;

        for update in &prepared.root_updates {
            if let Some(root) = library.source_roots.get_mut(&update.source_root_id) {
                root.watch_state = if outcome.status == "completed" {
                    update.watch_state.clone()
                } else {
                    source_root_watch_state(
                        root.enabled,
                        &SourceRootScanResult {
                            status: root.status.clone(),
                            observed_entries: root.observed_entries.clone(),
                            error: root.pending_watch_error.clone(),
                        },
                        false,
                    )
                };
                root.last_action = Some(SourceRootLastAction {
                    action: prepared.action.as_str().to_string(),
                    status: outcome.status.to_string(),
                    summary: outcome.summary.clone(),
                    job_id: Some(job_id.to_string()),
                });
            }
        }

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| "Job was not found.".to_string())?;
        job.snapshot.status = outcome.status.to_string();
        job.snapshot.phase = outcome.phase.to_string();
        job.snapshot.progress.completed = outcome.completed.min(job.snapshot.progress.total);
        job.snapshot.current_attempt.status = outcome.status.to_string();
        job.snapshot.current_attempt.summary = outcome.summary;

        Ok(())
    }

    pub(crate) fn list_jobs(&self, library_id: Option<&str>) -> JobsListData {
        let jobs = self
            .job_order
            .iter()
            .rev()
            .filter_map(|job_id| self.jobs.get(job_id))
            .filter(|job| {
                library_id
                    .map(|expected| job.snapshot.library_id == expected)
                    .unwrap_or(true)
            })
            .map(|job| job.snapshot.clone())
            .collect();

        JobsListData { jobs }
    }

    pub(crate) fn get_job(&self, job_id: &str) -> Result<JobSnapshot, ApiError> {
        self.jobs
            .get(job_id)
            .map(|job| job.snapshot.clone())
            .ok_or_else(|| ApiError::not_found("Job was not found."))
    }
}
