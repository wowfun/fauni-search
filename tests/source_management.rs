mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn disabled_source_root_refresh_is_rejected_without_queueing_a_job() {
    let env = TestEnv::new("source-management").await;
    let root_dir = env.create_dir("fixtures/disabled-root");
    env.write_bytes("fixtures/disabled-root/disabled.png", b"png");

    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "disabled-root"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let source_root = app
        .post_json(
            &format!("/libraries/{library_id}/source-roots"),
            json!({
                "root_path": root_dir.to_string_lossy(),
                "enabled": false,
                "rules": {}
            }),
        )
        .await
        .json();
    let source_root_id = source_root["data"]["source_root_id"].as_str().unwrap();

    let refresh = app
        .post_empty(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}/refresh"
        ))
        .await;
    assert_eq!(refresh.status, StatusCode::OK);
    let refresh_body = refresh.json();
    assert_eq!(refresh_body["data"]["accepted"], json!([]));
    assert_eq!(
        refresh_body["data"]["rejected"][0]["reason_code"],
        "not_enabled"
    );
    assert_eq!(refresh_body["data"]["job_handle"], serde_json::Value::Null);
    assert_eq!(refresh_body["data"]["job"], serde_json::Value::Null);

    let detail = app
        .get_json(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}"
        ))
        .await;
    assert_eq!(detail.status, StatusCode::OK);
    let detail_body = detail.json();
    assert_eq!(detail_body["data"]["source_root"]["status"], "disabled");
    assert_eq!(
        detail_body["data"]["source_root"]["watch_state"],
        "disabled"
    );
}

#[tokio::test]
async fn source_root_patch_updates_rules_and_disabled_state() {
    let env = TestEnv::new("source-management-patch").await;
    let root_dir = env.create_dir("fixtures/patch-root");
    env.write_bytes("fixtures/patch-root/chart.png", b"png");

    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "patch-root"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let source_root = app
        .post_json(
            &format!("/libraries/{library_id}/source-roots"),
            json!({
                "root_path": root_dir.to_string_lossy(),
                "enabled": true,
                "rules": { "include_extensions": ["png"] }
            }),
        )
        .await
        .json();
    let source_root_id = source_root["data"]["source_root_id"].as_str().unwrap();

    let updated = app
        .patch_json(
            &format!("/libraries/{library_id}/source-roots/{source_root_id}"),
            json!({
                "enabled": false,
                "rules": { "include_extensions": ["pdf"] }
            }),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    let updated_body = updated.json();
    assert_eq!(updated_body["data"]["enabled"], false);
    assert_eq!(updated_body["data"]["status"], "disabled");
    assert_eq!(updated_body["data"]["watch_state"], "disabled");
    assert_eq!(
        updated_body["data"]["rules"]["include_extensions"],
        json!(["pdf"])
    );
    assert_eq!(
        updated_body["data"]["coverage_summary"]["observed_file_count"],
        0
    );
    assert_eq!(
        updated_body["data"]["coverage_summary"]["matched_file_count"],
        0
    );

    let detail = app
        .get_json(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}"
        ))
        .await;
    assert_eq!(detail.status, StatusCode::OK);
    let detail_body = detail.json();
    assert_eq!(detail_body["data"]["source_root"]["enabled"], false);
    assert_eq!(detail_body["data"]["source_root"]["status"], "disabled");
    assert_eq!(
        detail_body["data"]["source_root"]["rules"]["include_extensions"],
        json!(["pdf"])
    );
}

#[tokio::test]
async fn source_root_delete_removes_it_from_listing_and_detail() {
    let env = TestEnv::new("source-management-delete").await;
    let root_dir = env.create_dir("fixtures/delete-root");
    env.write_bytes("fixtures/delete-root/chart.png", b"png");

    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "delete-root"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let source_root = app
        .post_json(
            &format!("/libraries/{library_id}/source-roots"),
            json!({
                "root_path": root_dir.to_string_lossy(),
                "enabled": true,
                "rules": {}
            }),
        )
        .await
        .json();
    let source_root_id = source_root["data"]["source_root_id"].as_str().unwrap();

    let deleted = app
        .delete(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}"
        ))
        .await;
    assert_eq!(deleted.status, StatusCode::OK);
    let deleted_body = deleted.json();
    assert_eq!(deleted_body["data"]["source_root_id"], source_root_id);
    assert_eq!(deleted_body["data"]["enabled"], true);

    let source_roots = app
        .get_json(&format!("/libraries/{library_id}/source-roots"))
        .await;
    assert_eq!(source_roots.status, StatusCode::OK);
    let source_roots_body = source_roots.json();
    assert_eq!(source_roots_body["data"]["source_roots"], json!([]));

    let detail = app
        .get_json(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}"
        ))
        .await;
    assert_eq!(detail.status, StatusCode::NOT_FOUND);
    let detail_body = detail.json();
    assert_eq!(detail_body["error"]["code"], "not_found");
}
