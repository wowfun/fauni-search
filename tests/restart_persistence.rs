mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::fs;
use support::TestEnv;

#[tokio::test]
async fn libraries_and_source_roots_survive_restart() {
    let env = TestEnv::new("restart-persistence").await;
    let root_dir = env.create_dir("fixtures/library-root");
    env.write_bytes("fixtures/library-root/chart.png", b"png");
    let canonical_root_dir = fs::canonicalize(&root_dir).unwrap();

    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "name": "restart-persistence",
                "config": { "enabled_index_lines": ["multivector"] }
            }),
        )
        .await;
    assert_eq!(library.status, StatusCode::CREATED);
    let library_body = library.json();
    let library_id = library_body["data"]["id"].as_str().unwrap();

    let source_root = app
        .post_json(
            &format!("/libraries/{library_id}/source-roots"),
            json!({
                "root_path": root_dir.to_string_lossy(),
                "enabled": true,
                "rules": { "include_extensions": ["png"] }
            }),
        )
        .await;
    assert_eq!(source_root.status, StatusCode::CREATED);
    let source_root_body = source_root.json();
    let source_root_id = source_root_body["data"]["source_root_id"].as_str().unwrap();

    let restarted = env.boot().await;
    let libraries = restarted.get_json("/libraries").await;
    assert_eq!(libraries.status, StatusCode::OK);
    let libraries_body = libraries.json();
    assert_eq!(
        libraries_body["data"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(libraries_body["data"]["libraries"][0]["id"], library_id);

    let source_roots = restarted
        .get_json(&format!("/libraries/{library_id}/source-roots"))
        .await;
    assert_eq!(source_roots.status, StatusCode::OK);
    let source_roots_body = source_roots.json();
    let root = &source_roots_body["data"]["source_roots"][0];
    assert_eq!(root["source_root_id"], source_root_id);
    assert_eq!(
        root["root_path"],
        canonical_root_dir.to_string_lossy().to_string()
    );
    assert_eq!(root["enabled"], true);
    assert_eq!(root["status"], "ready");
    assert_eq!(root["watch_state"], "watching");
    assert_eq!(root["rules"]["include_extensions"], json!(["png"]));
    assert_eq!(root["coverage_summary"]["observed_file_count"], 1);
    assert_eq!(root["coverage_summary"]["matched_file_count"], 1);
}
