use super::*;
use crate::*;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::json;
use std::future::Future;
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
            library_id: None,
            display_name: None,
            name: "durable-roundtrip".to_string(),
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

    let active_alias = stable_vector_space_name(&library.id, &available_vector_space_id());
    let active_target = staging_vector_space_collection_name(
        &library.id,
        &available_vector_space_id(),
        "job_000001",
    );
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
        .active_vector_spaces
        .contains(&available_vector_space_id()));
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
            library_id: None,
            display_name: None,
            name: "restart-sequences".to_string(),
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
            ImportJobOutcome::completed(
                "indexed first image".to_string(),
                1,
                BTreeSet::from([available_vector_space_id()]),
            ),
        )
        .unwrap();

    let active_alias = stable_vector_space_name(&library.id, &available_vector_space_id());
    let active_target = staging_vector_space_collection_name(
        &library.id,
        &available_vector_space_id(),
        "job_000001",
    );
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
            library_id: None,
            display_name: None,
            name: "restart-sequences-2".to_string(),
        })
        .unwrap();
    assert_eq!(second_library.id, "restart-sequences-2");

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
fn durable_state_roundtrip_restores_archived_library_lifecycle() {
    let store_path = unique_test_file_path("durable-archived-library.sqlite");

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: Some("archived-library".to_string()),
            display_name: Some("Archived Library".to_string()),
            name: String::new(),
        })
        .unwrap();
    let archived = state.archive_library(&library.id).unwrap();

    assert_eq!(archived.lifecycle_state, "archived");
    assert!(archived.archived_at_ms.is_some());

    let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
    let loaded_library = loaded.libraries.get(&library.id).unwrap();

    assert_eq!(loaded_library.lifecycle_state, "archived");
    assert!(loaded_library.archived_at_ms.is_some());

    let _ = fs::remove_file(&store_path);
}

#[test]
fn restart_load_missing_collection_marks_index_not_ready() {
    let store_path = unique_test_file_path("restart-missing-collection.sqlite");
    let image_path = unique_test_file_path("restart-missing-collection.png");
    fs::write(&image_path, b"png").unwrap();

    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "restart-missing-collection".to_string(),
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
            ImportJobOutcome::completed(
                "indexed first image".to_string(),
                1,
                BTreeSet::from([available_vector_space_id()]),
            ),
        )
        .unwrap();

    let mut loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
    let loaded_library = loaded.libraries.get(&library.id).unwrap();
    assert!(!loaded_library
        .active_vector_spaces
        .contains(&available_vector_space_id()));

    let error = run_async(loaded.prepare_text_search(&TextSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        text: "chart".to_string(),
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
    .unwrap_err();
    assert_eq!(error.payload.code, "not_ready");

    let active_alias = stable_vector_space_name(&library.id, &available_vector_space_id());
    let active_target = staging_vector_space_collection_name(
        &library.id,
        &available_vector_space_id(),
        "job_000001",
    );
    let reloaded = load_state_with_qdrant_namespaces(
        &store_path,
        &[(active_alias, active_target.clone())],
        &[active_target],
    );
    assert!(!reloaded
        .libraries
        .get(&library.id)
        .unwrap()
        .active_vector_spaces
        .contains(&available_vector_space_id()));

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
            library_id: None,
            display_name: None,
            name: "restart-legacy-direct".to_string(),
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
            ImportJobOutcome::completed(
                "indexed first image".to_string(),
                1,
                BTreeSet::from([available_vector_space_id()]),
            ),
        )
        .unwrap();

    let legacy_direct_collection =
        stable_vector_space_name(&library.id, &available_vector_space_id());
    let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[legacy_direct_collection]);
    assert!(!loaded
        .libraries
        .get(&library.id)
        .unwrap()
        .active_vector_spaces
        .contains(&available_vector_space_id()));

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
            library_id: None,
            display_name: None,
            name: "restart-watcher".to_string(),
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
fn apply_config_backed_model_state_prunes_unconfigured_active_vector_spaces_marks_retired_and_persists(
) {
    let store_path = unique_test_file_path("config-prunes-vector-spaces.sqlite");
    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "config-prunes-vector-spaces".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());
    state.persist_durable_state().unwrap();

    let replacement_vector_space_id = vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "single_vector",
    );
    let config = FauniConfig {
        provider: BTreeMap::from([(
            LOCAL_SIDECAR_PROVIDER_ID.to_string(),
            ProviderConfigFileRecord {
                kind: LOCAL_SIDECAR_PROVIDER_KIND.to_string(),
                display_name: Some("Local Sidecar".to_string()),
                enabled: true,
                active_model: Some("athrael-soju/colqwen3.5-4.5B-v3".to_string()),
                base_url: None,
                models: BTreeMap::from([(
                    "athrael-soju/colqwen3.5-4.5B-v3".to_string(),
                    ProviderModelConfigRecord {
                        enabled: true,
                        version: "main".to_string(),
                        embedding_capabilities: EmbeddingCapabilities {
                            input_types: vec!["text".to_string(), "image".to_string()],
                            vector_types: vec![
                                "multi_vector_late_interaction".to_string(),
                                "single_vector".to_string(),
                            ],
                            supports_mixed_inputs: false,
                        },
                    },
                )]),
            },
        )]),
        content_types: BTreeMap::from([(
            "image".to_string(),
            ContentTypeConfigRecord {
                enabled: true,
                model: format!(
                    "{}/{}",
                    LOCAL_SIDECAR_PROVIDER_ID, "athrael-soju/colqwen3.5-4.5B-v3"
                ),
                vector_type: "single_vector".to_string(),
            },
        )]),
        libraries: BTreeMap::new(),
    };

    state.apply_config_backed_model_state(&config).unwrap();

    let active_vector_spaces = &state
        .libraries
        .get(&library.id)
        .unwrap()
        .active_vector_spaces;
    assert!(!active_vector_spaces.contains(&available_vector_space_id()));
    assert!(!active_vector_spaces.contains(&replacement_vector_space_id));
    let retired_vector_spaces = &state
        .libraries
        .get(&library.id)
        .unwrap()
        .retired_vector_spaces;
    assert!(retired_vector_spaces.contains_key(&available_vector_space_id()));
    assert!(!retired_vector_spaces.contains_key(&replacement_vector_space_id));

    let loaded = load_durable_state_snapshot(&store_path)
        .unwrap()
        .unwrap()
        .snapshot;
    assert!(loaded.libraries[&library.id]
        .active_vector_spaces
        .is_empty());
    assert!(loaded.libraries[&library.id]
        .retired_vector_spaces
        .contains_key(&available_vector_space_id()));

    let _ = fs::remove_file(store_path);
}

