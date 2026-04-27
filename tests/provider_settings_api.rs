mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::fs;
use support::{MultipartFile, TestEnv};

const LOCAL_SIDECAR_PROVIDER_ID: &str = "local_sidecar";
const DASHSCOPE_PROVIDER_ID: &str = "dashscope";
const DEFAULT_MODEL_ID: &str = "athrael-soju/colqwen3.5-4.5B-v3";
const DASHSCOPE_MODEL_ID: &str = "qwen3-vl-embedding";

async fn create_library(app: &support::TestApp, name: &str) -> String {
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": name
            }),
        )
        .await
        .json();
    library["data"]["id"].as_str().unwrap().to_string()
}

fn runtime_config_json(env: &TestEnv) -> serde_json::Value {
    let path = env.runtime_dir.join("runtime-config.json");
    serde_json::from_str(
        &fs::read_to_string(&path).expect("runtime-config.json should be readable"),
    )
    .expect("runtime-config.json should contain valid json")
}

fn settings_model_test_file_count(env: &TestEnv) -> usize {
    let root = env.runtime_dir.join("settings-model-tests");
    if !root.exists() {
        return 0;
    }

    fn count_files(path: &std::path::Path) -> usize {
        fs::read_dir(path)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.flatten())
            .map(|entry| entry.path())
            .map(|path| if path.is_dir() { count_files(&path) } else { 1 })
            .sum()
    }

    count_files(&root)
}

fn dashscope_content_type_payload() -> serde_json::Value {
    json!({
        "content_types": {
            "image": {
                "enabled": true,
                "model": format!("{DASHSCOPE_PROVIDER_ID}/{DASHSCOPE_MODEL_ID}"),
                "vector_type": "single_vector"
            },
            "document": {
                "enabled": true,
                "model": format!("{DASHSCOPE_PROVIDER_ID}/{DASHSCOPE_MODEL_ID}"),
                "vector_type": "single_vector"
            },
            "video": {
                "enabled": false,
                "model": format!("{DASHSCOPE_PROVIDER_ID}/{DASHSCOPE_MODEL_ID}"),
                "vector_type": "single_vector"
            },
            "text": {
                "enabled": false,
                "model": format!("{DASHSCOPE_PROVIDER_ID}/{DASHSCOPE_MODEL_ID}"),
                "vector_type": "single_vector"
            }
        }
    })
}

