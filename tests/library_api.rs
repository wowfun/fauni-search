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
    assert_eq!(valid_body["data"]["lifecycle_state"], "active");
    assert!(valid_body["data"]["archived_at_ms"].is_null());
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
    assert_eq!(custom_body["data"]["lifecycle_state"], "active");

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

#[tokio::test]
async fn update_library_renames_display_name_and_rejects_library_id_changes() {
    let env = TestEnv::new("library-api-update").await;
    let app = env.boot().await;

    let created = app
        .post_json(
            "/libraries",
            json!({
                "library_id": "quarterly-reports",
                "display_name": "Quarterly Reports"
            }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);

    let renamed = app
        .patch_json(
            "/libraries/quarterly-reports",
            json!({
                "display_name": "Quarterly Reports Archive"
            }),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK);
    let renamed_body = renamed.json();
    assert_eq!(renamed_body["data"]["id"], "quarterly-reports");
    assert_eq!(
        renamed_body["data"]["display_name"],
        "Quarterly Reports Archive"
    );
    assert_eq!(renamed_body["data"]["lifecycle_state"], "active");

    let listing = app.get_json("/libraries").await;
    assert_eq!(listing.status, StatusCode::OK);
    assert_eq!(
        listing.json()["data"]["libraries"][0]["display_name"],
        "Quarterly Reports Archive"
    );

    let invalid = app
        .patch_json(
            "/libraries/quarterly-reports",
            json!({
                "display_name": "Still Quarterly Reports",
                "library_id": "should-not-change"
            }),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_body = invalid.json();
    assert_eq!(invalid_body["error"]["code"], "validation_failed");
    assert_eq!(invalid_body["error"]["details"]["field"], "library_id");
}

#[tokio::test]
async fn archive_and_restore_library_updates_lifecycle_state_and_listing_order() {
    let env = TestEnv::new("library-api-archive").await;
    let app = env.boot().await;

    let active = app
        .post_json(
            "/libraries",
            json!({
                "library_id": "active-library",
                "display_name": "Active Library"
            }),
        )
        .await;
    assert_eq!(active.status, StatusCode::CREATED);

    let archived = app
        .post_json(
            "/libraries",
            json!({
                "library_id": "archive-me",
                "display_name": "Archive Me"
            }),
        )
        .await;
    assert_eq!(archived.status, StatusCode::CREATED);

    let archive = app.post_empty("/libraries/archive-me/archive").await;
    assert_eq!(archive.status, StatusCode::OK);
    let archive_body = archive.json();
    assert_eq!(archive_body["data"]["id"], "archive-me");
    assert_eq!(archive_body["data"]["lifecycle_state"], "archived");
    assert!(archive_body["data"]["archived_at_ms"].as_u64().is_some());

    let listing = app.get_json("/libraries").await;
    assert_eq!(listing.status, StatusCode::OK);
    let libraries = listing.json()["data"]["libraries"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(libraries[0]["id"], "active-library");
    assert_eq!(libraries[0]["lifecycle_state"], "active");
    assert_eq!(libraries[1]["id"], "archive-me");
    assert_eq!(libraries[1]["lifecycle_state"], "archived");

    let restored = app.post_empty("/libraries/archive-me/restore").await;
    assert_eq!(restored.status, StatusCode::OK);
    let restored_body = restored.json();
    assert_eq!(restored_body["data"]["lifecycle_state"], "active");
    assert!(restored_body["data"]["archived_at_ms"].is_null());
}

#[tokio::test]
async fn delete_library_removes_it_from_get_and_list_surfaces() {
    let env = TestEnv::new("library-api-delete").await;
    let app = env.boot().await;

    let created = app
        .post_json(
            "/libraries",
            json!({
                "library_id": "to-delete",
                "display_name": "To Delete"
            }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);

    let deleted = app.delete("/libraries/to-delete").await;
    assert_eq!(deleted.status, StatusCode::OK);
    let deleted_body = deleted.json();
    assert_eq!(deleted_body["data"]["id"], "to-delete");
    assert_eq!(deleted_body["data"]["display_name"], "To Delete");
    assert_eq!(deleted_body["data"]["lifecycle_state"], "active");

    let missing = app.get_json("/libraries/to-delete").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json()["error"]["code"], "not_found");

    let listing = app.get_json("/libraries").await;
    assert_eq!(listing.status, StatusCode::OK);
    assert_eq!(listing.json()["data"]["libraries"], json!([]));
}