#[test]
fn source_root_refresh_activates_files_and_rule_update_moves_sources_out_of_scope() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "source-root-refresh".to_string(),
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
fn source_inventory_includes_representative_preview_when_visual_units_exist() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "source-inventory-preview".to_string(),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("source-inventory-preview");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("chart.png"), b"png").unwrap();
    write_test_pdf(&root_dir.join("report.pdf"), 2);

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
            SourceActionScope::SourceRoot(source_root.source_root_id),
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

    let sources = state
        .list_sources(
            &library.id,
            SourcesQuery {
                source_root_id: None,
                source_type: None,
                status: None,
            },
        )
        .unwrap();

    let image_source = sources
        .sources
        .iter()
        .find(|source| source.source_type == "image")
        .unwrap();
    let image_visual = image_source.representative_visual_unit.as_ref().unwrap();
    let image_preview = image_source.representative_preview.as_ref().unwrap();
    assert_eq!(image_visual.kind, "image");
    assert!(image_preview
        .url
        .contains(&format!("/libraries/{}/visual-units/", library.id)));
    assert!(image_preview.url.ends_with("/preview"));

    let document_source = sources
        .sources
        .iter()
        .find(|source| source.source_type == "pdf")
        .unwrap();
    let document_visual = document_source.representative_visual_unit.as_ref().unwrap();
    let document_preview = document_source.representative_preview.as_ref().unwrap();
    assert_eq!(document_visual.kind, "document_page");
    assert_eq!(document_visual.locator["page"], 1);
    assert!(document_preview
        .url
        .contains(&format!("/libraries/{}/visual-units/", library.id)));

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn source_root_refresh_marks_deleted_files_invalidated() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "source-root-invalidation".to_string(),
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

    let search_plan = run_async(state.prepare_text_search(&TextSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        text: "chart".to_string(),
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
    .unwrap();
    assert!(search_plan.active_visual_unit_refs.is_empty());

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn finalize_import_job_failed_with_activations_keeps_structured_state_and_active_vector_space() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "import-partial-vector-space".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("import-partial-vector-space.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .retired_vector_spaces
        .insert(
            available_vector_space_id(),
            RetiredVectorSpaceRecord {
                retired_at_ms: current_unix_ms().saturating_sub(1),
            },
        );

    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::failed_with_activations(
                "failed",
                "vector_space image-space failed after document-space activation".to_string(),
                1,
                BTreeSet::from([available_vector_space_id()]),
            ),
        )
        .unwrap();

    let library = state.libraries.get(&library.id).unwrap();
    assert_eq!(library.sources.len(), 1);
    assert_eq!(library.visual_units.len(), 1);
    assert!(library
        .active_vector_spaces
        .contains(&available_vector_space_id()));
    assert!(!library
        .retired_vector_spaces
        .contains_key(&available_vector_space_id()));

    let job = state.jobs.get(&job_id).unwrap();
    assert_eq!(job.snapshot.status, "failed");
    assert_eq!(job.snapshot.phase, "failed");
    assert_eq!(job.snapshot.progress.completed, 1);

    let _ = fs::remove_file(image_path);
}

#[test]
fn finalize_import_job_preserves_visual_unit_progress_granularity() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "import-progress-granularity".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("import-progress-granularity.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();

    state.update_job_progress_snapshot(
        &job_id,
        "running",
        "stage_write",
        13,
        57,
        "visual_unit",
        "Writing batch 13/57 into staged vector-space storage.",
    );
    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::completed(
                "Accepted path; indexed visual units.".to_string(),
                1,
                BTreeSet::new(),
            ),
        )
        .unwrap();

    let job = state.jobs.get(&job_id).unwrap();
    assert_eq!(job.snapshot.status, "completed");
    assert_eq!(job.snapshot.progress.completed, 57);
    assert_eq!(job.snapshot.progress.total, 57);
    assert_eq!(job.snapshot.progress.unit, "visual_unit");

    let _ = fs::remove_file(image_path);
}

#[test]
fn request_job_cancellation_marks_queued_import_job_as_canceled() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "cancel-queued-import".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("cancel-queued-import.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();

    let canceled = state.request_job_cancellation(&job_id).unwrap();
    assert_eq!(canceled.status, "canceled");
    assert_eq!(canceled.phase, "canceled");
    assert!(!canceled.cancelable);
    assert!(state.job_cancellation_requested(&job_id));

    let _ = fs::remove_file(image_path);
}

#[test]
fn request_job_cancellation_rejects_terminal_jobs() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "cancel-terminal-import".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("cancel-terminal-import.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();
    state
        .finalize_import_job(
            &job_id,
            prepared,
            ImportJobOutcome::completed("done".to_string(), 1, BTreeSet::new()),
        )
        .unwrap();

    let error = state.request_job_cancellation(&job_id).unwrap_err();
    assert_eq!(error.payload.code, "conflict");

    let _ = fs::remove_file(image_path);
}

