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
                "name": name,
                "config": { "enabled_index_lines": ["multivector"] }
            }),
        )
        .await
        .json();
    library["data"]["id"].as_str().unwrap().to_string()
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
            .map(|path| {
                if path.is_dir() {
                    count_files(&path)
                } else {
                    1
                }
            })
            .sum()
    }

    count_files(&root)
}

#[tokio::test]
async fn provider_settings_bootstrap_defaults_and_resolved_model() {
    let env = TestEnv::new("provider-settings-defaults").await;
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
    assert_eq!(
        providers_body["data"]["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|provider| provider["provider_id"] == LOCAL_SIDECAR_PROVIDER_ID)
            .unwrap()["provider_kind"],
        "local_sidecar"
    );

    let defaults = app.get_json("/settings/model-defaults").await;
    assert_eq!(defaults.status, StatusCode::OK);
    let defaults_body = defaults.json();
    assert_eq!(
        defaults_body["data"]["defaults"]["index_lines"]["multivector"]["provider_id"],
        LOCAL_SIDECAR_PROVIDER_ID
    );
    assert_eq!(
        defaults_body["data"]["defaults"]["index_lines"]["multivector"]["model_id"],
        DEFAULT_MODEL_ID
    );

    let library_id = create_library(&app, "provider-defaults").await;

    let overrides = app
        .get_json(&format!("/libraries/{library_id}/model-overrides"))
        .await;
    assert_eq!(overrides.status, StatusCode::OK);
    assert_eq!(
        overrides.json()["data"]["overrides"]["index_lines"]["multivector"],
        json!({})
    );

    let resolved = app
        .get_json(&format!("/libraries/{library_id}/resolved-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["binding_source"],
        "global_default"
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["provider_id"],
        LOCAL_SIDECAR_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["model_id"],
        DEFAULT_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["model_revision"],
        "main"
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false,
        })
    );
    assert!(
        resolved_body["data"]["index_lines"]["multivector"]
            .get("native_query_modalities")
            .is_none()
    );
}

#[tokio::test]
async fn model_catalog_exposes_runtime_model_and_only_multivector_entries() {
    let env = TestEnv::new("provider-model-catalog").await;
    let app = env.boot().await;

    let catalog = app.get_json("/settings/model-catalog").await;
    assert_eq!(catalog.status, StatusCode::OK);
    let body = catalog.json();
    let entries = body["data"]["entries"].as_array().unwrap();

    let local_entry = entries
        .iter()
        .find(|entry| entry["provider_id"] == LOCAL_SIDECAR_PROVIDER_ID)
        .expect("local sidecar catalog entry should exist");
    assert!(local_entry.get("supported_test_modalities").is_none());
    assert_eq!(local_entry["model_id"], DEFAULT_MODEL_ID);
    assert_eq!(local_entry["model_revision"], "main");
    assert_eq!(local_entry["editable"], false);
    assert_eq!(
        local_entry["supported_index_lines"].as_array().unwrap(),
        &[json!("multivector")]
    );
    assert_eq!(
        local_entry["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false,
        })
    );

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
    let dashscope_default_entry = dashscope_entries
        .iter()
        .find(|entry| entry["model_id"] == DASHSCOPE_MODEL_ID)
        .expect("dashscope multivector entry should exist");
    assert!(
        dashscope_entries.iter().all(|entry| {
            entry["embedding_capabilities"]["input_types"] == json!(["text", "image"])
                && entry["embedding_capabilities"]["supports_mixed_inputs"] == json!(true)
        })
    );
    assert_eq!(
        dashscope_default_entry["embedding_capabilities"]["vector_types"],
        json!(["single_vector", "independent_vectors"])
    );
    assert!(
        entries.iter().all(|entry| {
            entry["supported_index_lines"]
                .as_array()
                .unwrap()
                .iter()
                .all(|value| value == "multivector")
        }),
        "model catalog should only expose multivector-compatible entries"
    );
}

