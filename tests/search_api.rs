mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::path::PathBuf;
use support::{MultipartFile, TestEnv};
use tokio::time::{sleep, Duration};

fn example_video_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/example/lib/generate_q2_report_from_csv_bank_data-720-512.mp4")
}

async fn wait_for_job_completion(app: &support::TestApp, job_id: &str) -> serde_json::Value {
    for _ in 0..40 {
        let response = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(response.status, StatusCode::OK);
        let body = response.json();
        match body["data"]["status"].as_str() {
            Some("completed") => return body,
            Some("failed") => panic!("expected import job to complete, got failed: {body}"),
            Some("queued" | "running") => sleep(Duration::from_millis(25)).await,
            other => panic!("unexpected job status: {other:?}"),
        }
    }

    panic!("timed out waiting for import job {job_id} to complete");
}

#[tokio::test]
async fn search_text_rejects_empty_text() {
    let env = TestEnv::new("search-api-validation").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-validation"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "   "
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body = search.json();
    assert_eq!(body["error"]["code"], "validation_failed");
    assert_eq!(body["error"]["details"]["field"], "text");
}

#[tokio::test]
async fn search_text_reports_not_ready_when_library_has_no_active_index() {
    let env = TestEnv::new("search-api-not-ready").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-not-ready"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "chart"
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::CONFLICT);
    let body = search.json();
    assert_eq!(body["error"]["code"], "not_ready");
    assert_eq!(
        body["error"]["details"]["content_types"][0]["content_type"],
        "document"
    );
    assert_eq!(
        body["error"]["details"]["content_types"][0]["status"],
        "not_ready"
    );
}

#[tokio::test]
async fn search_text_rejects_target_content_types_that_are_not_enabled() {
    let env = TestEnv::new("search-api-target-content-types").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-target-content-types"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "chart",
                "target_content_types": ["text"]
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::CONFLICT);
    let body = search.json();
    assert_eq!(body["error"]["code"], "not_enabled");
    assert_eq!(body["error"]["details"]["target_content_types"][0], "text");
}

#[tokio::test]
async fn search_text_not_ready_includes_latest_job_details() {
    let env = TestEnv::new("search-api-latest-job").await;
    let pdf_path = env.write_test_pdf("fixtures/search/pending.pdf", 2);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-latest-job"
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
        .await
        .json();
    let job_id = import["data"]["job_handle"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "chart"
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::CONFLICT);
    let body = search.json();
    assert_eq!(body["error"]["code"], "not_ready");
    assert_eq!(
        body["error"]["details"]["content_types"][0]["content_type"],
        "document"
    );
    assert_eq!(
        body["error"]["details"]["content_types"][0]["status"],
        "not_ready"
    );
    assert_eq!(
        body["error"]["details"]["content_types"][0]["job"]["job_id"],
        job_id
    );

    let job_status = body["error"]["details"]["content_types"][0]["job"]["status"]
        .as_str()
        .unwrap();
    assert!(matches!(job_status, "queued" | "running" | "failed"));
    assert!(body["error"]["details"]["content_types"][0]["job"]["phase"]
        .as_str()
        .is_some());
}

#[tokio::test]
async fn search_text_returns_results_with_debug_vector_type_after_import() {
    let env = TestEnv::new_with_qdrant("search-api-success").await;
    let pdf_path = env.write_test_pdf("fixtures/search/ready.pdf", 2);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-success"
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
    let job_id = import_body["data"]["job_handle"].as_str().unwrap();
    let job_body = wait_for_job_completion(&app, job_id).await;
    assert_eq!(job_body["data"]["status"], "completed");

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "chart",
                "debug": true,
                "target_content_types": ["document"]
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::OK);
    let body = search.json();
    let data = &body["data"];
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| item["kind"] == "document_page"));
    assert!(results.iter().all(|item| item["source_type"] == "pdf"));
    assert!(results.iter().all(|item| {
        item["source_path"]
            .as_str()
            .map(|value| value.ends_with("ready.pdf"))
            .unwrap_or(false)
    }));
    assert_eq!(data["next_cursor"], serde_json::Value::Null);
    assert_eq!(data["unsupported_content_types"], serde_json::Value::Null);
    assert_eq!(
        data["debug"]["vector_type"],
        "multi_vector_late_interaction"
    );
    assert_eq!(data["debug"]["repr_kind"], serde_json::Value::Null);
    assert_eq!(data["debug"]["content_types"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["debug"]["content_types"][0]["content_type"],
        "document"
    );
    assert_eq!(
        data["debug"]["content_types"][0]["raw_scores"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(data["debug"]["vector_spaces"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["debug"]["vector_spaces"][0]["content_types"],
        json!(["document"])
    );
}

#[tokio::test]
async fn search_video_uses_execution_input_types_for_local_sidecar() {
    let env = TestEnv::new("search-api-video-execution-inputs").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-video-execution-inputs"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let query_asset = app
        .post_multipart(
            &format!("/libraries/{library_id}/query-assets/videos"),
            Vec::new(),
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "query-video.mp4".to_string(),
                content_type: "video/mp4".to_string(),
                bytes: std::fs::read(example_video_path())
                    .expect("example video should be readable"),
            }),
        )
        .await
        .json();
    let temp_asset_id = query_asset["data"]["temp_asset_id"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/video",
            json!({
                "library_id": library_id,
                "video_input": {
                    "kind": "temp_asset",
                    "temp_asset_id": temp_asset_id
                }
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::CONFLICT);
    let body = search.json();
    assert_eq!(body["error"]["code"], "not_ready");
}

#[tokio::test]
async fn search_document_uses_execution_input_types_for_local_sidecar() {
    let env = TestEnv::new("search-api-document-execution-inputs").await;
    let pdf_path = env.write_test_pdf("fixtures/search/query.pdf", 2);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-document-execution-inputs"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let query_asset = app
        .post_multipart(
            &format!("/libraries/{library_id}/query-assets/documents"),
            Vec::new(),
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "query-document.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                bytes: std::fs::read(pdf_path).expect("query document should be readable"),
            }),
        )
        .await
        .json();
    let temp_asset_id = query_asset["data"]["temp_asset_id"].as_str().unwrap();

    let search = app
        .post_json(
            "/search/document",
            json!({
                "library_id": library_id,
                "document_input": {
                    "kind": "temp_asset",
                    "temp_asset_id": temp_asset_id
                }
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::CONFLICT);
    let body = search.json();
    assert_eq!(body["error"]["code"], "not_ready");
}
