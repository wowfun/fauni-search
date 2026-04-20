mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

const LOCAL_SIDECAR_PROVIDER_ID: &str = "local_sidecar";
const DASHSCOPE_PROVIDER_ID: &str = "dashscope";
const DEFAULT_MODEL_ID: &str = "athrael-soju/colqwen3.5-4.5B-v3";

#[tokio::test]
async fn runtime_health_aggregates_app_qdrant_and_provider_diagnostics() {
    let env = TestEnv::new("runtime-health").await;
    let app = env.boot().await;

    let create = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "Runtime Health"
            }),
        )
        .await;
    assert_eq!(create.status, StatusCode::CREATED);

    let response = app.get_json("/runtime-health").await;
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    let data = &body["data"];

    assert_eq!(data["app"]["component_id"], "app");
    assert_eq!(data["app"]["status"], "available");
    assert_eq!(data["app"]["details"]["env"], "test");
    assert_eq!(data["app"]["details"]["libraries"], 1);

    assert_eq!(data["qdrant"]["component_id"], "qdrant");
    assert_eq!(data["qdrant"]["status"], "runtime_unavailable");
    assert!(data["qdrant"]["message"].as_str().is_some());

    let providers = data["providers"]
        .as_array()
        .expect("runtime health should include provider snapshots");

    let local = providers
        .iter()
        .find(|provider| provider["provider_id"] == LOCAL_SIDECAR_PROVIDER_ID)
        .expect("local sidecar runtime health should be present");
    assert_eq!(local["status"], "available");
    assert_eq!(local["model_id"], DEFAULT_MODEL_ID);
    assert_eq!(local["model_version"], "main");
    assert_eq!(
        local["embedding_capabilities"],
        json!({
            "input_types": ["text", "image"],
            "vector_types": ["multi_vector_late_interaction"],
            "supports_mixed_inputs": false,
        })
    );
    assert_eq!(
        local["execution_input_types"],
        json!(["text", "image", "document", "video"])
    );
    assert_eq!(
        local["runtime_adapters"],
        json!([
            "document_query_via_page_images",
            "video_query_via_frame_images"
        ])
    );

    let dashscope = providers
        .iter()
        .find(|provider| provider["provider_id"] == DASHSCOPE_PROVIDER_ID)
        .expect("dashscope runtime health should be present");
    assert_eq!(dashscope["status"], "not_supported");
    assert_eq!(dashscope["execution_input_types"], json!([]));
    assert_eq!(dashscope["runtime_adapters"], json!([]));
}