#[test]
fn request_job_retry_requeues_canceled_manual_source_action() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "retry-canceled-source-action".to_string(),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("retry-canceled-source-action");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("chart.png"), b"png").unwrap();

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

    let (response, _) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    let job_id = response.job_handle.unwrap();
    state.request_job_cancellation(&job_id).unwrap();

    let (retried, dispatch) = state.request_job_retry(&job_id).unwrap();
    assert_ne!(retried.job_id, job_id);
    assert_eq!(retried.status, "queued");
    assert!(retried.cancelable);
    assert!(retried.retryable);
    assert_eq!(retried.current_attempt.attempt, 2);
    assert_eq!(
        retried.retried_from_job_id.as_deref(),
        Some(job_id.as_str())
    );
    assert_eq!(
        state
            .libraries
            .get(&library.id)
            .unwrap()
            .latest_job_id
            .as_deref(),
        Some(retried.job_id.as_str())
    );
    assert_eq!(state.jobs.get(&job_id).unwrap().snapshot.status, "canceled");

    match dispatch {
        RetryJobDispatch::Import(_) => {
            panic!("expected source-action retry dispatch");
        }
        RetryJobDispatch::SourceAction(queued_action) => {
            assert_eq!(queued_action.job_id, retried.job_id);
            assert_eq!(queued_action.plan.action, SourceActionKind::Refresh);
            assert_eq!(
                queued_action.plan.target_root_ids,
                vec![source_root.source_root_id.clone()]
            );
        }
        RetryJobDispatch::Maintenance(_) => {
            panic!("expected source-action retry dispatch");
        }
    }

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn request_job_retry_requeues_canceled_import_job() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "retry-import".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("retry-import.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();
    state.request_job_cancellation(&job_id).unwrap();

    let (retried, dispatch) = state.request_job_retry(&job_id).unwrap();
    assert_ne!(retried.job_id, job_id);
    assert_eq!(retried.status, "queued");
    assert!(retried.retryable);
    assert_eq!(retried.current_attempt.attempt, 2);
    assert_eq!(
        retried.retried_from_job_id.as_deref(),
        Some(job_id.as_str())
    );

    match dispatch {
        RetryJobDispatch::Import(prepared) => {
            assert_eq!(
                prepared.request.paths,
                vec![image_path.to_string_lossy().to_string()]
            );
        }
        RetryJobDispatch::SourceAction(_) | RetryJobDispatch::Maintenance(_) => {
            panic!("expected import retry dispatch");
        }
    }

    let _ = fs::remove_file(image_path);
}

#[test]
fn request_job_resume_reopens_canceled_import_job() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "resume-import".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("resume-import.png");
    fs::write(&image_path, b"png").unwrap();

    let prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let queued = state.queue_import(&prepared).unwrap();
    let job_id = queued.job_handle.clone().unwrap();
    state.request_job_cancellation(&job_id).unwrap();

    let (resumed, dispatch) = state.request_job_resume(&job_id).unwrap();
    assert_eq!(resumed.job_id, job_id);
    assert_eq!(resumed.status, "queued");
    assert!(resumed.cancelable);
    assert!(resumed.retryable);
    assert_eq!(resumed.current_attempt.attempt, 2);
    assert_eq!(
        state
            .libraries
            .get(&library.id)
            .unwrap()
            .latest_job_id
            .as_deref(),
        Some(job_id.as_str())
    );

    match dispatch {
        ResumeJobDispatch::Import(prepared) => {
            assert_eq!(
                prepared.request.paths,
                vec![image_path.to_string_lossy().to_string()]
            );
        }
        ResumeJobDispatch::SourceAction(_) | ResumeJobDispatch::Maintenance(_) => {
            panic!("expected import resume dispatch");
        }
    }

    let _ = fs::remove_file(image_path);
}

#[test]
fn request_job_resume_reopens_canceled_manual_source_action() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "resume-canceled-source-action".to_string(),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("resume-canceled-source-action");
    fs::create_dir_all(&root_dir).unwrap();
    fs::write(root_dir.join("chart.png"), b"png").unwrap();

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

    let (response, _) = state
        .queue_source_action(
            &library.id,
            SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )
        .unwrap();
    let job_id = response.job_handle.unwrap();
    state.request_job_cancellation(&job_id).unwrap();

    let (resumed, dispatch) = state.request_job_resume(&job_id).unwrap();
    assert_eq!(resumed.job_id, job_id);
    assert_eq!(resumed.status, "queued");
    assert!(resumed.cancelable);
    assert_eq!(resumed.current_attempt.attempt, 2);
    assert_eq!(
        state
            .libraries
            .get(&library.id)
            .unwrap()
            .source_roots
            .get(&source_root.source_root_id)
            .unwrap()
            .watch_state,
        "queued_refresh"
    );

    match dispatch {
        ResumeJobDispatch::Import(_) | ResumeJobDispatch::Maintenance(_) => {
            panic!("expected source-action resume dispatch");
        }
        ResumeJobDispatch::SourceAction(plan) => {
            assert_eq!(plan.action, SourceActionKind::Refresh);
            assert_eq!(
                plan.target_root_ids,
                vec![source_root.source_root_id.clone()]
            );
        }
    }

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn request_job_resume_reopens_failed_maintenance_job() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "resume-maintenance".to_string(),
        })
        .unwrap();

    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .retired_vector_spaces
        .insert(
            "vs_retired_resume".to_string(),
            RetiredVectorSpaceRecord { retired_at_ms: 1 },
        );

    let (_, queued_action) = state
        .queue_maintenance_action(
            &library.id,
            MaintenanceActionKind::CleanupRetiredVectorSpaces,
        )
        .unwrap();
    let queued_action = queued_action.expect("maintenance should queue");
    state
        .finalize_maintenance_action_job(
            &queued_action.job_id,
            &queued_action.plan,
            &[],
            &[String::from("vs_retired_resume: cleanup failed")],
        )
        .unwrap();

    let (resumed, dispatch) = state.request_job_resume(&queued_action.job_id).unwrap();
    assert_eq!(resumed.job_id, queued_action.job_id);
    assert_eq!(resumed.status, "queued");
    assert!(resumed.cancelable);
    assert_eq!(resumed.current_attempt.attempt, 2);

    match dispatch {
        ResumeJobDispatch::Import(_) | ResumeJobDispatch::SourceAction(_) => {
            panic!("expected maintenance resume dispatch");
        }
        ResumeJobDispatch::Maintenance(plan) => {
            assert_eq!(
                plan.action,
                MaintenanceActionKind::CleanupRetiredVectorSpaces
            );
            assert_eq!(
                plan.target_vector_space_ids,
                vec![String::from("vs_retired_resume")]
            );
        }
    }
}

