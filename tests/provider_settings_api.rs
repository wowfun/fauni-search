mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

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
    assert_eq!(local_entry["model_id"], DEFAULT_MODEL_ID);
    assert_eq!(local_entry["model_revision"], "main");
    assert_eq!(local_entry["editable"], false);
    assert_eq!(
        local_entry["supported_index_lines"].as_array().unwrap(),
        &[json!("multivector")]
    );

    let dashscope_models = entries
        .iter()
        .filter(|entry| entry["provider_id"] == DASHSCOPE_PROVIDER_ID)
        .map(|entry| entry["model_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(dashscope_models.contains(&DASHSCOPE_MODEL_ID));
    assert!(!dashscope_models.contains(&"text-embedding-v4"));
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
}
