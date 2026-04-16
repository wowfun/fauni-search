mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn create_library_requires_multivector_only() {
    let env = TestEnv::new("library-api-validation").await;
    let app = env.boot().await;

    let invalid = app
        .post_json(
            "/libraries",
            json!({
                "name": "demo",
                "config": { "enabled_index_lines": ["single-vector"] }
            }),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_body = invalid.json();
    assert_eq!(invalid_body["error"]["code"], "validation_failed");
    assert_eq!(
        invalid_body["error"]["details"]["field"],
        "config.enabled_index_lines"
    );
    assert_eq!(
        invalid_body["error"]["details"]["expected"],
        json!(["multivector"])
    );
    assert_eq!(
        invalid_body["error"]["details"]["received"],
        json!(["single-vector"])
    );

    let valid = app
        .post_json(
            "/libraries",
            json!({
                "name": "demo",
                "config": { "enabled_index_lines": ["multivector"] }
            }),
        )
        .await;
    assert_eq!(valid.status, StatusCode::CREATED);
    let valid_body = valid.json();
    assert_eq!(valid_body["data"]["id"], "lib_000001");
    assert_eq!(
        valid_body["data"]["index_lines"][0]["index_line"],
        "multivector"
    );
    assert_eq!(valid_body["data"]["index_lines"][0]["status"], "not_ready");
}