#[test]
fn finalize_import_job_reuses_existing_manual_source_path_without_duplicates() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "manual-import-reuse".to_string(),
        })
        .unwrap();

    let image_path = unique_test_file_path("manual-import-reuse.png");
    fs::write(&image_path, b"png").unwrap();

    let first_prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    let first_job = state.queue_import(&first_prepared).unwrap();
    let first_job_id = first_job.job_handle.unwrap();
    state
        .finalize_import_job(
            &first_job_id,
            first_prepared,
            ImportJobOutcome::completed("done".to_string(), 1, BTreeSet::new()),
        )
        .unwrap();

    let library_snapshot = state.libraries.get(&library.id).unwrap();
    let original_source = library_snapshot.sources.values().next().unwrap().clone();
    let original_visual_unit_id = original_source.visual_unit_ids[0].clone();
    let original_point_id = library_snapshot
        .visual_units
        .get(&original_visual_unit_id)
        .unwrap()
        .point_id;

    let second_prepared = state
        .prepare_import(
            &library.id,
            ImportPathsRequest {
                paths: vec![image_path.to_string_lossy().to_string()],
            },
        )
        .unwrap();
    assert_eq!(second_prepared.sources[0].id, original_source.id);
    assert!(second_prepared
        .vector_space_batches
        .iter()
        .any(|batch| batch.stale_point_ids.contains(&original_point_id)));

    let second_visual_unit_id = second_prepared.visual_units[0].id.clone();
    let second_job = state.queue_import(&second_prepared).unwrap();
    let second_job_id = second_job.job_handle.unwrap();
    state
        .finalize_import_job(
            &second_job_id,
            second_prepared,
            ImportJobOutcome::completed("done again".to_string(), 1, BTreeSet::new()),
        )
        .unwrap();

    let library_snapshot = state.libraries.get(&library.id).unwrap();
    assert_eq!(library_snapshot.sources.len(), 1);
    assert_eq!(library_snapshot.source_order.len(), 1);
    assert_eq!(library_snapshot.visual_units.len(), 1);
    assert_eq!(library_snapshot.visual_unit_order.len(), 1);
    assert_eq!(
        library_snapshot
            .sources
            .get(&original_source.id)
            .unwrap()
            .visual_unit_ids,
        vec![second_visual_unit_id.clone()]
    );
    assert!(!library_snapshot
        .visual_units
        .contains_key(&original_visual_unit_id));
    assert!(library_snapshot
        .visual_units
        .contains_key(&second_visual_unit_id));

    let _ = fs::remove_file(image_path);
}

#[test]
fn finalize_source_action_job_failed_with_structured_changes_persists_mutations() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "source-action-partial-vector-space".to_string(),
        })
        .unwrap();

    let root_dir = unique_test_dir_path("source-action-partial-vector-space");
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
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .retired_vector_spaces
        .insert(
            available_vector_space_id(),
            RetiredVectorSpaceRecord {
                retired_at_ms: current_unix_ms().saturating_sub(1),
            },
        );

    state
        .finalize_source_action_job(
            &queued.job_id,
            prepared,
            SourceActionJobOutcome::failed_with_structured_changes(
                SourceActionKind::Refresh,
                1,
                BTreeSet::from([available_vector_space_id()]),
                "vector_space image-space failed after document-space activation".to_string(),
            ),
        )
        .unwrap();

    let library = state.libraries.get(&library.id).unwrap();
    assert_eq!(library.sources.len(), 1);
    assert_eq!(library.visual_units.len(), 1);
    assert!(library
        .active_vector_spaces
        .contains(&available_vector_space_id()));
    assert!(!library
        .retired_vector_spaces
        .contains_key(&available_vector_space_id()));

    let root = library
        .source_roots
        .get(&source_root.source_root_id)
        .unwrap();
    assert_eq!(root.watch_state, "watching");
    assert_eq!(root.last_action.as_ref().unwrap().status, "failed");

    let job = state.jobs.get(&queued.job_id).unwrap();
    assert_eq!(job.snapshot.status, "failed");
    assert_eq!(job.snapshot.phase, "failed");
    assert_eq!(job.snapshot.progress.completed, 1);

    let _ = fs::remove_dir_all(root_dir);
}

#[test]
fn retired_vector_space_cleanup_candidates_and_persistence_follow_retention_window() {
    let store_path = unique_test_file_path("retired-vector-space-cleanup.sqlite");
    let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "retired-vector-space-cleanup".to_string(),
        })
        .unwrap();
    let old_vector_space_id = available_vector_space_id();
    let fresh_vector_space_id = vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "single_vector",
    );
    let still_active_vector_space_id = vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "independent_vectors",
    );
    let now_ms = current_unix_ms();
    {
        let library = state.libraries.get_mut(&library.id).unwrap();
        library.retired_vector_spaces.insert(
            old_vector_space_id.clone(),
            RetiredVectorSpaceRecord {
                retired_at_ms: now_ms.saturating_sub(crate::RETIRED_VECTOR_SPACE_RETENTION_MS + 1),
            },
        );
        library.retired_vector_spaces.insert(
            fresh_vector_space_id.clone(),
            RetiredVectorSpaceRecord {
                retired_at_ms: now_ms,
            },
        );
        library.retired_vector_spaces.insert(
            still_active_vector_space_id.clone(),
            RetiredVectorSpaceRecord {
                retired_at_ms: now_ms.saturating_sub(crate::RETIRED_VECTOR_SPACE_RETENTION_MS + 1),
            },
        );
        library
            .active_vector_spaces
            .insert(still_active_vector_space_id.clone());
    }
    state.persist_durable_state().unwrap();

    let candidates = state.eligible_retired_vector_spaces_for_cleanup(now_ms);
    assert_eq!(
        candidates,
        vec![RetiredVectorSpaceCleanupCandidate {
            library_id: library.id.clone(),
            vector_space_id: old_vector_space_id.clone(),
        }]
    );

    state
        .forget_cleaned_retired_vector_spaces(&candidates)
        .unwrap();

    let library = state.libraries.get(&library.id).unwrap();
    assert!(!library
        .retired_vector_spaces
        .contains_key(&old_vector_space_id));
    assert!(library
        .retired_vector_spaces
        .contains_key(&fresh_vector_space_id));
    assert!(library
        .retired_vector_spaces
        .contains_key(&still_active_vector_space_id));

    let loaded = load_durable_state_snapshot(&store_path)
        .unwrap()
        .unwrap()
        .snapshot;
    let retired_vector_spaces = &loaded.libraries[&library.id].retired_vector_spaces;
    assert!(!retired_vector_spaces.contains_key(&old_vector_space_id));
    assert!(retired_vector_spaces.contains_key(&fresh_vector_space_id));
    assert!(retired_vector_spaces.contains_key(&still_active_vector_space_id));

    let _ = fs::remove_file(store_path);
}

