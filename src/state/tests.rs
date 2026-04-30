use super::*;
use serde_json::json;

#[test]
fn image_classification_generates_reused_asset_and_unit_content() {
    let mut state = AppState::default();
    let classification = PathClassification {
        source_id: "src_000001".to_string(),
        normalized_path: "/tmp/image.png".to_string(),
        source_type: "image".to_string(),
        kind: "image".to_string(),
        page_count: None,
        duration_ms: None,
    };

    let (contents, locations, assets, units) =
        state.new_assets_and_units_from_classification(&classification, None);

    assert_eq!(contents.len(), 1);
    assert_eq!(locations.len(), 1);
    assert_eq!(assets.len(), 1);
    assert_eq!(units.len(), 1);
    assert_eq!(assets[0].source_content_id, contents[0].id);
    assert_eq!(locations[0].source_id, classification.source_id);
    assert_eq!(locations[0].asset_id, assets[0].id);
    assert_eq!(assets[0].unit_ids, vec![units[0].id.clone()]);
    assert_eq!(assets[0].asset_type, "image");
    assert_eq!(units[0].unit_type, "image");
}

#[test]
fn pdf_classification_separates_page_asset_from_page_image_unit() {
    let mut state = AppState::default();
    let classification = PathClassification {
        source_id: "src_000001".to_string(),
        normalized_path: "/tmp/report.pdf".to_string(),
        source_type: "pdf".to_string(),
        kind: "document_page".to_string(),
        page_count: Some(2),
        duration_ms: None,
    };

    let (contents, locations, assets, units) =
        state.new_assets_and_units_from_classification(&classification, None);

    assert_eq!(contents.len(), 1);
    assert_eq!(locations.len(), 2);
    assert_eq!(assets.len(), 2);
    assert_eq!(units.len(), 2);
    assert_eq!(assets[0].asset_type, "document_page");
    assert_eq!(units[0].unit_type, "page_image");
    assert_eq!(assets[0].locator["page"], 1);
    assert_eq!(assets[1].locator["page"], 2);
}

#[test]
fn completed_source_content_reuses_existing_assets_and_units() {
    let temp_path = std::env::temp_dir().join(format!(
        "fauni-source-content-reuse-{}-{}.png",
        std::process::id(),
        current_unix_ms()
    ));
    std::fs::write(&temp_path, b"same source bytes").unwrap();
    let path = temp_path.to_string_lossy().to_string();
    let mut state = AppState::default();
    let first_classification = PathClassification {
        source_id: "src_000001".to_string(),
        normalized_path: path.clone(),
        source_type: "image".to_string(),
        kind: "image".to_string(),
        page_count: None,
        duration_ms: None,
    };
    let (contents, _locations, assets, units) =
        state.new_assets_and_units_from_classification(&first_classification, None);
    let source_content_id = contents[0].id.clone();
    let asset_id = assets[0].id.clone();

    let mut library = LibraryRecord {
        id: "lib".to_string(),
        display_name: "Lib".to_string(),
        lifecycle_state: "active".to_string(),
        archived_at_ms: None,
        content_type_overrides: BTreeMap::new(),
        source_roots: BTreeMap::new(),
        source_root_order: Vec::new(),
        contents: contents
            .iter()
            .map(|content| (content.id.clone(), content.clone()))
            .collect(),
        sources: BTreeMap::new(),
        source_order: Vec::new(),
        source_asset_locations: BTreeMap::new(),
        source_asset_location_order: Vec::new(),
        assets: assets
            .iter()
            .map(|asset| (asset.id.clone(), asset.clone()))
            .collect(),
        asset_order: assets.iter().map(|asset| asset.id.clone()).collect(),
        units: units
            .iter()
            .map(|unit| (unit.id.clone(), unit.clone()))
            .collect(),
        unit_order: units.iter().map(|unit| unit.id.clone()).collect(),
        vector_spaces: BTreeMap::new(),
        unit_indexes: BTreeMap::new(),
        content_e2e_index_states: BTreeMap::new(),
        latest_job_id: None,
    };
    library.content_e2e_index_states.insert(
        ContentE2eIndexStateRecord::key(&source_content_id, "image:image:v1", "vs_1"),
        ContentE2eIndexStateRecord {
            content_id: source_content_id.clone(),
            pipe_signature: "image:image:v1".to_string(),
            vector_space_id: "vs_1".to_string(),
            indexed_at_ms: current_unix_ms(),
        },
    );
    state.libraries.insert("lib".to_string(), library);

    let second_classification = PathClassification {
        source_id: "src_000002".to_string(),
        normalized_path: path,
        source_type: "image".to_string(),
        kind: "image".to_string(),
        page_count: None,
        duration_ms: None,
    };
    let (reused_contents, reused_locations, reused_assets, reused_units) =
        state.new_assets_and_units_from_classification(&second_classification, Some("lib"));

    assert_eq!(reused_contents[0].id, source_content_id);
    assert_eq!(reused_assets.len(), 1);
    assert_eq!(reused_assets[0].id, asset_id);
    assert_eq!(reused_units.len(), 0);
    assert_eq!(reused_locations.len(), 1);
    assert_eq!(reused_locations[0].source_id, "src_000002");
    assert_eq!(reused_locations[0].asset_id, asset_id);

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn unit_index_key_is_unit_and_vector_space() {
    assert_eq!(
        UnitIndexRecord::key("unit_000001", "vs_abc"),
        "unit_000001::vs_abc"
    );
}

#[test]
fn retired_unit_indexes_are_cleanup_candidates_and_can_be_forgotten() {
    let mut state = AppState::default();
    let mut library = LibraryRecord {
        id: "lib".to_string(),
        display_name: "Lib".to_string(),
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
    library.unit_indexes.insert(
        UnitIndexRecord::key("unit_1", "vs_old"),
        UnitIndexRecord {
            unit_id: "unit_1".to_string(),
            vector_space_id: "vs_old".to_string(),
            status: "ready".to_string(),
            visibility: "retired".to_string(),
            vector_ref: Some(json!({ "point_id": 1 })),
            job_id: Some("job_1".to_string()),
            error_summary: None,
        },
    );
    state.library_order.push("lib".to_string());
    state.libraries.insert("lib".to_string(), library);

    let candidates = state.eligible_retired_vector_spaces_for_cleanup(current_unix_ms());
    assert_eq!(
        candidates,
        vec![RetiredVectorSpaceCleanupCandidate {
            library_id: "lib".to_string(),
            vector_space_id: "vs_old".to_string(),
        }]
    );

    state
        .forget_cleaned_retired_vector_spaces(&candidates)
        .unwrap();
    assert!(state.libraries["lib"].unit_indexes.is_empty());
}
