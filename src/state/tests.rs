use super::*;
use crate::*;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, process::Command};

#[test]
fn durable_state_roundtrip_restores_library_source_roots_sources_visual_units_and_active_index() {
    let store_path = unique_test_file_path("durable-roundtrip.sqlite");
    let root_dir = unique_test_dir_path("durable-roundtrip");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("chart.png"), b"png").unwrap();
    write_test_pdf(&root_dir.join("report.pdf"), 2);

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            name: "durable-roundtrip".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload {
                    include_globs: Vec::new(),
                    exclude_globs: Vec::new(),
                    include_extensions: vec!["png".to_string(), "pdf".to_string()],
                }),
            },
        )
        .unwrap();
    let (_, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    let queued = queued.unwrap();
    let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
    let outcome = SourceActionJobOutcome::completed(&prepared);
    state
        .finalize_source_action_job(&queued.job_id, prepared, outcome)
        .unwrap();

    let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
    let active_target = staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
    let loaded = load_state_with_qdrant_namespaces(
        &store_path,
        &[(active_alias, active_target.clone())],
        &[active_target],
    );
    let loaded_library = loaded.libraries.get(&library.id).unwrap();
    let loaded_root = loaded_library
        .source_roots
        .get(&source_root.source_root_id)
        .unwrap();

    assert_eq!(loaded.library_order, vec![library.id.clone()]);
    assert_eq!(
        loaded_library.source_root_order,
        vec![source_root.source_root_id]
    );
    assert_eq!(loaded_library.sources.len(), 2);
    assert_eq!(loaded_library.visual_units.len(), 3);
    assert!(loaded_library
        .active_index_lines
        .contains(MULTIVECTOR_INDEX_LINE));
    assert_eq!(
        loaded_root.rules.include_extensions,
        vec!["pdf".to_string(), "png".to_string()]
    );
    assert_eq!(loaded_root.watch_state, "watching");
    assert!(loaded.jobs.is_empty());
    assert!(loaded.job_order.is_empty());
    assert_eq!(loaded_library.latest_job_id, None);

    let _ = fs::remove_file(&store_path);
    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn restart_load_continues_id_sequences_and_clears_jobs() {
    let store_path = unique_test_file_path("restart-sequences.sqlite");
    let first_image = unique_test_file_path("restart-sequences-first.png");
    let second_image = unique_test_file_path("restart-sequences-second.png");
    fs::write(&first_image, b"png").unwrap();
    fs::write(&second_image, b"png").unwrap();
    let root_dir = unique_test_dir_path("restart-sequences-root");
    fs::create_dir_all(&root_dir).unwrap();

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            name: "restart-sequences".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    let root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();
    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![first_image.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let import_data = state.queue_import(&prepared).unwrap();
    let job_id = import_data.job_handle.clone().unwrap();
    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::completed("indexed first image".to_string(), 1),
        )
        .unwrap();

    let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
    let active_target = staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
    let mut loaded = load_state_with_qdrant_namespaces(
        &store_path,
        &[(active_alias, active_target.clone())],
        &[active_target],
    );
    let loaded_library = loaded.libraries.get(&library.id).unwrap();
    assert!(loaded.jobs.is_empty());
    assert!(loaded.job_order.is_empty());
    assert_eq!(loaded_library.latest_job_id, None);

    let second_library = loaded
        .create_library(CreateLibraryRequest {
            name: "restart-sequences-2".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    assert_eq!(second_library.id, "lib_000002");

    let second_root = loaded
        .create_source_root(
            &second_library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(false),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();
    assert_eq!(root.source_root_id, "root_000001");
    assert_eq!(second_root.source_root_id, "root_000002");

    let prepared = loaded
        .prepare_import(
            &second_library.id,
            ImportPathsRequest {
                paths: vec![second_image.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    assert_eq!(prepared.sources[0].id, "src_000002");
    assert_eq!(prepared.visual_units[0].id, "vu_000002");

    let _ = fs::remove_file(&store_path);
    let _ = fs::remove_file(first_image);
    let _ = fs::remove_file(second_image);
    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn restart_load_missing_collection_marks_index_not_ready() {
    let store_path = unique_test_file_path("restart-missing-collection.sqlite");
    let image_path = unique_test_file_path("restart-missing-collection.png");
    fs::write(&image_path, b"png").unwrap();

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            name: "restart-missing-collection".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let import_data = state.queue_import(&prepared).unwrap();
    let job_id = import_data.job_handle.clone().unwrap();
    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::completed("indexed first image".to_string(), 1),
        )
        .unwrap();

    let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
    let loaded_library = loaded.libraries.get(&library.id).unwrap();
    assert!(!loaded_library
        .active_index_lines
        .contains(MULTIVECTOR_INDEX_LINE));

    let error = loaded
        .prepare_text_search(&TextSearchRequest {
            library_id: library.id.clone(),
            text: "chart".to_string(),
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();
    assert_eq!(error.payload.code, "not_ready");

    let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
    let active_target = staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
    let reloaded = load_state_with_qdrant_namespaces(
        &store_path,
        &[(active_alias, active_target.clone())],
        &[active_target],
    );
    assert!(!reloaded
        .libraries
        .get(&library.id)
        .unwrap()
        .active_index_lines
        .contains(MULTIVECTOR_INDEX_LINE));

    let _ = fs::remove_file(&store_path);
    let _ = fs::remove_file(image_path);
}

#[test]
fn restart_load_legacy_direct_collection_marks_index_not_ready() {
    let store_path = unique_test_file_path("restart-legacy-direct.sqlite");
    let image_path = unique_test_file_path("restart-legacy-direct.png");
    fs::write(&image_path, b"png").unwrap();

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            name: "restart-legacy-direct".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let import_data = state.queue_import(&prepared).unwrap();
    let job_id = import_data.job_handle.clone().unwrap();
    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::completed("indexed first image".to_string(), 1),
        )
        .unwrap();

    let legacy_direct_collection = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
    let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[legacy_direct_collection]);
    assert!(!loaded
        .libraries
        .get(&library.id)
        .unwrap()
        .active_index_lines
        .contains(MULTIVECTOR_INDEX_LINE));

    let _ = fs::remove_file(&store_path);
    let _ = fs::remove_file(image_path);
}

#[test]
fn restart_load_reseeds_watcher_runtime_fields_without_auto_queueing_jobs() {
    let store_path = unique_test_file_path("restart-watcher.sqlite");
    let root_dir = unique_test_dir_path("restart-watcher-root");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("watch.png"), b"png").unwrap();

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            name: "restart-watcher".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();

    {
        let root = state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .source_roots
            .get_mut(&source_root.source_root_id)
            .unwrap();
        root.watch_state = "queued_refresh".to_string();
        root.pending_watch_paths.insert("watch.png".to_string());
        root.pending_watch_deadline_ms = Some(0);
        root.pending_watch_error = Some("stale watcher error".to_string());
        root.last_action = Some(SourceRootLastAction {
            action: "refresh".to_string(),
            status: "completed".to_string(),
            summary: "stale".to_string(),
            job_id: Some("job_999999".to_string()),
        });
    }
    state.persist_durable_state().unwrap();

    let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
    let loaded_root = loaded
        .libraries
        .get(&library.id)
        .unwrap()
        .source_roots
        .get(&source_root.source_root_id)
        .unwrap();
    assert_eq!(loaded_root.watch_state, "watching");
    assert!(loaded_root.pending_watch_paths.is_empty());
    assert_eq!(loaded_root.pending_watch_deadline_ms, None);
    assert_eq!(loaded_root.pending_watch_error, None);
    assert!(loaded_root.last_action.is_none());
    assert!(loaded.jobs.is_empty());
    assert!(loaded.job_order.is_empty());

    let _ = fs::remove_file(&store_path);
    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn source_root_refresh_activates_files_and_rule_update_moves_sources_out_of_scope() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "source-root-refresh".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("source-root-refresh");
    fs::create_dir_all(&root_dir).unwrap();
    let image_path = root_dir.join("chart.png");
    let pdf_path = root_dir.join("report.pdf");
    fs::write(&image_path, b"png").unwrap();
    write_test_pdf(&pdf_path, 2);

    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();

    let (action, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(action.accepted.len(), 1);
    let queued = queued.unwrap();
    let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
    assert_eq!(prepared.summary.activated_sources, 2);
    assert_eq!(prepared.summary.indexing_visual_units, 3);
    let outcome = SourceActionJobOutcome::completed(&prepared);
    state
        .finalize_source_action_job(&queued.job_id, prepared, outcome)
        .unwrap();

    let sources = state
        .list_sources(
            &library.id,
            SourcesQuery {
                source_root_id: Some(source_root.source_root_id.clone()),
                source_type: None,
                status: None,
            },
        )
        .unwrap();
    assert_eq!(sources.sources.len(), 2);
    assert!(sources
        .sources
        .iter()
        .all(|source| source.status == "active"));

    state
        .update_source_root(
            &library.id,
            &source_root.source_root_id,
            UpdateSourceRootRequest {
                root_path: None,
                enabled: None,
                rules: Some(SourceRootRulesPayload {
                    include_globs: Vec::new(),
                    exclude_globs: vec!["chart.png".to_string()],
                    include_extensions: Vec::new(),
                }),
            },
        )
        .unwrap();

    let (action, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(action.accepted.len(), 1);
    let queued = queued.unwrap();
    let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
    assert_eq!(prepared.summary.out_of_scope_sources, 1);
    let outcome = SourceActionJobOutcome::completed(&prepared);
    state
        .finalize_source_action_job(&queued.job_id, prepared, outcome)
        .unwrap();

    let active_sources = state
        .list_sources(
            &library.id,
            SourcesQuery {
                source_root_id: Some(source_root.source_root_id.clone()),
                source_type: None,
                status: Some("active".to_string()),
            },
        )
        .unwrap();
    assert_eq!(active_sources.sources.len(), 1);
    assert_eq!(active_sources.sources[0].source_type, "pdf");

    let out_of_scope_sources = state
        .list_sources(
            &library.id,
            SourcesQuery {
                source_root_id: Some(source_root.source_root_id),
                source_type: None,
                status: Some("out_of_scope".to_string()),
            },
        )
        .unwrap();
    assert_eq!(out_of_scope_sources.sources.len(), 1);
    assert_eq!(out_of_scope_sources.sources[0].source_type, "image");

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn source_root_refresh_marks_deleted_files_invalidated() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "source-root-invalidation".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("source-root-invalidation");
    fs::create_dir_all(&root_dir).unwrap();
    let image_path = root_dir.join("chart.png");
    fs::write(&image_path, b"png").unwrap();

    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();

    let (_, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    let queued = queued.unwrap();
    let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
    let outcome = SourceActionJobOutcome::completed(&prepared);
    state
        .finalize_source_action_job(&queued.job_id, prepared, outcome)
        .unwrap();

    fs::remove_file(&image_path).unwrap();

    let (_, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    let queued = queued.unwrap();
    let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
    assert_eq!(prepared.summary.invalidated_sources, 1);
    let outcome = SourceActionJobOutcome::completed(&prepared);
    state
        .finalize_source_action_job(&queued.job_id, prepared, outcome)
        .unwrap();

    let invalidated_sources = state
        .list_sources(
            &library.id,
            SourcesQuery {
                source_root_id: Some(source_root.source_root_id.clone()),
                source_type: None,
                status: Some("invalidated".to_string()),
            },
        )
        .unwrap();
    assert_eq!(invalidated_sources.sources.len(), 1);
    assert_eq!(
        invalidated_sources.sources[0].status_reason.as_deref(),
        Some("not_found")
    );

    let search_plan = state
        .prepare_text_search(&TextSearchRequest {
            library_id: library.id.clone(),
            text: "chart".to_string(),
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();
    assert!(search_plan.active_visual_unit_ids.is_empty());

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn watcher_poll_debounces_into_incremental_refresh_queue() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "watcher-refresh".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("watcher-refresh");
    fs::create_dir_all(&root_dir).unwrap();
    let image_path = root_dir.join("watch.png");

    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(true),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();

    fs::write(&image_path, b"png").unwrap();

    let queued = state.poll_source_root_watchers();
    assert!(queued.is_empty());
    let root = state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .source_roots
        .get_mut(&source_root.source_root_id)
        .unwrap();
    assert_eq!(root.watch_state, "queued_refresh");
    root.pending_watch_deadline_ms = Some(0);

    let queued = state.poll_source_root_watchers();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].plan.action.as_str(), "refresh");
    assert_eq!(
        queued[0].plan.target_root_ids,
        vec![source_root.source_root_id]
    );

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn disabled_source_root_skips_watcher_and_rejects_manual_refresh() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "disabled-source-root".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("disabled-source-root");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("disabled.png"), b"png").unwrap();

    let source_root = state
        .create_source_root(
            &library.id,
            CreateSourceRootRequest {
                root_path: root_dir.to_string_lossy().to_string(),
                enabled: Some(false),
                rules: Some(SourceRootRulesPayload::default()),
            },
        )
        .unwrap();

    let queued = state.poll_source_root_watchers();
    assert!(queued.is_empty());

    let root = state
        .libraries
        .get(&library.id)
        .unwrap()
        .source_roots
        .get(&source_root.source_root_id)
        .unwrap();
    assert_eq!(root.status, "disabled");
    assert_eq!(root.watch_state, "disabled");

    let (action, queued) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    assert!(queued.is_none());
    assert!(action.accepted.is_empty());
    assert_eq!(action.rejected.len(), 1);
    assert_eq!(action.rejected[0].reason_code, "not_enabled");

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn build_search_response_returns_qdrant_results_after_import() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "ready-search".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let pdf_path = unique_test_file_path("report.pdf");
    let image_path = unique_test_file_path("report-chart.png");
    write_test_pdf(&pdf_path, 2);
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![
                    pdf_path.to_string_lossy().to_string(),
                    image_path.to_string_lossy().to_string(),
                ],
            },
        )
        .unwrap();

    let image_visual_unit_id = prepared
        .accepted
        .iter()
        .find(|item| item.kind == "image")
        .unwrap()
        .visual_units[0]
        .visual_unit_id
        .clone();
    let document_visual_unit_id = prepared
        .accepted
        .iter()
        .find(|item| item.kind == "document_page")
        .unwrap()
        .visual_units[0]
        .visual_unit_id
        .clone();

    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();
    state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed(
                    "Accepted 2 path(s); indexed 3 visual unit(s) into the active multivector collection."
                        .to_string(),
                    2,
                ),
            )
            .unwrap();

    let plan = state
        .prepare_text_search(&TextSearchRequest {
            library_id: library.id.clone(),
            text: "report".to_string(),
            filters: None,
            top_k: Some(10),
            cursor: None,
            debug: Some(true),
            target_index_lines: None,
        })
        .unwrap();

    let response = build_search_response(
        plan,
        QueryEmbeddingResult {
            vectors: vec![vec![0.1, 0.2, 0.3], vec![0.3, 0.2, 0.1]],
            pooled_vector: vec![0.2, 0.2, 0.2],
        },
        vec![
            QdrantScoredPoint {
                score: 0.9,
                payload: Some(QdrantPointPayload {
                    visual_unit_id: image_visual_unit_id,
                    source_id: "src_000002".to_string(),
                    source_path: image_path.to_string_lossy().to_string(),
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    locator: json!({ "path": image_path.to_string_lossy().to_string() }),
                }),
            },
            QdrantScoredPoint {
                score: 0.8,
                payload: Some(QdrantPointPayload {
                    visual_unit_id: document_visual_unit_id,
                    source_id: "src_000001".to_string(),
                    source_path: pdf_path.to_string_lossy().to_string(),
                    source_type: "pdf".to_string(),
                    kind: "document_page".to_string(),
                    locator: json!({ "page": 1, "page_label": "1" }),
                }),
            },
        ],
    )
    .unwrap();

    assert_eq!(response.results.len(), 2);
    assert!(response
        .results
        .iter()
        .any(|item| item.kind == "document_page"));
    assert!(response.results.iter().any(|item| item.kind == "image"));
    assert_eq!(response.results[0].score, Some(0.9));
    assert_eq!(response.results[1].score, Some(0.8));
    assert_eq!(response.results[0].cursor, "search:v1:1");
    assert_eq!(response.results[1].cursor, "search:v1:2");
    assert_eq!(response.next_cursor, None);
    assert!(response.results.iter().all(|item| item
        .preview
        .url
        .starts_with("http://127.0.0.1:53210/libraries/")));
    assert_eq!(response.debug.as_ref().unwrap()["repr_kind"], "multivector");
    assert_eq!(
        response.debug.as_ref().unwrap()["provider"]["model_profile"],
        "local_python"
    );
    assert_eq!(
        response.debug.as_ref().unwrap()["index_lines"][0]["index_line"],
        "multivector"
    );

    let _ = fs::remove_file(pdf_path);
    let _ = fs::remove_file(image_path);
}

#[test]
fn build_search_response_supports_cursor_pagination() {
    set_test_app_env();
    let plan = SearchPlan {
        library_id: "lib_000001".to_string(),
        collection_name: stable_collection_name("lib_000001", MULTIVECTOR_INDEX_LINE),
        top_k: 1,
        cursor_offset: 0,
        kind_filter: None,
        path_prefix_filter: None,
        source_type_filter: None,
        time_range_filter: None,
        target_index_lines: vec![MULTIVECTOR_INDEX_LINE.to_string()],
        active_visual_unit_ids: BTreeSet::from([
            "vu_000001".to_string(),
            "vu_000002".to_string(),
        ]),
        debug: true,
    };
    let candidates = vec![
        QdrantScoredPoint {
            score: 0.9,
            payload: Some(QdrantPointPayload {
                visual_unit_id: "vu_000001".to_string(),
                source_id: "src_000001".to_string(),
                source_path: "/library/chart.png".to_string(),
                source_type: "image".to_string(),
                kind: "image".to_string(),
                locator: json!({ "path": "/library/chart.png" }),
            }),
        },
        QdrantScoredPoint {
            score: 0.8,
            payload: Some(QdrantPointPayload {
                visual_unit_id: "vu_000002".to_string(),
                source_id: "src_000002".to_string(),
                source_path: "/library/report.pdf".to_string(),
                source_type: "pdf".to_string(),
                kind: "document_page".to_string(),
                locator: json!({ "page": 1, "page_label": "1" }),
            }),
        },
    ];

    let first_page = build_search_response(
        SearchPlan {
            cursor_offset: 0,
            ..plan.clone()
        },
        QueryEmbeddingResult {
            vectors: vec![vec![0.1, 0.2, 0.3]],
            pooled_vector: vec![0.1, 0.2, 0.3],
        },
        candidates.clone(),
    )
    .unwrap();
    assert_eq!(first_page.results.len(), 1);
    assert_eq!(first_page.results[0].visual_unit_id, "vu_000001");
    assert_eq!(first_page.results[0].cursor, "search:v1:1");
    assert_eq!(first_page.next_cursor.as_deref(), Some("search:v1:1"));

    let second_page = build_search_response(
        SearchPlan {
            cursor_offset: 1,
            ..plan
        },
        QueryEmbeddingResult {
            vectors: vec![vec![0.1, 0.2, 0.3]],
            pooled_vector: vec![0.1, 0.2, 0.3],
        },
        candidates,
    )
    .unwrap();
    assert_eq!(second_page.results.len(), 1);
    assert_eq!(second_page.results[0].visual_unit_id, "vu_000002");
    assert_eq!(second_page.results[0].cursor, "search:v1:2");
    assert_eq!(second_page.next_cursor, None);
}

#[test]
fn build_search_response_applies_path_prefix_kind_source_type_and_time_range_filters() {
    set_test_app_env();
    let response = build_search_response(
        SearchPlan {
            library_id: "lib_000001".to_string(),
            collection_name: stable_collection_name("lib_000001", MULTIVECTOR_INDEX_LINE),
            top_k: 10,
            cursor_offset: 0,
            kind_filter: Some(BTreeSet::from(["video_segment".to_string()])),
            path_prefix_filter: Some(BTreeSet::from(["/library/videos/".to_string()])),
            source_type_filter: Some(BTreeSet::from(["video".to_string()])),
            time_range_filter: Some(SearchTimeRangeFilter {
                start_ms: 600,
                end_ms: 1400,
            }),
            target_index_lines: vec![MULTIVECTOR_INDEX_LINE.to_string()],
            active_visual_unit_ids: BTreeSet::from([
                "vu_000001".to_string(),
                "vu_000002".to_string(),
                "vu_000003".to_string(),
            ]),
            debug: false,
        },
        QueryEmbeddingResult {
            vectors: vec![vec![0.1, 0.2, 0.3]],
            pooled_vector: vec![0.1, 0.2, 0.3],
        },
        vec![
            QdrantScoredPoint {
                score: 0.9,
                payload: Some(QdrantPointPayload {
                    visual_unit_id: "vu_000001".to_string(),
                    source_id: "src_000001".to_string(),
                    source_path: "/library/videos/clip.mp4".to_string(),
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    locator: json!({ "start_ms": 500, "end_ms": 1500, "duration_ms": 3000 }),
                }),
            },
            QdrantScoredPoint {
                score: 0.8,
                payload: Some(QdrantPointPayload {
                    visual_unit_id: "vu_000002".to_string(),
                    source_id: "src_000002".to_string(),
                    source_path: "/library/videos/clip.mp4".to_string(),
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    locator: json!({ "start_ms": 1600, "end_ms": 2200, "duration_ms": 3000 }),
                }),
            },
            QdrantScoredPoint {
                score: 0.7,
                payload: Some(QdrantPointPayload {
                    visual_unit_id: "vu_000003".to_string(),
                    source_id: "src_000003".to_string(),
                    source_path: "/library/images/chart.png".to_string(),
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    locator: json!({ "path": "/library/images/chart.png" }),
                }),
            },
        ],
    )
    .unwrap();

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].visual_unit_id, "vu_000001");
    assert_eq!(response.results[0].kind, "video_segment");
    assert_eq!(response.results[0].source_type, "video");
    assert_eq!(response.next_cursor, None);
}

#[test]
fn prepare_text_search_rejects_invalid_cursor_and_time_range_filter() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "search-filter-validation".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let invalid_cursor = state
        .prepare_text_search(&TextSearchRequest {
            library_id: library.id.clone(),
            text: "report".to_string(),
            filters: None,
            top_k: Some(5),
            cursor: Some("bogus-cursor".to_string()),
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();
    assert_eq!(invalid_cursor.payload.code, "validation_failed");
    assert_eq!(
        invalid_cursor.payload.details.as_ref().unwrap()["field"],
        "cursor"
    );

    let invalid_time_range = state
        .prepare_text_search(&TextSearchRequest {
            library_id: library.id,
            text: "report".to_string(),
            filters: Some(json!({
                "time_range": {
                    "start_ms": 1000,
                    "end_ms": 1000,
                }
            })),
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();
    assert_eq!(invalid_time_range.payload.code, "validation_failed");
    assert_eq!(
        invalid_time_range.payload.details.as_ref().unwrap()["field"],
        "filters.time_range"
    );
}

#[test]
fn prepare_image_search_requires_existing_temp_asset() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "image-search".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let image_path = unique_test_file_path("query.png");
    fs::write(&image_path, b"png").unwrap();

    let staged = StagedQueryAsset {
        path: image_path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: "image/png".to_string(),
        original_filename: Some("query.png".to_string()),
        page_count: None,
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_asset(&library.id, staged)
        .unwrap();

    let (plan, temp_asset) = state
        .prepare_image_search(&ImageSearchRequest {
            library_id: library.id.clone(),
            image_input: QueryImageInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id.clone()),
                visual_unit_id: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(true),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(plan.library_id, library.id);
    match temp_asset {
        ResolvedImageQueryInput::TempAsset(temp_asset) => {
            assert_eq!(temp_asset.id, asset.temp_asset_id);
            assert_eq!(temp_asset.path, image_path.to_string_lossy().to_string());
        }
        ResolvedImageQueryInput::LibraryVisualUnit(_) => {
            panic!("expected temp query asset input")
        }
    }

    let missing = state
        .prepare_image_search(&ImageSearchRequest {
            library_id: library.id.clone(),
            image_input: QueryImageInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some("temp_asset_999999".to_string()),
                visual_unit_id: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(missing.payload.code, "not_found");

    let _ = fs::remove_file(image_path);
}

#[test]
fn prepare_image_search_accepts_library_image_objects() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "image-search-library-object".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let image_path = unique_test_file_path("library-query.png");
    fs::write(&image_path, b"png").unwrap();
    let classification = state
        .inspect_import_path(&image_path.to_string_lossy())
        .unwrap();
    let visual_unit = state
        .new_visual_units_from_classification(&classification)
        .into_iter()
        .next()
        .unwrap();
    let visual_unit_id = visual_unit.id.clone();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .visual_units
        .insert(visual_unit.id.clone(), visual_unit.clone());

    let (plan, input) = state
        .prepare_image_search(&ImageSearchRequest {
            library_id: library.id.clone(),
            image_input: QueryImageInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                visual_unit_id: Some(visual_unit_id),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(plan.library_id, library.id);
    match input {
        ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => {
            assert_eq!(visual_unit.kind, "image");
            assert_eq!(visual_unit.source_path, image_path.to_string_lossy());
        }
        ResolvedImageQueryInput::TempAsset(_) => {
            panic!("expected library visual unit query input")
        }
    }

    let _ = fs::remove_file(image_path);
}

#[test]
fn prepare_image_search_accepts_library_document_page_objects() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "document-page-query-object".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let pdf_path = unique_test_file_path("library-query-page.pdf");
    write_test_pdf(&pdf_path, 1);
    let classification = state
        .inspect_import_path(&pdf_path.to_string_lossy())
        .unwrap();
    let visual_unit = state
        .new_visual_units_from_classification(&classification)
        .into_iter()
        .next()
        .unwrap();
    let visual_unit_id = visual_unit.id.clone();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .visual_units
        .insert(visual_unit.id.clone(), visual_unit.clone());

    let (plan, input) = state
        .prepare_image_search(&ImageSearchRequest {
            library_id: library.id.clone(),
            image_input: QueryImageInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                visual_unit_id: Some(visual_unit_id),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(plan.library_id, library.id);
    match input {
        ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => {
            assert_eq!(visual_unit.kind, "document_page");
            assert_eq!(visual_unit.locator["page"], 1);
        }
        ResolvedImageQueryInput::TempAsset(_) => {
            panic!("expected library visual unit query input")
        }
    }

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_image_search_rejects_unsupported_library_object_query_images() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "unsupported-query-object".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let visual_unit_id = "vu_video_000001".to_string();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .visual_units
        .insert(
            visual_unit_id.clone(),
            VisualUnitRecord {
                id: visual_unit_id.clone(),
                point_id: 1,
                source_id: "src_video_000001".to_string(),
                source_path: "/tmp/example.mp4".to_string(),
                source_type: "video".to_string(),
                kind: "video_segment".to_string(),
                locator: json!({ "start_ms": 0, "end_ms": 1000 }),
                neighbor_context: json!({}),
            },
        );

    let error = state
        .prepare_image_search(&ImageSearchRequest {
            library_id: library.id.clone(),
            image_input: QueryImageInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                visual_unit_id: Some(visual_unit_id),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "not_supported");
    let details = error.payload.details.unwrap();
    assert_eq!(details["supported_kinds"][0], "image");
    assert_eq!(details["supported_kinds"][1], "document_page");
}

#[test]
fn get_temp_query_asset_rejects_expired_assets() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "expired-query-image".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let image_path = unique_test_file_path("expired-query.png");
    fs::write(&image_path, b"png").unwrap();
    let staged = StagedQueryAsset {
        path: image_path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: "image/png".to_string(),
        original_filename: Some("expired-query.png".to_string()),
        page_count: None,
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_asset(&library.id, staged)
        .unwrap();
    state
        .temp_query_assets
        .get_mut(&asset.temp_asset_id)
        .unwrap()
        .expires_at_ms = 0;

    let error = state
        .get_temp_query_asset(&library.id, &asset.temp_asset_id)
        .unwrap_err();

    assert_eq!(error.payload.code, "not_found");
    assert_eq!(
        error.payload.message,
        "Query image was not found or has expired."
    );

    let _ = fs::remove_file(image_path);
}

#[test]
fn prune_temp_query_assets_removes_expired_asset_records_and_files() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "prune-expired-query-image".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let image_path = unique_test_file_path("expired-prune-query.png");
    fs::write(&image_path, b"png").unwrap();
    let staged = StagedQueryAsset {
        path: image_path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: "image/png".to_string(),
        original_filename: Some("expired-prune-query.png".to_string()),
        page_count: None,
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_asset(&library.id, staged)
        .unwrap();
    state
        .temp_query_assets
        .get_mut(&asset.temp_asset_id)
        .unwrap()
        .expires_at_ms = 0;

    let summary = state.prune_temp_query_assets();

    assert_eq!(summary.expired_removed, 1);
    assert_eq!(summary.missing_removed, 0);
    assert!(!state.temp_query_assets.contains_key(&asset.temp_asset_id));
    assert!(!image_path.exists());
}

#[test]
fn prune_temp_query_assets_removes_missing_asset_records() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "prune-missing-query-image".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let image_path = unique_test_file_path("missing-prune-query.png");
    fs::write(&image_path, b"png").unwrap();
    let staged = StagedQueryAsset {
        path: image_path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: "image/png".to_string(),
        original_filename: Some("missing-prune-query.png".to_string()),
        page_count: None,
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_asset(&library.id, staged)
        .unwrap();
    fs::remove_file(&image_path).unwrap();

    let summary = state.prune_temp_query_assets();

    assert_eq!(summary.expired_removed, 0);
    assert_eq!(summary.missing_removed, 1);
    assert!(!state.temp_query_assets.contains_key(&asset.temp_asset_id));
}

#[test]
fn import_paths_accepts_video_and_generates_video_segments() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "video-import".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();

    let video_path = unique_test_file_path("fixture.mp4");
    write_test_video(&video_path, 2.5);

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![video_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();

    assert_eq!(prepared.sources.len(), 1);
    assert_eq!(prepared.sources[0].source_type, "video");
    assert_eq!(prepared.accepted.len(), 1);
    assert_eq!(prepared.accepted[0].source_type, "video");
    assert_eq!(prepared.accepted[0].kind, "video_segment");
    assert_eq!(prepared.accepted[0].visual_units.len(), 1);
    assert_eq!(prepared.accepted[0].visual_units[0].source_id, "src_000001");
    assert_eq!(prepared.accepted[0].visual_units[0].locator["start_ms"], 0);
    assert_eq!(
        prepared.accepted[0].visual_units[0].locator["duration_ms"],
        2500
    );
    assert_eq!(
        prepared.accepted[0].source_id.as_deref(),
        Some("src_000001")
    );

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_accepts_temp_assets_and_library_sources() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "video-search".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let video_path = unique_test_file_path("query.mp4");
    write_test_video(&video_path, 3.0);
    let staged = StagedQueryAsset {
        path: video_path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: "video/mp4".to_string(),
        original_filename: Some("query.mp4".to_string()),
        page_count: None,
        duration_ms: Some(3000),
    };
    let asset = state
        .register_temp_query_video_asset(&library.id, staged)
        .unwrap();

    let (plan, temp_input) = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id.clone()),
                source_id: None,
                visual_unit_id: None,
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();
    assert_eq!(plan.library_id, library.id);
    assert_eq!(temp_input.path, video_path.to_string_lossy());
    assert!(temp_input.locator.is_none());

    let classification = state
        .inspect_import_path(&video_path.to_string_lossy())
        .unwrap();
    let source = state.source_record_from_classification(&classification, Vec::new());
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .sources
        .insert(source.id.clone(), source.clone());

    let (_, library_input) = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: Some(source.id),
                visual_unit_id: None,
                locator: Some(json!({ "start_ms": 500, "end_ms": 1500 })),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(library_input.path, video_path.to_string_lossy());
    assert_eq!(
        library_input.locator.unwrap(),
        json!({ "start_ms": 500, "end_ms": 1500, "duration_ms": 3000 })
    );

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_rejects_invalid_ranges() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "video-range-errors".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let video_path = unique_test_file_path("invalid-range.mp4");
    write_test_video(&video_path, 2.0);
    let staged = StagedQueryAsset {
        path: video_path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: "video/mp4".to_string(),
        original_filename: Some("invalid-range.mp4".to_string()),
        page_count: None,
        duration_ms: Some(2000),
    };
    let asset = state
        .register_temp_query_video_asset(&library.id, staged)
        .unwrap();

    let error = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id),
                source_id: None,
                visual_unit_id: None,
                locator: Some(json!({ "start_ms": 1500, "end_ms": 2500 })),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_rejects_expired_temp_assets() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "expired-query-video".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let video_path = unique_test_file_path("expired-query.mp4");
    write_test_video(&video_path, 2.0);
    let staged = StagedQueryAsset {
        path: video_path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: "video/mp4".to_string(),
        original_filename: Some("expired-query.mp4".to_string()),
        page_count: None,
        duration_ms: Some(2000),
    };
    let asset = state
        .register_temp_query_video_asset(&library.id, staged)
        .unwrap();
    state
        .temp_query_assets
        .get_mut(&asset.temp_asset_id)
        .unwrap()
        .expires_at_ms = 0;

    let error = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id),
                source_id: None,
                visual_unit_id: None,
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "not_found");

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_rejects_non_video_library_sources() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "unsupported-video-query-source".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .sources
        .insert(
            "src_image_000001".to_string(),
            SourceRecord {
                id: "src_image_000001".to_string(),
                source_root_id: None,
                source_root_path: None,
                source_path: "/tmp/example.png".to_string(),
                relative_path: None,
                source_type: "image".to_string(),
                kind: "image".to_string(),
                status: "active".to_string(),
                status_reason: None,
                page_count: None,
                duration_ms: None,
                observed_size_bytes: None,
                observed_modified_at_ms: None,
                visual_unit_ids: Vec::new(),
            },
        );

    let error = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: Some("src_image_000001".to_string()),
                visual_unit_id: None,
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "not_supported");
    let details = error.payload.details.unwrap();
    assert_eq!(details["supported_source_type"], "video");
    assert_eq!(details["received_source_type"], "image");
}

#[test]
fn prepare_video_search_accepts_library_video_segments() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "video-segment-query".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let visual_unit_id = "vu_video_000123".to_string();
    let locator = json!({ "start_ms": 600, "end_ms": 1800, "duration_ms": 3000 });
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .visual_units
        .insert(
            visual_unit_id.clone(),
            VisualUnitRecord {
                id: visual_unit_id.clone(),
                point_id: 1,
                source_id: "src_video_000123".to_string(),
                source_path: "/tmp/example.mp4".to_string(),
                source_type: "video".to_string(),
                kind: "video_segment".to_string(),
                locator: locator.clone(),
                neighbor_context: json!({}),
            },
        );

    let (_, input) = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: None,
                visual_unit_id: Some(visual_unit_id),
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(input.path, "/tmp/example.mp4");
    assert_eq!(input.locator.unwrap(), locator);
}

#[test]
fn prepare_video_search_rejects_locator_override_for_library_video_segments() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "video-segment-query-locator".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let visual_unit_id = "vu_video_000124".to_string();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .visual_units
        .insert(
            visual_unit_id.clone(),
            VisualUnitRecord {
                id: visual_unit_id.clone(),
                point_id: 1,
                source_id: "src_video_000124".to_string(),
                source_path: "/tmp/example.mp4".to_string(),
                source_type: "video".to_string(),
                kind: "video_segment".to_string(),
                locator: json!({ "start_ms": 600, "end_ms": 1800, "duration_ms": 3000 }),
                neighbor_context: json!({}),
            },
        );

    let error = state
        .prepare_video_search(&VideoSearchRequest {
            library_id: library.id.clone(),
            video_input: QueryVideoInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: None,
                visual_unit_id: Some(visual_unit_id),
                locator: Some(json!({ "start_ms": 0, "end_ms": 1000 })),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");
}

#[test]
fn prepare_document_search_accepts_temp_assets_and_library_sources() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "document-search".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let pdf_path = unique_test_file_path("query-document.pdf");
    write_test_pdf(&pdf_path, 3);
    let staged = StagedQueryAsset {
        path: pdf_path.to_string_lossy().to_string(),
        source_type: "pdf".to_string(),
        content_type: "application/pdf".to_string(),
        original_filename: Some("query-document.pdf".to_string()),
        page_count: Some(3),
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_document_asset(&library.id, staged)
        .unwrap();

    let (plan, temp_input) = state
        .prepare_document_search(&DocumentSearchRequest {
            library_id: library.id.clone(),
            document_input: QueryDocumentInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id.clone()),
                source_id: None,
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();
    assert_eq!(plan.library_id, library.id);
    assert_eq!(temp_input.path, pdf_path.to_string_lossy());
    assert!(temp_input.locator.is_none());

    let classification = state
        .inspect_import_path(&pdf_path.to_string_lossy())
        .unwrap();
    let source = state.source_record_from_classification(&classification, Vec::new());
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .sources
        .insert(source.id.clone(), source.clone());

    let (_, library_input) = state
        .prepare_document_search(&DocumentSearchRequest {
            library_id: library.id.clone(),
            document_input: QueryDocumentInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: Some(source.id),
                locator: Some(json!({ "start_page": 2, "end_page": 3 })),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap();

    assert_eq!(library_input.path, pdf_path.to_string_lossy());
    assert_eq!(
        library_input.locator.unwrap(),
        json!({ "start_page": 2, "end_page": 3, "page_count": 3 })
    );

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_document_search_rejects_invalid_ranges() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "document-range-errors".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let pdf_path = unique_test_file_path("invalid-document-range.pdf");
    write_test_pdf(&pdf_path, 2);
    let staged = StagedQueryAsset {
        path: pdf_path.to_string_lossy().to_string(),
        source_type: "pdf".to_string(),
        content_type: "application/pdf".to_string(),
        original_filename: Some("invalid-document-range.pdf".to_string()),
        page_count: Some(2),
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_document_asset(&library.id, staged)
        .unwrap();

    let error = state
        .prepare_document_search(&DocumentSearchRequest {
            library_id: library.id.clone(),
            document_input: QueryDocumentInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id),
                source_id: None,
                locator: Some(json!({ "start_page": 2, "end_page": 5 })),
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_document_search_rejects_expired_temp_assets() {
    set_test_app_env();
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "expired-query-document".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    let pdf_path = unique_test_file_path("expired-query-document.pdf");
    write_test_pdf(&pdf_path, 2);
    let staged = StagedQueryAsset {
        path: pdf_path.to_string_lossy().to_string(),
        source_type: "pdf".to_string(),
        content_type: "application/pdf".to_string(),
        original_filename: Some("expired-query-document.pdf".to_string()),
        page_count: Some(2),
        duration_ms: None,
    };
    let asset = state
        .register_temp_query_document_asset(&library.id, staged)
        .unwrap();
    state
        .temp_query_assets
        .get_mut(&asset.temp_asset_id)
        .unwrap()
        .expires_at_ms = 0;

    let error = state
        .prepare_document_search(&DocumentSearchRequest {
            library_id: library.id.clone(),
            document_input: QueryDocumentInputRequest {
                kind: "temp_asset".to_string(),
                temp_asset_id: Some(asset.temp_asset_id),
                source_id: None,
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "not_found");

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_document_search_rejects_non_pdf_library_sources() {
    let mut state = AppState::default();
    let library = state
        .create_library(CreateLibraryRequest {
            name: "unsupported-document-query-source".to_string(),
            config: Some(CreateLibraryConfigRequest {
                enabled_index_lines: vec!["multivector".to_string()],
            }),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_index_lines
        .insert(MULTIVECTOR_INDEX_LINE.to_string());

    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .sources
        .insert(
            "src_image_000002".to_string(),
            SourceRecord {
                id: "src_image_000002".to_string(),
                source_root_id: None,
                source_root_path: None,
                source_path: "/tmp/example.png".to_string(),
                relative_path: None,
                source_type: "image".to_string(),
                kind: "image".to_string(),
                status: "active".to_string(),
                status_reason: None,
                page_count: None,
                duration_ms: None,
                observed_size_bytes: None,
                observed_modified_at_ms: None,
                visual_unit_ids: Vec::new(),
            },
        );

    let error = state
        .prepare_document_search(&DocumentSearchRequest {
            library_id: library.id.clone(),
            document_input: QueryDocumentInputRequest {
                kind: "library_object".to_string(),
                temp_asset_id: None,
                source_id: Some("src_image_000002".to_string()),
                locator: None,
            },
            filters: None,
            top_k: Some(5),
            cursor: None,
            debug: Some(false),
            target_index_lines: None,
        })
        .unwrap_err();

    assert_eq!(error.payload.code, "not_supported");
    let details = error.payload.details.unwrap();
    assert_eq!(details["supported_source_type"], "pdf");
    assert_eq!(details["received_source_type"], "image");
}

#[test]
fn chunk_qdrant_points_splits_large_batches_by_request_size() {
    let point = json!({
        "id": 1,
        "vector": {
            "mv": vec![vec![0.1_f32; 32]; 8],
            "prefetch_dense": vec![0.1_f32; 32],
        },
        "payload": {
            "visual_unit_id": "vu_000001",
            "source_path": "/tmp/demo.png",
            "source_type": "image",
            "kind": "image",
            "locator": { "path": "/tmp/demo.png" },
        }
    });

    let single_size = serde_json::to_vec(&point).unwrap().len();
    let max_body_bytes = QDRANT_UPSERT_BODY_OVERHEAD_BYTES + (single_size * 2) + 1;
    let chunks = chunk_qdrant_points(
        vec![point.clone(), point.clone(), point.clone()],
        max_body_bytes,
    )
    .unwrap();

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].len(), 2);
    assert_eq!(chunks[1].len(), 1);

    for chunk in chunks {
        let body_len = serde_json::to_vec(&json!({ "points": chunk }))
            .unwrap()
            .len();
        assert!(body_len <= max_body_bytes);
    }
}

#[test]
fn build_qdrant_collection_create_payload_sets_on_disk_and_init_from() {
    let payload = build_qdrant_collection_create_payload(96, Some("index_stage_src"));

    assert_eq!(payload["vectors"]["mv"]["size"], 96);
    assert_eq!(payload["vectors"]["mv"]["on_disk"], true);
    assert_eq!(payload["vectors"]["prefetch_dense"]["size"], 96);
    assert_eq!(payload["vectors"]["prefetch_dense"]["on_disk"], true);
    assert_eq!(payload["init_from"]["collection"], "index_stage_src");
}

fn unique_test_file_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("fauni-search-{stamp}-{name}"))
}

fn unique_test_dir_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("fauni-search-{stamp}-{name}"))
}

fn simulated_active_namespace_probe(
    alias_name: &str,
    alias_targets: &BTreeMap<String, String>,
    existing_collections: &BTreeSet<String>,
) -> ActiveNamespaceProbeResult {
    if let Some(target_collection) = alias_targets.get(alias_name) {
        if existing_collections.contains(target_collection) {
            return ActiveNamespaceProbeResult::Ready {
                target_collection: target_collection.clone(),
            };
        }
        return ActiveNamespaceProbeResult::MissingTarget {
            target_collection: target_collection.clone(),
        };
    }
    if existing_collections.contains(alias_name) {
        ActiveNamespaceProbeResult::LegacyDirectCollection
    } else {
        ActiveNamespaceProbeResult::Missing
    }
}

fn load_state_with_qdrant_namespaces(
    store_path: &std::path::Path,
    alias_targets: &[(String, String)],
    existing_collections: &[String],
) -> AppState {
    let alias_targets = alias_targets.iter().cloned().collect::<BTreeMap<_, _>>();
    let existing_collections = existing_collections
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(AppState::load_from_durable_store_path_with_probe(
            Some(store_path.to_path_buf()),
            move |collection_name| {
                let probe = simulated_active_namespace_probe(
                    collection_name,
                    &alias_targets,
                    &existing_collections,
                );
                async move { Ok(probe) }
            },
        ))
        .unwrap()
}

fn write_test_pdf(path: &std::path::Path, page_count: usize) {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let resources_id = document.add_object(dictionary! {});

    let mut page_refs = Vec::new();
    for _ in 0..page_count {
        let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page_id = document.new_object_id();
        let page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
            "Contents" => content_id,
            "Resources" => resources_id,
        };
        document.objects.insert(page_id, Object::Dictionary(page));
        page_refs.push(Object::Reference(page_id));
    }

    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_refs,
            "Count" => page_count as i64,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);
    document.compress();
    document.save(path).unwrap();
}

fn write_test_video(path: &std::path::Path, duration_secs: f64) {
    let duration_arg = format!("{duration_secs:.3}");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=640x360:r=30",
            "-t",
            &duration_arg,
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn set_test_app_env() {
    std::env::set_var("APP_HOST", "127.0.0.1");
    std::env::set_var("APP_PORT", "53210");
}