#[test]
fn get_vector_space_diagnostics_reports_active_and_retired_spaces() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "vector-space-diagnostics".to_string(),
        })
        .unwrap();
    let retired_vector_space_id = vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "single_vector",
    );
    {
        let library = state.libraries.get_mut(&library.id).unwrap();
        library
            .active_vector_spaces
            .insert(available_vector_space_id());
        library.retired_vector_spaces.insert(
            retired_vector_space_id.clone(),
            RetiredVectorSpaceRecord {
                retired_at_ms: 1234,
            },
        );
    }

    let diagnostics = run_async(state.get_vector_space_diagnostics(&library.id)).unwrap();
    assert_eq!(diagnostics.vector_spaces.len(), 2);

    let active = diagnostics
        .vector_spaces
        .iter()
        .find(|item| item.lifecycle_state == "active")
        .unwrap();
    assert_eq!(active.vector_space_id, available_vector_space_id());
    assert_eq!(
        active.content_types,
        vec![
            "document".to_string(),
            "image".to_string(),
            "video".to_string()
        ]
    );
    assert_eq!(
        active.provider_id.as_deref(),
        Some(LOCAL_SIDECAR_PROVIDER_ID)
    );
    assert_eq!(
        active.model_id.as_deref(),
        Some("athrael-soju/colqwen3.5-4.5B-v3")
    );
    assert_eq!(active.model_version.as_deref(), Some("main"));
    assert_eq!(
        active.vector_type.as_deref(),
        Some("multi_vector_late_interaction")
    );
    assert_eq!(active.retired_at_ms, None);

    let retired = diagnostics
        .vector_spaces
        .iter()
        .find(|item| item.lifecycle_state == "retired")
        .unwrap();
    assert_eq!(retired.vector_space_id, retired_vector_space_id);
    assert!(retired.content_types.is_empty());
    assert_eq!(retired.provider_id, None);
    assert_eq!(retired.model_id, None);
    assert_eq!(retired.vector_type, None);
    assert_eq!(retired.retired_at_ms, Some(1234));
}

#[test]
fn watcher_poll_debounces_into_incremental_refresh_queue() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "watcher-refresh".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "disabled-source-root".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "ready-search".to_string(),
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
                    "Accepted 2 path(s); indexed 3 visual unit(s) into the active vector_space namespace."
                        .to_string(),
                    2,
                    BTreeSet::from([available_vector_space_id()]),
                ),
            )
            .unwrap();

    let plan = run_async(state.prepare_text_search(&TextSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        text: "report".to_string(),
        filters: None,
        top_k: Some(10),
        cursor: None,
        debug: Some(true),
        target_content_types: None,
    }))
    .unwrap();

    let response = build_search_response(
        plan,
        executed_search_groups_for_library(
            &library.id,
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
        ),
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
    assert_eq!(
        response.debug.as_ref().unwrap()["vector_type"],
        "multi_vector_late_interaction"
    );
    assert_eq!(
        response.debug.as_ref().unwrap()["vector_spaces"][0]["vector_space_id"],
        available_vector_space_id()
    );
    assert_eq!(
        response.debug.as_ref().unwrap()["vector_spaces"][0]["provider_kind"],
        LOCAL_SIDECAR_PROVIDER_KIND
    );
    assert_eq!(
        response.debug.as_ref().unwrap()["vector_spaces"][0]["provider_id"],
        LOCAL_SIDECAR_PROVIDER_ID
    );

    let _ = fs::remove_file(pdf_path);
    let _ = fs::remove_file(image_path);
}

#[test]
fn build_search_response_supports_cursor_pagination() {
    set_test_app_env();
    let plan = SearchPlan {
        search_scope_kind: "library".to_string(),
        library_id: "lib_000001".to_string(),
        top_k: 1,
        cursor_offset: 0,
        kind_filter: None,
        path_prefix_filter: None,
        source_type_filter: None,
        time_range_filter: None,
        target_content_types: vec!["image".to_string(), "document".to_string()],
        unsupported_content_types: Vec::new(),
        active_visual_unit_refs: scoped_visual_unit_refs("lib_000001", &["vu_000001", "vu_000002"]),
        execution_groups: available_execution_groups(&["image", "document"]),
        debug_content_types: default_search_debug_content_types("lib_000001"),
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
        executed_search_groups(&["image", "document"], candidates.clone()),
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
        executed_search_groups(&["image", "document"], candidates),
    )
    .unwrap();
    assert_eq!(second_page.results.len(), 1);
    assert_eq!(second_page.results[0].visual_unit_id, "vu_000002");
    assert_eq!(second_page.results[0].cursor, "search:v1:2");
    assert_eq!(second_page.next_cursor, None);
}

