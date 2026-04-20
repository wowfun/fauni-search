mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::path::PathBuf;
use support::{MultipartFile, TestEnv};

fn example_video_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/example/generate_q2_report_from_csv_bank_data-720-512.mp4")
}

#[tokio::test]
async fn search_text_rejects_empty_text() {
    let env = TestEnv::new("search-api-validation").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "name": "search-validation"
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
                "name": "search-not-ready"
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
                "name": "search-target-content-types"
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
                "name": "search-latest-job"
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
async fn search_video_uses_execution_input_types_for_local_sidecar() {
    let env = TestEnv::new("search-api-video-execution-inputs").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "name": "search-video-execution-inputs"
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
                bytes: std::fs::read(example_video_path()).expect("example video should be readable"),
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
                "name": "search-document-execution-inputs"
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
