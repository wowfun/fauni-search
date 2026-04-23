use super::*;

impl AppState {
    fn resolve_maintenance_target_vector_space_ids(
        &self,
        library_id: &str,
        action: MaintenanceActionKind,
    ) -> Result<Vec<String>, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        Ok(match action {
            MaintenanceActionKind::CleanupRetiredVectorSpaces => library
                .retired_vector_spaces
                .keys()
                .filter(|vector_space_id| !library.active_vector_spaces.contains(*vector_space_id))
                .cloned()
                .collect::<Vec<_>>(),
        })
    }

    fn reopen_job_attempt(
        &mut self,
        job_id: &str,
        total: usize,
        unit: &str,
        attempt: u32,
        summary: String,
    ) -> Result<JobSnapshot, ApiError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ApiError::not_found("Job was not found."))?;
        job.cancellation_requested = false;
        job.snapshot.status = "queued".to_string();
        job.snapshot.phase = "intake".to_string();
        job.snapshot.progress.completed = 0;
        job.snapshot.progress.total = total;
        job.snapshot.progress.unit = unit.to_string();
        job.snapshot.cancelable = true;
        job.snapshot.current_attempt.attempt = attempt;
        job.snapshot.current_attempt.status = "queued".to_string();
        job.snapshot.current_attempt.summary = summary;
        Ok(job.snapshot.clone())
    }

    pub(crate) fn queue_maintenance_action(
        &mut self,
        library_id: &str,
        action: MaintenanceActionKind,
    ) -> Result<(MaintenanceActionData, Option<QueuedMaintenanceAction>), ApiError> {
        self.queue_maintenance_action_with_context(library_id, action, JobQueueContext::default())
    }

    pub(crate) fn queue_retried_maintenance_action(
        &mut self,
        library_id: &str,
        action: MaintenanceActionKind,
        retried_from_job_id: String,
        attempt: u32,
    ) -> Result<(MaintenanceActionData, Option<QueuedMaintenanceAction>), ApiError> {
        self.queue_maintenance_action_with_context(
            library_id,
            action,
            JobQueueContext {
                attempt,
                retried_from_job_id: Some(retried_from_job_id),
            },
        )
    }

    fn queue_maintenance_action_with_context(
        &mut self,
        library_id: &str,
        action: MaintenanceActionKind,
        queue_context: JobQueueContext,
    ) -> Result<(MaintenanceActionData, Option<QueuedMaintenanceAction>), ApiError> {
        let target_vector_space_ids =
            self.resolve_maintenance_target_vector_space_ids(library_id, action)?;

        if target_vector_space_ids.is_empty() {
            return Ok((
                MaintenanceActionData {
                    action: action.as_str().to_string(),
                    accepted: Vec::new(),
                    rejected: vec![MaintenanceActionRejectedItem {
                        reason_code: "nothing_to_clean".to_string(),
                        message: "当前库没有可立即清理的退役执行空间。".to_string(),
                    }],
                    job_handle: None,
                    job: None,
                },
                None,
            ));
        }

        let accepted = target_vector_space_ids
            .iter()
            .map(|vector_space_id| MaintenanceActionAcceptedItem {
                target_kind: "vector_space".to_string(),
                target_id: vector_space_id.clone(),
                message: "已加入退役执行空间清理队列。".to_string(),
            })
            .collect::<Vec<_>>();

        let job_id = self.next_job_id();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            library_id: library_id.to_string(),
            kind: "cleanup".to_string(),
            status: "queued".to_string(),
            phase: "intake".to_string(),
            progress: JobProgress {
                completed: 0,
                total: target_vector_space_ids.len(),
                unit: "vector_space".to_string(),
            },
            cancelable: true,
            retryable: true,
            retried_from_job_id: queue_context.retried_from_job_id.clone(),
            current_attempt: JobAttemptSnapshot {
                attempt: queue_context.attempt,
                status: "queued".to_string(),
                summary: match queue_context.retried_from_job_id.as_deref() {
                    Some(retried_from_job_id) => format!(
                        "Retry attempt {} for {} after {}; queued across {} retired vector-space namespace(s).",
                        queue_context.attempt,
                        action.as_str(),
                        retried_from_job_id,
                        target_vector_space_ids.len(),
                    ),
                    None => format!(
                        "Queued {} across {} retired vector-space namespace(s).",
                        action.as_str(),
                        target_vector_space_ids.len(),
                    ),
                },
            },
        };

        let plan = MaintenanceActionPlan {
            library_id: library_id.to_string(),
            action,
            target_vector_space_ids,
        };

        let library = self
            .libraries
            .get_mut(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        library.latest_job_id = Some(job_id.clone());

        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                snapshot: snapshot.clone(),
                cancellation_requested: false,
                replay: Some(JobReplayAction::Maintenance { action }),
            },
        );
        self.job_order.push(job_id.clone());

        Ok((
            MaintenanceActionData {
                action: action.as_str().to_string(),
                accepted,
                rejected: Vec::new(),
                job_handle: Some(job_id.clone()),
                job: Some(snapshot),
            },
            Some(QueuedMaintenanceAction { job_id, plan }),
        ))
    }

    pub(crate) fn queue_import(
        &mut self,
        prepared: &PreparedImport,
    ) -> Result<ImportPathsData, ApiError> {
        self.queue_import_with_context(prepared, JobQueueContext::default())
    }

    pub(crate) fn queue_retried_import(
        &mut self,
        prepared: &PreparedImport,
        retried_from_job_id: String,
        attempt: u32,
    ) -> Result<ImportPathsData, ApiError> {
        self.queue_import_with_context(
            prepared,
            JobQueueContext {
                attempt,
                retried_from_job_id: Some(retried_from_job_id),
            },
        )
    }

    fn queue_import_with_context(
        &mut self,
        prepared: &PreparedImport,
        queue_context: JobQueueContext,
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
            cancelable: true,
            retryable: true,
            retried_from_job_id: queue_context.retried_from_job_id.clone(),
            current_attempt: JobAttemptSnapshot {
                attempt: queue_context.attempt,
                status: "queued".to_string(),
                summary: match queue_context.retried_from_job_id.as_deref() {
                    Some(retried_from_job_id) => format!(
                        "Retry attempt {} for import after {}; accepted {} path(s) and queued them for vector-space indexing.",
                        queue_context.attempt,
                        retried_from_job_id,
                        prepared.accepted.len(),
                    ),
                    None => format!(
                        "Accepted {} path(s); queued for vector-space indexing.",
                        prepared.accepted.len()
                    ),
                },
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
                cancellation_requested: false,
                replay: Some(JobReplayAction::Import {
                    request: prepared.request.clone(),
                }),
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

    pub(crate) fn request_job_cancellation(
        &mut self,
        job_id: &str,
    ) -> Result<JobSnapshot, ApiError> {
        let (library_id, replay) = {
            let job = self
                .jobs
                .get(job_id)
                .ok_or_else(|| ApiError::not_found("Job was not found."))?;
            (job.snapshot.library_id.clone(), job.replay.clone())
        };

        let mut reset_source_action_scope = None;

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| ApiError::not_found("Job was not found."))?;

        if is_terminal_job_status(&job.snapshot.status) {
            return Err(ApiError::conflict(
                "Job is already in a terminal state and cannot be canceled.",
                Some(json!({ "job_id": job_id, "status": job.snapshot.status })),
            ));
        }

        if !job.snapshot.cancelable {
            return Err(ApiError::conflict(
                "Job is not cancelable.",
                Some(json!({ "job_id": job_id, "status": job.snapshot.status })),
            ));
        }

        job.cancellation_requested = true;
        job.snapshot.cancelable = false;

        if job.snapshot.status == "queued" {
            job.snapshot.status = "canceled".to_string();
            job.snapshot.phase = "canceled".to_string();
            job.snapshot.current_attempt.status = "canceled".to_string();
            job.snapshot.current_attempt.summary =
                "Canceled before background execution started.".to_string();
            if let Some(JobReplayAction::SourceAction { scope, action }) = replay {
                reset_source_action_scope =
                    Some((scope, action, job.snapshot.current_attempt.summary.clone()));
            }
        } else {
            let current_phase = job.snapshot.phase.clone();
            job.snapshot.phase = "cancel_requested".to_string();
            job.snapshot.current_attempt.summary = format!(
                "Cancellation requested during {current_phase}. The job will stop at the next safe boundary."
            );
        }

        let snapshot = job.snapshot.clone();

        if let Some((scope, action, summary)) = reset_source_action_scope {
            restore_canceled_source_action_roots(
                self,
                &library_id,
                &scope,
                action,
                job_id,
                &summary,
            );
        }

        Ok(snapshot)
    }

    pub(crate) fn request_job_retry(
        &mut self,
        job_id: &str,
    ) -> Result<(JobSnapshot, RetryJobDispatch), ApiError> {
        let (library_id, status, retryable, replay, next_attempt) = {
            let job = self
                .jobs
                .get(job_id)
                .ok_or_else(|| ApiError::not_found("Job was not found."))?;
            (
                job.snapshot.library_id.clone(),
                job.snapshot.status.clone(),
                job.snapshot.retryable,
                job.replay.clone(),
                job.snapshot.current_attempt.attempt.saturating_add(1),
            )
        };

        if !is_terminal_job_status(&status) {
            return Err(ApiError::conflict(
                "Only terminal jobs can be retried.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        if status == "completed" {
            return Err(ApiError::conflict(
                "Completed jobs do not require retry.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        if !retryable {
            return Err(ApiError::conflict(
                "Job does not support retry.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        let replay = replay.ok_or_else(|| {
            ApiError::conflict(
                "Job no longer has a replayable request.",
                Some(json!({ "job_id": job_id, "status": status })),
            )
        })?;

        match replay {
            JobReplayAction::Import { request } => {
                let prepared = self.prepare_import(&library_id, request).map_err(|error| {
                    ApiError::conflict(
                        error.payload.message,
                        Some(json!({ "job_id": job_id, "status": status, "kind": "import" })),
                    )
                })?;
                let response =
                    self.queue_retried_import(&prepared, job_id.to_string(), next_attempt)?;
                let snapshot = response.job.ok_or_else(|| {
                    let message = response
                        .rejected
                        .first()
                        .map(|item| item.message.clone())
                        .unwrap_or_else(|| {
                            "Retry request did not produce a new import job snapshot.".to_string()
                        });
                    ApiError::conflict(
                        message,
                        Some(json!({ "job_id": job_id, "status": status, "kind": "import" })),
                    )
                })?;
                Ok((snapshot, RetryJobDispatch::Import(prepared)))
            }
            JobReplayAction::SourceAction { scope, action } => {
                let (response, queued_action) = self.queue_retried_source_action(
                    &library_id,
                    scope,
                    action,
                    job_id.to_string(),
                    next_attempt,
                )?;
                let queued_action = queued_action.ok_or_else(|| {
                    let message = response
                        .rejected
                        .first()
                        .map(|item| item.message.clone())
                        .unwrap_or_else(|| {
                            "Retry request did not queue any source-management work.".to_string()
                        });
                    ApiError::conflict(
                        message,
                        Some(
                            json!({ "job_id": job_id, "status": status, "kind": "source_action" }),
                        ),
                    )
                })?;
                let snapshot = response.job.ok_or_else(|| {
                    ApiError::conflict(
                        "Retry request did not produce a new job snapshot.",
                        Some(
                            json!({ "job_id": job_id, "status": status, "kind": "source_action" }),
                        ),
                    )
                })?;
                Ok((snapshot, RetryJobDispatch::SourceAction(queued_action)))
            }
            JobReplayAction::Maintenance { action } => {
                let (response, queued_action) = self.queue_retried_maintenance_action(
                    &library_id,
                    action,
                    job_id.to_string(),
                    next_attempt,
                )?;
                let queued_action = queued_action.ok_or_else(|| {
                    let message = response
                        .rejected
                        .first()
                        .map(|item| item.message.clone())
                        .unwrap_or_else(|| {
                            "Retry request did not queue any maintenance work.".to_string()
                        });
                    ApiError::conflict(
                        message,
                        Some(
                            json!({ "job_id": job_id, "status": status, "kind": action.as_str() }),
                        ),
                    )
                })?;
                let snapshot = response.job.ok_or_else(|| {
                    ApiError::conflict(
                        "Retry request did not produce a new job snapshot.",
                        Some(
                            json!({ "job_id": job_id, "status": status, "kind": action.as_str() }),
                        ),
                    )
                })?;
                Ok((snapshot, RetryJobDispatch::Maintenance(queued_action)))
            }
        }
    }

    pub(crate) fn request_job_resume(
        &mut self,
        job_id: &str,
    ) -> Result<(JobSnapshot, ResumeJobDispatch), ApiError> {
        let (library_id, status, retryable, replay, next_attempt) = {
            let job = self
                .jobs
                .get(job_id)
                .ok_or_else(|| ApiError::not_found("Job was not found."))?;
            (
                job.snapshot.library_id.clone(),
                job.snapshot.status.clone(),
                job.snapshot.retryable,
                job.replay.clone(),
                job.snapshot.current_attempt.attempt.saturating_add(1),
            )
        };

        if !is_terminal_job_status(&status) {
            return Err(ApiError::conflict(
                "Only terminal jobs can be resumed.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        if status == "completed" {
            return Err(ApiError::conflict(
                "Completed jobs do not require resume.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        if !retryable {
            return Err(ApiError::conflict(
                "Job does not support resume.",
                Some(json!({ "job_id": job_id, "status": status })),
            ));
        }

        let replay = replay.ok_or_else(|| {
            ApiError::conflict(
                "Job no longer has a replayable request.",
                Some(json!({ "job_id": job_id, "status": status })),
            )
        })?;

        match replay {
            JobReplayAction::Import { request } => {
                let prepared = self.prepare_import(&library_id, request).map_err(|error| {
                    ApiError::conflict(
                        error.payload.message,
                        Some(json!({ "job_id": job_id, "status": status, "kind": "import" })),
                    )
                })?;
                if prepared.accepted.is_empty() {
                    let message = prepared
                        .rejected
                        .first()
                        .map(|item| item.message.clone())
                        .unwrap_or_else(|| {
                            "Resume request did not produce any accepted import paths.".to_string()
                        });
                    return Err(ApiError::conflict(
                        message,
                        Some(json!({ "job_id": job_id, "status": status, "kind": "import" })),
                    ));
                }
                if let Some(library) = self.libraries.get_mut(&prepared.library_id) {
                    library.latest_job_id = Some(job_id.to_string());
                }
                let snapshot = self.reopen_job_attempt(
                    job_id,
                    prepared.accepted.len(),
                    "item",
                    next_attempt,
                    format!(
                        "Resume attempt {} for import on existing job; accepted {} path(s) and requeued them for vector-space indexing.",
                        next_attempt,
                        prepared.accepted.len(),
                    ),
                )?;
                Ok((snapshot, ResumeJobDispatch::Import(prepared)))
            }
            JobReplayAction::SourceAction { scope, action } => {
                let plan = self.plan_source_action_replay(&library_id, scope, action)?;
                if let Some(library) = self.libraries.get_mut(&library_id) {
                    for source_root_id in &plan.target_root_ids {
                        if let Some(root) = library.source_roots.get_mut(source_root_id) {
                            root.watch_state = queued_watch_state_for_action(action).to_string();
                        }
                    }
                    library.latest_job_id = Some(job_id.to_string());
                }
                let snapshot = self.reopen_job_attempt(
                    job_id,
                    plan.target_root_ids.len(),
                    "source_root",
                    next_attempt,
                    format!(
                        "Resume attempt {} for {} on existing job across {} source root(s).",
                        next_attempt,
                        action.as_str(),
                        plan.target_root_ids.len(),
                    ),
                )?;
                Ok((snapshot, ResumeJobDispatch::SourceAction(plan)))
            }
            JobReplayAction::Maintenance { action } => {
                let target_vector_space_ids =
                    self.resolve_maintenance_target_vector_space_ids(&library_id, action)?;
                if target_vector_space_ids.is_empty() {
                    return Err(ApiError::conflict(
                        "Resume request did not find any remaining maintenance targets.",
                        Some(
                            json!({ "job_id": job_id, "status": status, "kind": action.as_str() }),
                        ),
                    ));
                }
                if let Some(library) = self.libraries.get_mut(&library_id) {
                    library.latest_job_id = Some(job_id.to_string());
                }
                let snapshot = self.reopen_job_attempt(
                    job_id,
                    target_vector_space_ids.len(),
                    "vector_space",
                    next_attempt,
                    format!(
                        "Resume attempt {} for {} on existing job across {} retired vector-space namespace(s).",
                        next_attempt,
                        action.as_str(),
                        target_vector_space_ids.len(),
                    ),
                )?;
                Ok((
                    snapshot,
                    ResumeJobDispatch::Maintenance(MaintenanceActionPlan {
                        library_id,
                        action,
                        target_vector_space_ids,
                    }),
                ))
            }
        }
    }

    pub(crate) fn job_cancellation_requested(&self, job_id: &str) -> bool {
        self.jobs
            .get(job_id)
            .map(|job| job.cancellation_requested || job.snapshot.status == "canceled")
            .unwrap_or(false)
    }

    pub(crate) fn job_is_terminal(&self, job_id: &str) -> bool {
        self.jobs
            .get(job_id)
            .map(|job| is_terminal_job_status(&job.snapshot.status))
            .unwrap_or(true)
    }

    pub(crate) fn finalize_job_as_canceled(
        &mut self,
        job_id: &str,
        completed: usize,
        summary: impl Into<String>,
    ) -> Result<(), String> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| "Job was not found.".to_string())?;
        job.cancellation_requested = true;
        job.snapshot.status = "canceled".to_string();
        job.snapshot.phase = "canceled".to_string();
        job.snapshot.cancelable = false;
        job.snapshot.progress.completed = completed.min(job.snapshot.progress.total);
        job.snapshot.current_attempt.status = "canceled".to_string();
        job.snapshot.current_attempt.summary = summary.into();
        Ok(())
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
            let old_visual_unit_ids = library
                .sources
                .get(&source.id)
                .map(|existing_source| existing_source.visual_unit_ids.clone())
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

            if !library.sources.contains_key(&source.id) {
                library.source_order.push(source.id.clone());
            }
            library.sources.insert(source.id.clone(), source.clone());
        }

        for visual_unit in &prepared.visual_units {
            library.visual_unit_order.push(visual_unit.id.clone());
            library
                .visual_units
                .insert(visual_unit.id.clone(), visual_unit.clone());
        }

        for vector_space_id in &outcome.activated_vector_spaces {
            library.active_vector_spaces.insert(vector_space_id.clone());
            library.retired_vector_spaces.remove(vector_space_id);
        }

        if let Err(message) = self.persist_durable_state() {
            *self = before;
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.snapshot.cancelable = false;
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
        job.snapshot.cancelable = false;
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
        let active_vector_spaces = self
            .libraries
            .get(&plan.library_id)
            .map(|library| library.active_vector_spaces.clone())
            .ok_or_else(|| "Library was not found.".to_string())?;
        let vector_space_bindings = self
            .configured_vector_space_bindings_for_library(&plan.library_id)
            .map_err(|error| error.payload.message)?;
        let can_rebuild_from_scratch = self
            .libraries
            .get(&plan.library_id)
            .map(|library| {
                plan.action.rebuilds_from_scratch()
                    || (plan.action.is_rescan()
                        && plan.target_root_ids.len() == library.source_root_order.len())
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
        let vector_space_batches = vector_space_bindings
            .into_iter()
            .filter_map(|binding| {
                let visual_units = visual_units_to_index
                    .iter()
                    .filter(|visual_unit| {
                        binding.content_types.iter().any(|content_type| {
                            source_action_content_type_matches_visual_unit(
                                content_type,
                                &visual_unit.kind,
                            )
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if visual_units.is_empty() && stale_point_ids.is_empty() {
                    return None;
                }
                Some(PreparedSourceActionVectorSpaceBatch {
                    vector_space_id: binding.vector_space_id.clone(),
                    content_types: binding.content_types,
                    can_rebuild_from_scratch,
                    had_existing_index: active_vector_spaces.contains(&binding.vector_space_id),
                    stale_point_ids: stale_point_ids.clone(),
                    visual_units_to_index: visual_units,
                })
            })
            .collect::<Vec<_>>();

        Ok(PreparedSourceAction {
            library_id: plan.library_id.clone(),
            action: plan.action,
            accepted_root_count: plan.target_root_ids.len(),
            root_updates,
            source_mutations,
            summary,
            vector_space_batches,
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

        if outcome.apply_structured_changes {
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

            for vector_space_id in &outcome.activated_vector_spaces {
                library.active_vector_spaces.insert(vector_space_id.clone());
                library.retired_vector_spaces.remove(vector_space_id);
            }

            if let Err(message) = self.persist_durable_state() {
                *self = before;
                if let Some(job) = self.jobs.get_mut(job_id) {
                    job.snapshot.cancelable = false;
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
                root.watch_state = if outcome.apply_structured_changes {
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
        job.snapshot.cancelable = false;
        job.snapshot.status = outcome.status.to_string();
        job.snapshot.phase = outcome.phase.to_string();
        job.snapshot.progress.completed = outcome.completed.min(job.snapshot.progress.total);
        job.snapshot.current_attempt.status = outcome.status.to_string();
        job.snapshot.current_attempt.summary = outcome.summary;

        Ok(())
    }

    pub(crate) fn finalize_maintenance_action_job(
        &mut self,
        job_id: &str,
        plan: &MaintenanceActionPlan,
        cleaned: &[RetiredVectorSpaceCleanupCandidate],
        failures: &[String],
    ) -> Result<(), String> {
        if !self.jobs.contains_key(job_id) {
            return Err("Job was not found.".to_string());
        }
        if !self.libraries.contains_key(&plan.library_id) {
            return Err("Library was not found.".to_string());
        }

        if let Err(message) = self.forget_cleaned_retired_vector_spaces(cleaned) {
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.snapshot.cancelable = false;
                job.snapshot.status = "failed".to_string();
                job.snapshot.phase = "failed".to_string();
                job.snapshot.current_attempt.status = "failed".to_string();
                job.snapshot.current_attempt.summary =
                    format!("Persisting cleanup progress failed: {message}");
            }
            return Err(format!("Failed to persist cleanup progress: {message}"));
        }

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| "Job was not found.".to_string())?;
        job.snapshot.progress.completed = cleaned.len().min(job.snapshot.progress.total);
        job.snapshot.cancelable = false;

        if failures.is_empty() {
            job.snapshot.status = "completed".to_string();
            job.snapshot.phase = "cleaned".to_string();
            job.snapshot.current_attempt.status = "completed".to_string();
            job.snapshot.current_attempt.summary = format!(
                "Cleaned {} retired vector-space namespace(s).",
                cleaned.len()
            );
            return Ok(());
        }

        job.snapshot.status = "failed".to_string();
        job.snapshot.phase = "failed".to_string();
        job.snapshot.current_attempt.status = "failed".to_string();
        job.snapshot.current_attempt.summary = if cleaned.is_empty() {
            format!("{} failed: {}", plan.action.as_str(), failures.join(" | "))
        } else {
            format!(
                "{} partially failed after cleaning {} retired vector-space namespace(s): {}",
                plan.action.as_str(),
                cleaned.len(),
                failures.join(" | ")
            )
        };

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

fn source_action_content_type_matches_visual_unit(
    content_type: &str,
    visual_unit_kind: &str,
) -> bool {
    match content_type {
        "image" => visual_unit_kind == "image",
        "document" => visual_unit_kind == "document_page",
        "video" => visual_unit_kind == "video_segment",
        "text" => visual_unit_kind == "text",
        _ => false,
    }
}

fn is_terminal_job_status(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "canceled")
}

fn restore_canceled_source_action_roots(
    state: &mut AppState,
    library_id: &str,
    scope: &SourceActionScope,
    action: SourceActionKind,
    job_id: &str,
    summary: &str,
) {
    let Some(library) = state.libraries.get_mut(library_id) else {
        return;
    };

    let target_root_ids = match scope {
        SourceActionScope::Library => library.source_root_order.clone(),
        SourceActionScope::SourceRoot(source_root_id) => vec![source_root_id.clone()],
    };

    for source_root_id in target_root_ids {
        let Some(root) = library.source_roots.get_mut(&source_root_id) else {
            continue;
        };
        root.watch_state = source_root_watch_state(
            root.enabled,
            &SourceRootScanResult {
                status: root.status.clone(),
                observed_entries: root.observed_entries.clone(),
                error: root.pending_watch_error.clone(),
            },
            false,
        );
        root.last_action = Some(SourceRootLastAction {
            action: action.as_str().to_string(),
            status: "canceled".to_string(),
            summary: summary.to_string(),
            job_id: Some(job_id.to_string()),
        });
    }
}