#[test]
fn build_search_response_merges_multiple_vector_spaces_by_score() {
    set_test_app_env();
    let secondary_vector_space_id = vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "single_vector",
    );
    let mut resolved_content_models = default_resolved_content_models();
    if let Some(document) = resolved_content_models.get_mut("document") {
        document.vector_type = "single_vector".to_string();
        document.vector_space_id = Some(secondary_vector_space_id.clone());
    }
    let plan = SearchPlan {
        search_scope_kind: "library".to_string(),
        library_id: "lib_000001".to_string(),
        top_k: 10,
        cursor_offset: 0,
        kind_filter: None,
        path_prefix_filter: None,
        source_type_filter: None,
        time_range_filter: None,
        target_content_types: vec!["image".to_string(), "document".to_string()],
        unsupported_content_types: Vec::new(),
        active_visual_unit_refs: scoped_visual_unit_refs("lib_000001", &["vu_000001", "vu_000002"]),
        execution_groups: vec![
            VectorSpaceExecutionGroup {
                library_id: "lib_000001".to_string(),
                vector_space_id: available_vector_space_id(),
                active_visual_unit_count: 2,
                content_types: vec!["image".to_string()],
                resolved_model: available_provider_selection(),
            },
            VectorSpaceExecutionGroup {
                library_id: "lib_000001".to_string(),
                vector_space_id: secondary_vector_space_id.clone(),
                active_visual_unit_count: 2,
                content_types: vec!["document".to_string()],
                resolved_model: ResolvedExecutionModelSelection {
                    vector_type: "single_vector".to_string(),
                    vector_space_id: secondary_vector_space_id.clone(),
                    summary: available_provider_selection().summary,
                    execution_input_types: vec!["text".to_string(), "image".to_string()],
                },
            },
        ],
        debug_content_types: resolved_content_models
            .into_iter()
            .map(
                |(content_type, resolved_model)| SearchContentTypeDebugEntry {
                    library_id: "lib_000001".to_string(),
                    content_type,
                    resolved_model,
                },
            )
            .collect(),
        debug: true,
    };

    let response = build_search_response(
        plan,
        vec![
            crate::indexing::ExecutedSearchGroup {
                library_id: "lib_000001".to_string(),
                query_embedding: QueryEmbeddingResult {
                    vectors: vec![vec![0.1, 0.2, 0.3]],
                    pooled_vector: vec![0.1, 0.2, 0.3],
                },
                candidates: vec![QdrantScoredPoint {
                    score: 0.7,
                    payload: Some(QdrantPointPayload {
                        visual_unit_id: "vu_000001".to_string(),
                        source_id: "src_000001".to_string(),
                        source_path: "/library/chart.png".to_string(),
                        source_type: "image".to_string(),
                        kind: "image".to_string(),
                        locator: json!({ "path": "/library/chart.png" }),
                    }),
                }],
            },
            crate::indexing::ExecutedSearchGroup {
                library_id: "lib_000001".to_string(),
                query_embedding: QueryEmbeddingResult {
                    vectors: vec![vec![0.3, 0.2, 0.1]],
                    pooled_vector: vec![0.3, 0.2, 0.1],
                },
                candidates: vec![QdrantScoredPoint {
                    score: 0.9,
                    payload: Some(QdrantPointPayload {
                        visual_unit_id: "vu_000002".to_string(),
                        source_id: "src_000002".to_string(),
                        source_path: "/library/report.pdf".to_string(),
                        source_type: "pdf".to_string(),
                        kind: "document_page".to_string(),
                        locator: json!({ "page": 1, "page_label": "1" }),
                    }),
                }],
            },
        ],
    )
    .unwrap();

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].visual_unit_id, "vu_000002");
    assert_eq!(response.results[1].visual_unit_id, "vu_000001");
    assert_eq!(
        response.debug.as_ref().unwrap()["vector_spaces"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn build_search_response_applies_path_prefix_kind_source_type_and_time_range_filters() {
    set_test_app_env();
    let response = build_search_response(
        SearchPlan {
            search_scope_kind: "library".to_string(),
            library_id: "lib_000001".to_string(),
            top_k: 10,
            cursor_offset: 0,
            kind_filter: Some(BTreeSet::from(["video_segment".to_string()])),
            path_prefix_filter: Some(BTreeSet::from(["/library/videos/".to_string()])),
            source_type_filter: Some(BTreeSet::from(["video".to_string()])),
            time_range_filter: Some(SearchTimeRangeFilter {
                start_ms: 600,
                end_ms: 1400,
            }),
            target_content_types: vec!["video".to_string()],
            unsupported_content_types: Vec::new(),
            active_visual_unit_refs: scoped_visual_unit_refs(
                "lib_000001",
                &["vu_000001", "vu_000002", "vu_000003"],
            ),
            execution_groups: available_execution_groups(&["video"]),
            debug_content_types: default_search_debug_content_types("lib_000001"),
            debug: false,
        },
        executed_search_groups(
            &["video"],
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
        ),
    )
    .unwrap();

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].visual_unit_id, "vu_000001");
    assert_eq!(response.results[0].kind, "video_segment");
    assert_eq!(response.results[0].source_type, "video");
    assert_eq!(response.next_cursor, None);
}

#[test]
fn prepare_import_groups_visual_units_by_vector_space() {
    let pdf_path = unique_test_file_path("prepare-import-grouped.pdf");
    let image_path = unique_test_file_path("prepare-import-grouped.png");
    write_test_pdf(&pdf_path, 2);
    fs::write(&image_path, b"png").unwrap();

    let mut state = test_state();
    state.provider_embedding_capabilities.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        EmbeddingCapabilities {
            input_types: vec!["text".to_string(), "image".to_string()],
            vector_types: vec![
                "multi_vector_late_interaction".to_string(),
                "single_vector".to_string(),
            ],
            supports_mixed_inputs: false,
        },
    );
    state.global_content_types.insert(
        "document".to_string(),
        ContentTypeConfigRecord {
            enabled: true,
            model: format!(
                "{}/{}",
                LOCAL_SIDECAR_PROVIDER_ID, "athrael-soju/colqwen3.5-4.5B-v3"
            ),
            vector_type: "single_vector".to_string(),
        },
    );
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "prepare-import-grouped".to_string(),
        })
        .unwrap();

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

    assert_eq!(prepared.vector_space_batches.len(), 2);
    assert!(prepared
        .vector_space_batches
        .iter()
        .any(|batch| batch.visual_units.iter().all(|item| item.kind == "image")));
    assert!(prepared.vector_space_batches.iter().any(|batch| batch
        .visual_units
        .iter()
        .all(|item| item.kind == "document_page")));

    let _ = fs::remove_file(pdf_path);
    let _ = fs::remove_file(image_path);
}

