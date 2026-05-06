mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
};
use support::{MultipartFile, TestEnv};
use tokio::time::{sleep, Duration};

fn example_video_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    [
        "data/example/lib1/generate_q2_report_from_csv_bank_data-720-512.mp4",
        "data/generate_q2_report_from_csv_bank_data-720-512.mp4",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .find(|path| path.exists())
    .unwrap_or_else(|| {
        root.join("data/example/lib1/generate_q2_report_from_csv_bank_data-720-512.mp4")
    })
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

fn test_file_uri(path: &Path) -> String {
    let path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    format!("file://{path}")
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

    let history = app.get_json("/queries/history").await;
    assert_eq!(history.status, StatusCode::OK);
    assert!(history.json()["data"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
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
    assert!(results
        .iter()
        .all(|item| item["asset_type"] == "document_page"));
    assert!(results.iter().all(|item| item["source_type"] == "pdf"));
    assert!(results.iter().all(|item| {
        item["source_uri"]
            .as_str()
            .map(|value| value.ends_with("ready.pdf"))
            .unwrap_or(false)
    }));
    assert!(results.iter().all(|item| item.get("visibility").is_none()));
    assert_eq!(data["next_cursor"], serde_json::Value::Null);
    assert_eq!(data["unsupported_content_types"], serde_json::Value::Null);
    assert_eq!(
        data["debug"]["vector_type"],
        "multi_vector_late_interaction"
    );
    assert_eq!(
        data["debug"][concat!("repr", "_kind")],
        serde_json::Value::Null
    );
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
    assert_eq!(data["debug"]["prefilter"][0]["mode"], "point_allow_list");
    assert_eq!(data["debug"]["prefilter"][0]["candidate_point_count"], 2);

    let history = app.get_json("/queries/history").await;
    assert_eq!(history.status, StatusCode::OK);
    let history_body = history.json();
    let items = history_body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["query_kind"], "text");
    assert_eq!(items[0]["input_kind"], "inline_text");
    assert_eq!(items[0]["input_summary"], "chart");
    assert_eq!(items[0]["status"], "completed");
    assert_eq!(items[0]["result_count"], 2);
    assert_eq!(items[0]["input_available"], true);
    let query_id = items[0]["query_id"].as_str().unwrap();

    let detail = app.get_json(&format!("/queries/history/{query_id}")).await;
    assert_eq!(detail.status, StatusCode::OK);
    let detail_body = detail.json();
    assert_eq!(detail_body["data"]["input"]["text"], "chart");
    assert_eq!(
        detail_body["data"]["search_scope"]["library_id"],
        library_id
    );
}

#[tokio::test]
async fn search_prefilter_applies_path_prefix_before_qdrant_allow_list() {
    let env = TestEnv::new_with_qdrant("search-api-prefilter-path").await;
    let first_pdf = env.write_test_pdf("fixtures/search/scope-a.pdf", 2);
    let second_pdf = env.write_test_pdf("fixtures/search/scope-b.pdf", 2);
    let first_uri = test_file_uri(&first_pdf);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-prefilter-path"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let import = app
        .post_json(
            &format!("/libraries/{library_id}/imports"),
            json!({
                "paths": [
                    first_pdf.to_string_lossy().to_string(),
                    second_pdf.to_string_lossy().to_string(),
                ]
            }),
        )
        .await
        .json();
    let job_id = import["data"]["job_handle"].as_str().unwrap();
    let job_body = wait_for_job_completion(&app, job_id).await;
    assert_eq!(job_body["data"]["status"], "completed");

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": library_id,
                "text": "chart",
                "debug": true,
                "target_content_types": ["document"],
                "filters": {
                    "path_prefix": first_uri
                }
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::OK);
    let body = search.json();
    let data = &body["data"];
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| item["source_uri"] == first_uri));
    assert_eq!(data["debug"]["prefilter"][0]["mode"], "point_allow_list");
    assert_eq!(data["debug"]["prefilter"][0]["candidate_point_count"], 2);
    assert_eq!(
        data["debug"]["content_types"][0]["raw_scores"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn search_image_all_libraries_uses_global_query_asset_and_records_history() {
    let env = TestEnv::new_with_qdrant("search-api-global-image-query").await;
    let pdf_path = env.write_test_pdf("fixtures/search/source.pdf", 1);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-global-image"
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
    let job_body = wait_for_job_completion(&app, job_id).await;
    assert_eq!(job_body["data"]["status"], "completed");

    let upload = app
        .post_multipart(
            "/query-assets/images",
            vec![],
            Some(MultipartFile {
                field_name: "file".to_string(),
                filename: "query.png".to_string(),
                content_type: "image/png".to_string(),
                bytes: b"query image bytes".to_vec(),
            }),
        )
        .await;
    assert_eq!(upload.status, StatusCode::CREATED);
    let upload_body = upload.json();
    let temp_asset_id = upload_body["data"]["temp_asset_id"].as_str().unwrap();
    assert!(upload_body["data"]["preview"]["url"]
        .as_str()
        .unwrap()
        .contains("/query-assets/images/"));

    let search = app
        .post_json(
            "/search/image",
            json!({
                "search_scope": { "kind": "all_libraries" },
                "image_input": {
                    "kind": "temp_asset",
                    "temp_asset_id": temp_asset_id
                },
                "target_content_types": ["document"],
                "debug": true
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::OK);
    let body = search.json();
    let data = &body["data"];
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["asset_type"], "document_page");
    assert_eq!(results[0]["library_id"], library_id);
    assert_eq!(data["debug"]["prefilter"][0]["mode"], "point_allow_list");

    let history = app.get_json("/queries/history?query_kind=image").await;
    assert_eq!(history.status, StatusCode::OK);
    let history_body = history.json();
    let items = history_body["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["query_kind"], "image");
    assert_eq!(items[0]["input_kind"], "query_asset");
    assert_eq!(items[0]["input_summary"], temp_asset_id);
    assert_eq!(items[0]["scope_summary"], "all_libraries");
    assert_eq!(items[0]["input_available"], true);
}

#[tokio::test]
async fn search_prefilter_empty_filters_do_not_require_qdrant() {
    let env = TestEnv::new_with_qdrant("search-api-prefilter-empty").await;
    let pdf_path = env.write_test_pdf("fixtures/search/filter-empty.pdf", 2);
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "search-prefilter-empty"
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
    let job_body = wait_for_job_completion(&app, job_id).await;
    assert_eq!(job_body["data"]["status"], "completed");

    env::set_var("QDRANT_URL", "http://127.0.0.1:1");
    for filters in [
        json!({ "asset_type": "image" }),
        json!({ "source_type": "image" }),
        json!({ "time_range": { "start_ms": 1, "end_ms": 2 } }),
    ] {
        let search = app
            .post_json(
                "/search/text",
                json!({
                    "library_id": library_id,
                    "text": "chart",
                    "debug": true,
                    "target_content_types": ["document"],
                    "filters": filters
                }),
            )
            .await;
        assert_eq!(search.status, StatusCode::OK);
        let body = search.json();
        assert!(body["data"]["results"].as_array().unwrap().is_empty());
        assert_eq!(
            body["data"]["debug"]["prefilter"][0]["candidate_point_count"],
            0
        );
    }
}

#[tokio::test]
async fn same_source_content_reuses_global_units_across_libraries() {
    let env = TestEnv::new_with_qdrant("search-api-global-reuse").await;
    let pdf_path = env.write_test_pdf("fixtures/search/shared.pdf", 2);
    let app = env.boot().await;

    let first_library = app
        .post_json("/libraries", json!({ "display_name": "global-reuse-a" }))
        .await
        .json();
    let first_library_id = first_library["data"]["id"].as_str().unwrap();
    let first_import = app
        .post_json(
            &format!("/libraries/{first_library_id}/imports"),
            json!({ "paths": [pdf_path.to_string_lossy().to_string()] }),
        )
        .await
        .json();
    let first_job_id = first_import["data"]["job_handle"].as_str().unwrap();
    let first_job = wait_for_job_completion(&app, first_job_id).await;
    assert!(first_job["data"]["current_attempt"]["summary"]
        .as_str()
        .unwrap()
        .contains("indexed 2 unit(s)"));

    let second_library = app
        .post_json("/libraries", json!({ "display_name": "global-reuse-b" }))
        .await
        .json();
    let second_library_id = second_library["data"]["id"].as_str().unwrap();
    let second_import = app
        .post_json(
            &format!("/libraries/{second_library_id}/imports"),
            json!({ "paths": [pdf_path.to_string_lossy().to_string()] }),
        )
        .await
        .json();
    let second_job_id = second_import["data"]["job_handle"].as_str().unwrap();
    let second_job = wait_for_job_completion(&app, second_job_id).await;
    assert!(second_job["data"]["current_attempt"]["summary"]
        .as_str()
        .unwrap()
        .contains("indexed 0 unit(s)"));

    let search = app
        .post_json(
            "/search/text",
            json!({
                "library_id": second_library_id,
                "text": "chart",
                "debug": true,
                "target_content_types": ["document"]
            }),
        )
        .await;
    assert_eq!(search.status, StatusCode::OK);
    let body = search.json();
    let results = body["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .all(|item| item["library_id"] == second_library_id));
    assert_eq!(
        body["data"]["debug"]["prefilter"][0]["candidate_point_count"],
        2
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