#[tokio::test]
async fn provider_settings_bootstrap_global_content_types_and_resolved_content_models() {
    let env = TestEnv::new("provider-settings-global-content-types").await;
    let app = env.boot().await;

    let providers = app.get_json("/settings/providers").await;
    assert_eq!(providers.status, StatusCode::OK);
    let providers_body = providers.json();
    let provider_ids = providers_body["data"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| provider["provider_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(provider_ids.len(), 2);
    assert!(provider_ids.contains(&LOCAL_SIDECAR_PROVIDER_ID));
    assert!(provider_ids.contains(&DASHSCOPE_PROVIDER_ID));
    assert!(!provider_ids.contains(&"qdrant"));

    let global_content_types = app.get_json("/settings/content-types").await;
    assert_eq!(global_content_types.status, StatusCode::OK);
    let global_content_types_body = global_content_types.json();
    assert_eq!(
        global_content_types_body["data"]["content_types"]["content_types"]["image"]["model"],
        json!(format!("{LOCAL_SIDECAR_PROVIDER_ID}/{DEFAULT_MODEL_ID}"))
    );
    assert_eq!(
        global_content_types_body["data"]["content_types"]["content_types"]["document"]
            ["vector_type"],
        json!("multi_vector_late_interaction")
    );

    let library_id = create_library(&app, "provider-global-content-types").await;

    let content_types = app
        .get_json(&format!("/libraries/{library_id}/content-types"))
        .await;
    assert_eq!(content_types.status, StatusCode::OK);
    assert_eq!(
        content_types.json()["data"]["content_types"]["content_types"]["document"]["model"],
        json!(format!("{LOCAL_SIDECAR_PROVIDER_ID}/{DEFAULT_MODEL_ID}"))
    );

    let resolved = app
        .get_json(&format!("/libraries/{library_id}/resolved-content-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["binding_source"],
        "global_content_type"
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["provider_id"],
        LOCAL_SIDECAR_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["model_id"],
        DEFAULT_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["model_version"],
        "main"
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["vector_type"],
        "multi_vector_late_interaction"
    );
    assert!(
        resolved_body["data"]["content_types"]["document"]["vector_space_id"]
            .as_str()
            .is_some()
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false,
        })
    );
}

#[tokio::test]
async fn model_catalog_exposes_runtime_model_versions_and_supported_entries() {
    let env = TestEnv::new("provider-model-catalog").await;
    let app = env.boot().await;

    let catalog = app.get_json("/settings/model-catalog").await;
    assert_eq!(catalog.status, StatusCode::OK);
    let body = catalog.json();
    let entries = body["data"]["entries"].as_array().unwrap();

    let local_entry = entries
        .iter()
        .find(|entry| {
            entry["provider_id"] == LOCAL_SIDECAR_PROVIDER_ID
                && entry["model_id"] == DEFAULT_MODEL_ID
        })
        .expect("local sidecar catalog entry should exist");
    assert_eq!(local_entry["model_id"], DEFAULT_MODEL_ID);
    assert_eq!(local_entry["model_version"], "main");
    assert_eq!(local_entry["model_revision"], "main");
    assert_eq!(local_entry["editable"], true);
    assert_eq!(
        local_entry["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false,
        })
    );
    assert!(entries.iter().any(|entry| {
        entry["provider_id"] == LOCAL_SIDECAR_PROVIDER_ID
            && entry["model_id"] == "Qwen/Qwen3-VL-Embedding-2B"
            && entry["embedding_capabilities"]["vector_types"] == json!(["single_vector"])
    }));

    let dashscope_entries = entries
        .iter()
        .filter(|entry| entry["provider_id"] == DASHSCOPE_PROVIDER_ID)
        .collect::<Vec<_>>();
    let dashscope_models = dashscope_entries
        .iter()
        .map(|entry| entry["model_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(dashscope_models.contains(&DASHSCOPE_MODEL_ID));
    assert!(!dashscope_models.contains(&"text-embedding-v4"));
}

#[tokio::test]
async fn provider_probe_cache_coalesces_capabilities_requests_until_forced_refresh() {
    let env = TestEnv::new("provider-probe-cache-coalesces").await;
    let app = env.boot().await;
    let boot_probe_count = env.sidecar_capabilities_count();
    assert_eq!(boot_probe_count, 1);

    let providers = app.get_json("/settings/providers").await;
    assert_eq!(providers.status, StatusCode::OK);
    let runtime_status = app.get_json("/runtime/status").await;
    assert_eq!(runtime_status.status, StatusCode::OK);
    let providers_again = app.get_json("/settings/providers").await;
    assert_eq!(providers_again.status, StatusCode::OK);
    assert_eq!(env.sidecar_capabilities_count(), boot_probe_count);

    let provider = app
        .patch_json(
            &format!("/settings/providers/{LOCAL_SIDECAR_PROVIDER_ID}"),
            json!({
                "display_name": "Local Sidecar",
                "provider_kind": "local_sidecar",
                "enabled": true,
                "active_model": DEFAULT_MODEL_ID
            }),
        )
        .await;
    assert_eq!(provider.status, StatusCode::OK);
    assert_eq!(env.sidecar_capabilities_count(), boot_probe_count + 1);

    let runtime_status = app.get_json("/runtime/status").await;
    assert_eq!(runtime_status.status, StatusCode::OK);
    assert_eq!(env.sidecar_capabilities_count(), boot_probe_count + 1);
}

#[tokio::test]
async fn provider_runtime_overlay_crud_manages_provider_models() {
    let env = TestEnv::new("provider-runtime-overlay-crud").await;
    let app = env.boot().await;
    let provider_id = "custom_provider";
    let model_id = "custom-model";

    let provider = app
        .patch_json(
            &format!("/settings/providers/{provider_id}"),
            json!({
                "display_name": "Custom Provider",
                "provider_kind": "custom",
                "enabled": true,
                "base_url": "http://localhost:9999"
            }),
        )
        .await;
    assert_eq!(provider.status, StatusCode::OK);
    assert_eq!(provider.json()["data"]["origin"], "runtime_overlay");

    let model = app
        .patch_json(
            &format!("/settings/providers/{provider_id}/models/{model_id}"),
            json!({
                "enabled": true,
                "version": "v1",
                "embedding_capabilities": {
                    "input_types": ["text"],
                    "vector_types": ["single_vector"],
                    "supports_mixed_inputs": false
                }
            }),
        )
        .await;
    assert_eq!(model.status, StatusCode::OK);
    assert_eq!(
        model.json()["data"]["models"][0]["origin"],
        "runtime_overlay"
    );

    let runtime_config = runtime_config_json(&env);
    assert_eq!(
        runtime_config["provider"][provider_id]["models"][model_id]["version"],
        json!("v1")
    );

    let catalog = app.get_json("/settings/model-catalog").await.json();
    assert!(catalog["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["provider_id"] == provider_id && entry["model_id"] == model_id));

    let delete_model = app
        .delete(&format!(
            "/settings/providers/{provider_id}/models/{model_id}"
        ))
        .await;
    assert_eq!(delete_model.status, StatusCode::OK);
    let runtime_config = runtime_config_json(&env);
    assert!(runtime_config["provider"][provider_id]["models"][model_id].is_null());

    let delete_provider = app
        .delete(&format!("/settings/providers/{provider_id}"))
        .await;
    assert_eq!(delete_provider.status, StatusCode::OK);
    let runtime_config = runtime_config_json(&env);
    assert!(runtime_config["provider"][provider_id].is_null());
}

#[tokio::test]
async fn content_type_runtime_overlay_single_item_crud_restores_baseline_and_inheritance() {
    let env = TestEnv::new("content-type-runtime-overlay-crud").await;
    let app = env.boot().await;

    let global_patch = app
        .patch_json(
            "/settings/content-types/image",
            json!({
                "enabled": true,
                "model": "local_sidecar/Qwen/Qwen3-VL-Embedding-2B",
                "vector_type": "single_vector"
            }),
        )
        .await;
    assert_eq!(global_patch.status, StatusCode::OK);
    assert_eq!(
        global_patch.json()["data"]["origins"]["image"]["origin"],
        "runtime_overlay"
    );
    let runtime_config = runtime_config_json(&env);
    assert_eq!(
        runtime_config["content_types"]["image"]["model"],
        json!("local_sidecar/Qwen/Qwen3-VL-Embedding-2B")
    );

    let global_delete = app.delete("/settings/content-types/image").await;
    assert_eq!(global_delete.status, StatusCode::OK);
    let global_delete_body = global_delete.json();
    assert_eq!(
        global_delete_body["data"]["content_types"]["content_types"]["image"]["model"],
        json!(format!("{LOCAL_SIDECAR_PROVIDER_ID}/{DEFAULT_MODEL_ID}"))
    );
    assert_eq!(
        global_delete_body["data"]["origins"]["image"]["origin"],
        "baseline"
    );
    let runtime_config = runtime_config_json(&env);
    assert!(runtime_config["content_types"]["image"].is_null());

    let invalid_content_type = app
        .patch_json(
            "/settings/content-types/custom",
            json!({
                "enabled": true,
                "model": format!("{LOCAL_SIDECAR_PROVIDER_ID}/{DEFAULT_MODEL_ID}"),
                "vector_type": "multi_vector_late_interaction"
            }),
        )
        .await;
    assert_eq!(
        invalid_content_type.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let invalid_vector_type = app
        .patch_json(
            "/settings/content-types/text",
            json!({
                "enabled": true,
                "model": "local_sidecar/Qwen/Qwen3-VL-Embedding-2B",
                "vector_type": "multi_vector_late_interaction"
            }),
        )
        .await;
    assert_eq!(invalid_vector_type.status, StatusCode::UNPROCESSABLE_ENTITY);

    let library_id = create_library(&app, "content-type-overlay-crud").await;
    let library_patch = app
        .patch_json(
            &format!("/libraries/{library_id}/content-types/image"),
            json!({
                "enabled": true,
                "model": "local_sidecar/Qwen/Qwen3-VL-Embedding-2B",
                "vector_type": "single_vector"
            }),
        )
        .await;
    assert_eq!(library_patch.status, StatusCode::OK);
    assert_eq!(
        library_patch.json()["data"]["origins"]["image"]["origin"],
        "runtime_overlay"
    );
    let runtime_config = runtime_config_json(&env);
    assert_eq!(
        runtime_config["libraries"][library_id.as_str()]["content_types"]["image"]["model"],
        json!("local_sidecar/Qwen/Qwen3-VL-Embedding-2B")
    );

    let library_delete = app
        .delete(&format!("/libraries/{library_id}/content-types/image"))
        .await;
    assert_eq!(library_delete.status, StatusCode::OK);
    assert_eq!(
        library_delete.json()["data"]["origins"]["image"]["origin"],
        "inherited"
    );
    let runtime_config = runtime_config_json(&env);
    assert!(runtime_config["libraries"][library_id.as_str()].is_null());
}

#[tokio::test]
async fn settings_model_tests_only_cover_native_embedding_inputs_without_mutating_saved_config() {
    let env = TestEnv::new("provider-settings-model-tests").await;
    let app = env.boot().await;
    let global_content_types_before = app.get_json("/settings/content-types").await.json();
    let providers_before = app.get_json("/settings/providers").await.json();

    let text_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                (
                    "provider_id".to_string(),
                    LOCAL_SIDECAR_PROVIDER_ID.to_string(),
                ),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "text".to_string()),
                ("text".to_string(), "Revenue 46 percent".to_string()),
            ],
            None,
        )
        .await;
    assert_eq!(text_response.status, StatusCode::OK);
    assert_eq!(text_response.json()["data"]["vector_shape"], json!([2, 3]));

    let image_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                (
                    "provider_id".to_string(),
                    LOCAL_SIDECAR_PROVIDER_ID.to_string(),
                ),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "image".to_string()),
            ],
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "tatdqa-page-0001.png".to_string(),
                content_type: "image/png".to_string(),
                bytes: fs::read(
                    env.repo_path("tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png"),
                )
                .expect("image fixture should exist"),
            }),
        )
        .await;
    assert_eq!(image_response.status, StatusCode::OK);
    assert_eq!(image_response.json()["data"]["vector_shape"], json!([1, 3]));

    let comparison_response = app
        .post_multipart_with_files(
            "/settings/model-tests",
            vec![
                (
                    "provider_id".to_string(),
                    LOCAL_SIDECAR_PROVIDER_ID.to_string(),
                ),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "text".to_string()),
                ("text".to_string(), "Revenue 46 percent".to_string()),
                ("comparison_input_modality".to_string(), "image".to_string()),
            ],
            vec![MultipartFile {
                field_name: "comparison_file".to_string(),
                filename: "tatdqa-page-0001.png".to_string(),
                content_type: "image/png".to_string(),
                bytes: fs::read(
                    env.repo_path("tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png"),
                )
                .expect("image fixture should exist"),
            }],
        )
        .await;
    assert_eq!(comparison_response.status, StatusCode::OK);
    let similarity = comparison_response.json()["data"]["comparison"]["similarity_to_primary"]
        .as_f64()
        .expect("similarity should be numeric");
    assert!(similarity > 0.98 && similarity <= 1.0);

    let video_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                (
                    "provider_id".to_string(),
                    LOCAL_SIDECAR_PROVIDER_ID.to_string(),
                ),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "video".to_string()),
            ],
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "query-video.mp4".to_string(),
                content_type: "video/mp4".to_string(),
                bytes: b"fake-video".to_vec(),
            }),
        )
        .await;
    assert_eq!(video_response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(video_response.json()["error"]["code"], "validation_failed");

    let global_content_types_after = app.get_json("/settings/content-types").await.json();
    let providers_after = app.get_json("/settings/providers").await.json();
    assert_eq!(global_content_types_before, global_content_types_after);
    let provider_projection = |body: &serde_json::Value| {
        body["data"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|provider| {
                json!({
                    "provider_id": provider["provider_id"],
                    "enabled": provider["enabled"],
                    "base_url": provider["base_url"],
                })
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        provider_projection(&providers_before),
        provider_projection(&providers_after)
    );
    assert_eq!(settings_model_test_file_count(&env), 0);
}

#[tokio::test]
async fn dashscope_provider_config_and_library_content_types_take_effect() {
    let env = TestEnv::new("provider-settings-library-content-types").await;
    let app = env.boot().await;
    let library_id = create_library(&app, "provider-library-content-types").await;

    let provider = app
        .patch_json(
            &format!("/settings/providers/{DASHSCOPE_PROVIDER_ID}"),
            json!({
                "enabled": true,
                "base_url": "https://dashscope.aliyuncs.com"
            }),
        )
        .await;
    assert_eq!(provider.status, StatusCode::OK);

    let content_types = app
        .patch_json(
            &format!("/libraries/{library_id}/content-types"),
            dashscope_content_type_payload(),
        )
        .await;
    assert_eq!(content_types.status, StatusCode::OK);

    let runtime_config = runtime_config_json(&env);
    assert_eq!(
        runtime_config["provider"][DASHSCOPE_PROVIDER_ID]["enabled"],
        json!(true)
    );
    assert_eq!(
        runtime_config["provider"][DASHSCOPE_PROVIDER_ID]["base_url"],
        json!("https://dashscope.aliyuncs.com")
    );
    assert_eq!(
        runtime_config["libraries"][library_id.as_str()]["content_types"]["document"]["model"],
        json!(format!("{DASHSCOPE_PROVIDER_ID}/{DASHSCOPE_MODEL_ID}"))
    );
    assert_eq!(
        runtime_config["libraries"][library_id.as_str()]["content_types"]["document"]
            ["vector_type"],
        json!("single_vector")
    );

    let resolved = app
        .get_json(&format!("/libraries/{library_id}/resolved-content-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["binding_source"],
        "library_content_type"
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["provider_id"],
        DASHSCOPE_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["model_id"],
        DASHSCOPE_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["model_version"],
        "main"
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["vector_type"],
        "single_vector"
    );
    assert!(
        resolved_body["data"]["content_types"]["document"]["vector_space_id"]
            .as_str()
            .is_some()
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["document"]["status"],
        "not_supported"
    );
}

#[tokio::test]
async fn dashscope_rejects_text_only_embedding_models_for_content_types() {
    let env = TestEnv::new("provider-settings-invalid-dashscope-model").await;
    let app = env.boot().await;

    let library_id = create_library(&app, "provider-invalid-model").await;
    let response = app
        .patch_json(
            &format!("/libraries/{library_id}/content-types"),
            json!({
                "content_types": {
                    "image": {
                        "enabled": true,
                        "model": "dashscope/text-embedding-v4",
                        "vector_type": "single_vector"
                    }
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json()["error"]["details"]["field"],
        "content_types.image.model"
    );
}

#[tokio::test]
async fn provider_settings_persist_across_restart() {
    let env = TestEnv::new("provider-settings-restart").await;
    let app = env.boot().await;
    let library_id = create_library(&app, "provider-restart").await;

    let provider = app
        .patch_json(
            &format!("/settings/providers/{DASHSCOPE_PROVIDER_ID}"),
            json!({
                "enabled": true,
                "base_url": "https://dashscope.aliyuncs.com"
            }),
        )
        .await;
    assert_eq!(provider.status, StatusCode::OK);

    let content_types = app
        .patch_json(
            &format!("/libraries/{library_id}/content-types"),
            dashscope_content_type_payload(),
        )
        .await;
    assert_eq!(content_types.status, StatusCode::OK);

    let reloaded = env.boot().await;
    let resolved = reloaded
        .get_json(&format!("/libraries/{library_id}/resolved-content-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["content_types"]["image"]["binding_source"],
        "library_content_type"
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["image"]["provider_id"],
        DASHSCOPE_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["image"]["model_id"],
        DASHSCOPE_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["content_types"]["image"]["model_version"],
        "main"
    );
    assert!(
        resolved_body["data"]["content_types"]["image"]["vector_space_id"]
            .as_str()
            .is_some()
    );
}