#[test]
fn prepare_text_search_rejects_invalid_cursor_and_time_range_filter() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "search-filter-validation".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

    let invalid_cursor = run_async(state.prepare_text_search(&TextSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        text: "report".to_string(),
        filters: None,
        top_k: Some(5),
        cursor: Some("bogus-cursor".to_string()),
        debug: Some(false),
        target_content_types: None,
    }))
    .unwrap_err();
    assert_eq!(invalid_cursor.payload.code, "validation_failed");
    assert_eq!(
        invalid_cursor.payload.details.as_ref().unwrap()["field"],
        "cursor"
    );

    let invalid_time_range = run_async(state.prepare_text_search(&TextSearchRequest {
        search_scope: None,
        library_id: Some(library.id),
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
        target_content_types: None,
    }))
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "image-search".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (plan, temp_asset) = run_async(state.prepare_image_search(&ImageSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        image_input: QueryImageInputRequest {
            kind: "temp_asset".to_string(),
            temp_asset_id: Some(asset.temp_asset_id.clone()),
            visual_unit_id: None,
        },
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(true),
        target_content_types: None,
    }))
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

    let missing = run_async(state.prepare_image_search(&ImageSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        image_input: QueryImageInputRequest {
            kind: "temp_asset".to_string(),
            temp_asset_id: Some("temp_asset_999999".to_string()),
            visual_unit_id: None,
        },
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(missing.payload.code, "not_found");

    let _ = fs::remove_file(image_path);
}

#[test]
fn prepare_image_search_accepts_library_image_objects() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "image-search-library-object".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (plan, input) = run_async(state.prepare_image_search(&ImageSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        image_input: QueryImageInputRequest {
            kind: "library_object".to_string(),
            temp_asset_id: None,
            visual_unit_id: Some(visual_unit_id),
        },
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "document-page-query-object".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (plan, input) = run_async(state.prepare_image_search(&ImageSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        image_input: QueryImageInputRequest {
            kind: "library_object".to_string(),
            temp_asset_id: None,
            visual_unit_id: Some(visual_unit_id),
        },
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "unsupported-query-object".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_image_search(&ImageSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
        image_input: QueryImageInputRequest {
            kind: "library_object".to_string(),
            temp_asset_id: None,
            visual_unit_id: Some(visual_unit_id),
        },
        filters: None,
        top_k: Some(5),
        cursor: None,
        debug: Some(false),
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "not_supported");
    let details = error.payload.details.unwrap();
    assert_eq!(details["supported_kinds"][0], "image");
    assert_eq!(details["supported_kinds"][1], "document_page");
}

#[test]
fn get_temp_query_asset_rejects_expired_assets() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "expired-query-image".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "prune-expired-query-image".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "prune-missing-query-image".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "video-import".to_string(),
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "video-search".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (plan, temp_input) = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
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

    let (_, library_input) = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "video-range-errors".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_rejects_expired_temp_assets() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "expired-query-video".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "not_found");

    let _ = fs::remove_file(video_path);
}

#[test]
fn prepare_video_search_rejects_non_video_library_sources() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "unsupported-video-query-source".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "not_supported");
    let details = error.payload.details.unwrap();
    assert_eq!(details["supported_source_type"], "video");
    assert_eq!(details["received_source_type"], "image");
}

#[test]
fn prepare_video_search_accepts_library_video_segments() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "video-segment-query".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (_, input) = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap();

    assert_eq!(input.path, "/tmp/example.mp4");
    assert_eq!(input.locator.unwrap(), locator);
}

#[test]
fn prepare_video_search_rejects_locator_override_for_library_video_segments() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "video-segment-query-locator".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_video_search(&VideoSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");
}