#[tokio::test]
async fn settings_model_tests_only_cover_native_embedding_inputs_without_mutating_saved_config() {
    let env = TestEnv::new("provider-settings-model-tests").await;
    let app = env.boot().await;
    let defaults_before = app.get_json("/settings/model-defaults").await.json();
    let providers_before = app.get_json("/settings/providers").await.json();

    let text_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                ("provider_id".to_string(), LOCAL_SIDECAR_PROVIDER_ID.to_string()),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "text".to_string()),
                ("text".to_string(), "Revenue 46 percent".to_string()),
            ],
            None,
        )
        .await;
    assert_eq!(text_response.status, StatusCode::OK);
    let text_body = text_response.json();
    assert_eq!(text_body["data"]["vector_shape"], json!([2, 3]));
    assert_eq!(text_body["data"]["input_modality"], "text");
    assert_eq!(
        text_body["data"]["resolved_model"]["binding_source"],
        "settings_draft"
    );
    assert_eq!(
        text_body["data"]["input_summary"]["text_preview"],
        "Revenue 46 percent"
    );

    let image_bytes = fs::read(env.repo_path(
        "tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png",
    ))
    .expect("image fixture should exist");
    let image_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                ("provider_id".to_string(), LOCAL_SIDECAR_PROVIDER_ID.to_string()),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "image".to_string()),
            ],
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "tatdqa-page-0001.png".to_string(),
                content_type: "image/png".to_string(),
                bytes: image_bytes,
            }),
        )
        .await;
    assert_eq!(image_response.status, StatusCode::OK);
    assert_eq!(image_response.json()["data"]["vector_shape"], json!([1, 3]));

    let video_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                ("provider_id".to_string(), LOCAL_SIDECAR_PROVIDER_ID.to_string()),
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

    let document_response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                ("provider_id".to_string(), LOCAL_SIDECAR_PROVIDER_ID.to_string()),
                ("model_id".to_string(), DEFAULT_MODEL_ID.to_string()),
                ("input_modality".to_string(), "document".to_string()),
            ],
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "settings-model-test.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                bytes: b"%PDF-1.7".to_vec(),
            }),
        )
        .await;
    assert_eq!(document_response.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(document_response.json()["error"]["code"], "validation_failed");

    let defaults_after = app.get_json("/settings/model-defaults").await.json();
    let providers_after = app.get_json("/settings/providers").await.json();
    assert_eq!(defaults_before, defaults_after);
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
async fn settings_model_test_returns_not_supported_for_dashscope() {
    let env = TestEnv::new("provider-settings-model-tests-dashscope").await;
    let app = env.boot().await;

    let response = app
        .post_multipart(
            "/settings/model-tests",
            vec![
                ("provider_id".to_string(), DASHSCOPE_PROVIDER_ID.to_string()),
                ("model_id".to_string(), DASHSCOPE_MODEL_ID.to_string()),
                ("input_modality".to_string(), "text".to_string()),
                ("text".to_string(), "Revenue 46 percent".to_string()),
            ],
            None,
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.json();
    assert_eq!(body["error"]["code"], "not_supported");
    assert_eq!(settings_model_test_file_count(&env), 0);
}

#[tokio::test]
async fn dashscope_provider_config_and_library_override_take_effect() {
    let env = TestEnv::new("provider-settings-library-override").await;
    let app = env.boot().await;
    let library_id = create_library(&app, "provider-library-override").await;

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
    let provider_body = provider.json();
    assert_eq!(provider_body["data"]["provider_id"], DASHSCOPE_PROVIDER_ID);
    assert_eq!(provider_body["data"]["base_url"], "https://dashscope.aliyuncs.com");

    let defaults = app
        .patch_json(
            "/settings/model-defaults",
            json!({
                "index_lines": {
                    "multivector": {
                        "provider_id": LOCAL_SIDECAR_PROVIDER_ID,
                        "model_id": DEFAULT_MODEL_ID
                    }
                }
            }),
        )
        .await;
    assert_eq!(defaults.status, StatusCode::OK);

    let overrides = app
        .patch_json(
            &format!("/libraries/{library_id}/model-overrides"),
            json!({
                "index_lines": {
                    "multivector": {
                        "provider_id": DASHSCOPE_PROVIDER_ID,
                        "model_id": DASHSCOPE_MODEL_ID
                    }
                }
            }),
        )
        .await;
    assert_eq!(overrides.status, StatusCode::OK);

    let resolved = app
        .get_json(&format!("/libraries/{library_id}/resolved-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["binding_source"],
        "library_override"
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["provider_id"],
        DASHSCOPE_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["model_id"],
        DASHSCOPE_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["single_vector", "independent_vectors"],
            "supports_mixed_inputs": true,
        })
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["status"],
        "not_supported"
    );
}

#[tokio::test]
async fn dashscope_rejects_text_only_embedding_models_for_multivector() {
    let env = TestEnv::new("provider-settings-invalid-dashscope-model").await;
    let app = env.boot().await;

    let library_id = create_library(&app, "provider-invalid-model").await;
    let response = app
        .patch_json(
            &format!("/libraries/{library_id}/model-overrides"),
            json!({
                "index_lines": {
                    "multivector": {
                        "provider_id": DASHSCOPE_PROVIDER_ID,
                        "model_id": "text-embedding-v4"
                    }
                }
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.json();
    assert_eq!(
        body["error"]["details"]["field"],
        "index_lines.multivector.model_id"
    );
    assert_eq!(
        body["error"]["code"],
        "validation_failed"
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

    let overrides = app
        .patch_json(
            &format!("/libraries/{library_id}/model-overrides"),
            json!({
                "index_lines": {
                    "multivector": {
                        "provider_id": DASHSCOPE_PROVIDER_ID,
                        "model_id": DASHSCOPE_MODEL_ID
                    }
                }
            }),
        )
        .await;
    assert_eq!(overrides.status, StatusCode::OK);

    let reloaded = env.boot().await;
    let resolved = reloaded
        .get_json(&format!("/libraries/{library_id}/resolved-models"))
        .await;
    assert_eq!(resolved.status, StatusCode::OK);
    let resolved_body = resolved.json();
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["binding_source"],
        "library_override"
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["provider_id"],
        DASHSCOPE_PROVIDER_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["model_id"],
        DASHSCOPE_MODEL_ID
    );
    assert_eq!(
        resolved_body["data"]["index_lines"]["multivector"]["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["single_vector", "independent_vectors"],
            "supports_mixed_inputs": true,
        })
    );
}
