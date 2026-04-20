mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn create_library_requires_display_name_or_library_id() {
    let env = TestEnv::new("library-api-validation").await;
    let app = env.boot().await;

    let invalid = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "   ",
                "library_id": "   "
            }),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_body = invalid.json();
    assert_eq!(invalid_body["error"]["code"], "validation_failed");
    assert_eq!(invalid_body["error"]["details"]["field"], "display_name");

    let legacy_name = app
        .post_json(
            "/libraries",
            json!({
                "name": "demo"
            }),
        )
        .await;
    assert_eq!(legacy_name.status, StatusCode::UNPROCESSABLE_ENTITY);
    let legacy_name_body = legacy_name.json();
    assert_eq!(legacy_name_body["error"]["code"], "validation_failed");
    assert_eq!(legacy_name_body["error"]["details"]["field"], "name");

    let valid = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "demo"
            }),
        )
        .await;
    assert_eq!(valid.status, StatusCode::CREATED);
    let valid_body = valid.json();
    assert_eq!(valid_body["data"]["id"], "demo");
    assert_eq!(valid_body["data"]["display_name"], "demo");
    assert!(valid_body["data"].get("name").is_none());
    assert!(valid_body["data"].get("index_lines").is_none());
}

#[tokio::test]
async fn create_library_accepts_custom_library_id_and_generates_unique_slugs() {
    let env = TestEnv::new("library-api-identity").await;
    let app = env.boot().await;

    let custom = app
        .post_json(
            "/libraries",
            json!({
                "library_id": "invoice_demo",
                "display_name": "Invoice Demo"
            }),
        )
        .await;
    assert_eq!(custom.status, StatusCode::CREATED);
    let custom_body = custom.json();
    assert_eq!(custom_body["data"]["id"], "invoice_demo");
    assert_eq!(custom_body["data"]["display_name"], "Invoice Demo");

    let generated = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "Invoice Demo"
            }),
        )
        .await;
    assert_eq!(generated.status, StatusCode::CREATED);
    assert_eq!(generated.json()["data"]["id"], "invoice-demo");

    let deduped = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "Invoice Demo"
            }),
        )
        .await;
    assert_eq!(deduped.status, StatusCode::CREATED);
    assert_eq!(deduped.json()["data"]["id"], "invoice-demo-2");
}