#[test]
fn prepare_document_search_accepts_temp_assets_and_library_sources() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "document-search".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let (plan, temp_input) = run_async(state.prepare_document_search(&DocumentSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
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

    let (_, library_input) = run_async(state.prepare_document_search(&DocumentSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
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
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "document-range-errors".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_document_search(&DocumentSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "validation_failed");

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_document_search_rejects_expired_temp_assets() {
    set_test_app_env();
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "expired-query-document".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_document_search(&DocumentSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
    .unwrap_err();

    assert_eq!(error.payload.code, "not_found");

    let _ = fs::remove_file(pdf_path);
}

#[test]
fn prepare_document_search_rejects_non_pdf_library_sources() {
    let mut state = test_state();
    let library = state
        .create_library(CreateLibraryRequest {
            library_id: None,
            display_name: None,
            name: "unsupported-document-query-source".to_string(),
        })
        .unwrap();
    state
        .libraries
        .get_mut(&library.id)
        .unwrap()
        .active_vector_spaces
        .insert(available_vector_space_id());

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

    let error = run_async(state.prepare_document_search(&DocumentSearchRequest {
        search_scope: None,
        library_id: Some(library.id.clone()),
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
        target_content_types: None,
    }))
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

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn test_state() -> AppState {
    let mut state = AppState::default();
    state.global_content_types = BTreeMap::from([
        (
            "image".to_string(),
            ContentTypeConfigRecord {
                enabled: true,
                model: format!(
                    "{}/{}",
                    LOCAL_SIDECAR_PROVIDER_ID, "athrael-soju/colqwen3.5-4.5B-v3"
                ),
                vector_type: "multi_vector_late_interaction".to_string(),
            },
        ),
        (
            "document".to_string(),
            ContentTypeConfigRecord {
                enabled: true,
                model: format!(
                    "{}/{}",
                    LOCAL_SIDECAR_PROVIDER_ID, "athrael-soju/colqwen3.5-4.5B-v3"
                ),
                vector_type: "multi_vector_late_interaction".to_string(),
            },
        ),
        (
            "video".to_string(),
            ContentTypeConfigRecord {
                enabled: true,
                model: format!(
                    "{}/{}",
                    LOCAL_SIDECAR_PROVIDER_ID, "athrael-soju/colqwen3.5-4.5B-v3"
                ),
                vector_type: "multi_vector_late_interaction".to_string(),
            },
        ),
    ]);
    seed_available_provider_probe_cache(&mut state);
    state
}

fn seed_available_provider_probe_cache(state: &mut AppState) {
    state.provider_probe_cache.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        ProviderProbeSnapshot {
            status: "available".to_string(),
            message: "local_sidecar probe seeded for unit tests".to_string(),
            last_probed_at: None,
        },
    );
    state.provider_runtime_models.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        ProviderRuntimeModelSnapshot {
            model_id: "athrael-soju/colqwen3.5-4.5B-v3".to_string(),
            model_revision: Some("main".to_string()),
        },
    );
    state.provider_embedding_capabilities.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        local_sidecar_embedding_capabilities(),
    );
    state.provider_execution_input_types.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        local_sidecar_execution_input_types(),
    );
}

fn available_provider_selection() -> ResolvedExecutionModelSelection {
    ResolvedExecutionModelSelection {
        vector_type: "multi_vector_late_interaction".to_string(),
        vector_space_id: available_vector_space_id(),
        execution_input_types: local_sidecar_execution_input_types(),
        summary: ResolvedModelSelectionPayload {
            binding_source: "global_content_type".to_string(),
            provider_id: LOCAL_SIDECAR_PROVIDER_ID.to_string(),
            provider_kind: LOCAL_SIDECAR_PROVIDER_KIND.to_string(),
            model_id: "athrael-soju/colqwen3.5-4.5B-v3".to_string(),
            model_version: "main".to_string(),
            model_revision: Some("main".to_string()),
            embedding_capabilities: local_sidecar_embedding_capabilities(),
            status: "available".to_string(),
            message: "Resolved search model for tests.".to_string(),
            last_probed_at: None,
        },
    }
}

fn available_execution_groups(content_types: &[&str]) -> Vec<VectorSpaceExecutionGroup> {
    vec![VectorSpaceExecutionGroup {
        library_id: "lib_000001".to_string(),
        vector_space_id: available_vector_space_id(),
        active_visual_unit_count: 3,
        content_types: content_types
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        resolved_model: available_provider_selection(),
    }]
}

fn executed_search_groups(
    _content_types: &[&str],
    candidates: Vec<QdrantScoredPoint>,
) -> Vec<crate::indexing::ExecutedSearchGroup> {
    executed_search_groups_for_library("lib_000001", candidates)
}

fn executed_search_groups_for_library(
    library_id: &str,
    candidates: Vec<QdrantScoredPoint>,
) -> Vec<crate::indexing::ExecutedSearchGroup> {
    vec![crate::indexing::ExecutedSearchGroup {
        library_id: library_id.to_string(),
        query_embedding: QueryEmbeddingResult {
            vectors: vec![vec![0.1, 0.2, 0.3]],
            pooled_vector: vec![0.1, 0.2, 0.3],
        },
        candidates,
    }]
}

fn available_vector_space_id() -> String {
    vector_space_id(
        LOCAL_SIDECAR_PROVIDER_ID,
        "athrael-soju/colqwen3.5-4.5B-v3",
        "main",
        "multi_vector_late_interaction",
    )
}

fn default_resolved_content_models() -> BTreeMap<String, ResolvedContentModelSelectionPayload> {
    let summary = available_provider_selection().summary;
    BTreeMap::from([
        (
            "image".to_string(),
            ResolvedContentModelSelectionPayload {
                binding_source: "global_content_type".to_string(),
                content_type: "image".to_string(),
                provider_id: summary.provider_id.clone(),
                provider_kind: summary.provider_kind.clone(),
                model_id: summary.model_id.clone(),
                model_version: summary.model_version.clone(),
                model_revision: summary.model_revision.clone(),
                vector_type: "multi_vector_late_interaction".to_string(),
                vector_space_id: Some(available_vector_space_id()),
                embedding_capabilities: summary.embedding_capabilities.clone(),
                status: summary.status.clone(),
                message: summary.message.clone(),
                last_probed_at: summary.last_probed_at.clone(),
            },
        ),
        (
            "document".to_string(),
            ResolvedContentModelSelectionPayload {
                binding_source: "global_content_type".to_string(),
                content_type: "document".to_string(),
                provider_id: summary.provider_id.clone(),
                provider_kind: summary.provider_kind.clone(),
                model_id: summary.model_id.clone(),
                model_version: summary.model_version.clone(),
                model_revision: summary.model_revision.clone(),
                vector_type: "multi_vector_late_interaction".to_string(),
                vector_space_id: Some(available_vector_space_id()),
                embedding_capabilities: summary.embedding_capabilities.clone(),
                status: summary.status.clone(),
                message: summary.message.clone(),
                last_probed_at: summary.last_probed_at.clone(),
            },
        ),
        (
            "video".to_string(),
            ResolvedContentModelSelectionPayload {
                binding_source: "global_content_type".to_string(),
                content_type: "video".to_string(),
                provider_id: summary.provider_id.clone(),
                provider_kind: summary.provider_kind.clone(),
                model_id: summary.model_id.clone(),
                model_version: summary.model_version.clone(),
                model_revision: summary.model_revision,
                vector_type: "multi_vector_late_interaction".to_string(),
                vector_space_id: Some(available_vector_space_id()),
                embedding_capabilities: summary.embedding_capabilities,
                status: summary.status,
                message: summary.message,
                last_probed_at: summary.last_probed_at,
            },
        ),
    ])
}

fn default_search_debug_content_types(library_id: &str) -> Vec<SearchContentTypeDebugEntry> {
    default_resolved_content_models()
        .into_iter()
        .map(
            |(content_type, resolved_model)| SearchContentTypeDebugEntry {
                library_id: library_id.to_string(),
                content_type,
                resolved_model,
            },
        )
        .collect()
}

fn scoped_visual_unit_refs(library_id: &str, ids: &[&str]) -> BTreeSet<String> {
    ids.iter()
        .map(|visual_unit_id| format!("{library_id}:{visual_unit_id}"))
        .collect()
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
