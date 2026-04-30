mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;
use tokio::time::{sleep, Duration};

async fn wait_for_job_completion(app: &support::TestApp, job_id: &str) -> serde_json::Value {
    for _ in 0..80 {
        let response = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(response.status, StatusCode::OK);
        let body = response.json();
        match body["data"]["status"].as_str() {
            Some("completed") => return body,
            Some("failed") => panic!("expected job to complete, got failed: {body}"),
            Some("queued" | "running") => sleep(Duration::from_millis(25)).await,
            other => panic!("unexpected job status: {other:?}"),
        }
    }

    panic!("timed out waiting for job {job_id} to complete");
}

#[tokio::test]
async fn rebuild_endpoint_queues_background_rebuild_job() {
    let env = TestEnv::new_with_qdrant("maintenance-rebuild").await;
    let root_dir = env.create_dir("fixtures/rebuild-root");
    env.write_test_pdf("fixtures/rebuild-root/report.pdf", 2);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "maintenance-rebuild"
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
        .await;
    assert_eq!(source_root.status, StatusCode::CREATED);

    let rebuild = app
        .post_empty(&format!("/libraries/{library_id}/rebuild"))
        .await;
    assert_eq!(rebuild.status, StatusCode::OK);
    let rebuild_body = rebuild.json();
    assert_eq!(rebuild_body["data"]["accepted"][0]["action"], "rebuild");
    assert_eq!(rebuild_body["data"]["job"]["kind"], "rebuild");
    let job_id = rebuild_body["data"]["job_handle"].as_str().unwrap();

    let job = wait_for_job_completion(&app, job_id).await;
    assert_eq!(job["data"]["kind"], "rebuild");
    assert_eq!(job["data"]["status"], "completed");

    let sources = app
        .get_json(&format!("/libraries/{library_id}/sources"))
        .await;
    assert_eq!(sources.status, StatusCode::OK);
    let sources_body = sources.json();
    assert_eq!(sources_body["data"]["sources"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn maintenance_cleanup_endpoint_queues_cleanup_for_retired_unit_indexes_after_rebind() {
    let env = TestEnv::new_with_qdrant("maintenance-cleanup").await;
    let pdf_path = env.write_test_pdf("fixtures/cleanup/report.pdf", 2);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "maintenance-cleanup"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let import = app
        .post_json(
            &format!("/libraries/{library_id}/imports"),
            json!({
                "paths": [pdf_path.to_string_lossy().to_string()]
            }),
        )
        .await;
    assert_eq!(import.status, StatusCode::OK);
    let import_body = import.json();
    let import_job_id = import_body["data"]["job_handle"].as_str().unwrap();
    let import_job = wait_for_job_completion(&app, import_job_id).await;
    assert_eq!(import_job["data"]["status"], "completed");

    let content_types = app
        .patch_json(
            &format!("/libraries/{library_id}/content-types"),
            json!({
                "content_types": {
                    "image": {
                        "enabled": true,
                        "model": "openai_compatible/Qwen/Qwen3-VL-Embedding-2B",
                        "vector_type": "single_vector"
                    },
                    "document": {
                        "enabled": true,
                        "model": "openai_compatible/Qwen/Qwen3-VL-Embedding-2B",
                        "vector_type": "single_vector"
                    },
                    "video": {
                        "enabled": true,
                        "model": "openai_compatible/Qwen/Qwen3-VL-Embedding-2B",
                        "vector_type": "single_vector"
                    }
                }
            }),
        )
        .await;
    assert_eq!(content_types.status, StatusCode::OK);

    let diagnostics_before = app
        .get_json(&format!("/libraries/{library_id}/vector-space-diagnostics"))
        .await;
    assert_eq!(diagnostics_before.status, StatusCode::OK);
    let diagnostics_before_body = diagnostics_before.json();
    let diagnostic_spaces = diagnostics_before_body["data"]["vector_spaces"]
        .as_array()
        .unwrap();
    assert!(diagnostic_spaces
        .iter()
        .all(|item| item.get("lifecycle_state").is_none()));
    assert!(diagnostic_spaces
        .iter()
        .all(|item| item.get("unit_index_summary").is_some()));
    assert!(diagnostic_spaces.iter().all(|item| {
        let summary = &item["unit_index_summary"];
        summary.get("retired").is_some() && summary.get("staging").is_none()
    }));
    assert!(diagnostic_spaces
        .iter()
        .all(|item| item.get("content_e2e_index_summary").is_some()));

    let cleanup = app
        .post_json(
            &format!("/libraries/{library_id}/maintenance"),
            json!({
                "action": "cleanup_retired_vector_spaces"
            }),
        )
        .await;
    assert_eq!(cleanup.status, StatusCode::OK);
    let cleanup_body = cleanup.json();
    assert_eq!(
        cleanup_body["data"]["action"],
        "cleanup_retired_vector_spaces"
    );
    assert_eq!(cleanup_body["data"]["job"]["kind"], "cleanup");
    assert_eq!(cleanup_body["data"]["job"]["status"], "queued");
    assert_eq!(
        cleanup_body["data"]["accepted"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        cleanup_body["data"]["rejected"].as_array().unwrap().len(),
        0
    );
    let cleanup_job_id = cleanup_body["data"]["job_handle"].as_str().unwrap();
    let cleanup_job = wait_for_job_completion(&app, cleanup_job_id).await;
    assert_eq!(cleanup_job["data"]["status"], "completed");

    let diagnostics_after = app
        .get_json(&format!("/libraries/{library_id}/vector-space-diagnostics"))
        .await;
    assert_eq!(diagnostics_after.status, StatusCode::OK);
    let diagnostics_after_body = diagnostics_after.json();
    let diagnostic_spaces_after = diagnostics_after_body["data"]["vector_spaces"]
        .as_array()
        .unwrap();
    assert!(diagnostic_spaces_after
        .iter()
        .all(|item| item.get("lifecycle_state").is_none()));
}
