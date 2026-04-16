mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn search_text_rejects_empty_text() {
    let env = TestEnv::new("search-api-validation").await;
    let app = env.boot().await;
    let library = app
        .post_json(
            "/libraries",
            json!({
                "name": "search-validation",
                "config": { "enabled_index_lines": ["multivector"] }
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
                "name": "search-not-ready",
                "config": { "enabled_index_lines": ["multivector"] }
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
        body["error"]["details"]["index_lines"][0]["index_line"],
        "multivector"
    );
    assert_eq!(
        body["error"]["details"]["index_lines"][0]["status"],
        "not_ready"
    );
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
                "name": "search-latest-job",
                "config": { "enabled_index_lines": ["multivector"] }
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
        body["error"]["details"]["index_lines"][0]["index_line"],
        "multivector"
    );
    assert_eq!(
        body["error"]["details"]["index_lines"][0]["status"],
        "not_ready"
    );
    assert_eq!(
        body["error"]["details"]["index_lines"][0]["job"]["job_id"],
        job_id
    );

    let job_status = body["error"]["details"]["index_lines"][0]["job"]["status"]
        .as_str()
        .unwrap();
    assert!(matches!(job_status, "queued" | "running" | "failed"));
    assert!(body["error"]["details"]["index_lines"][0]["job"]["phase"]
        .as_str()
        .is_some());
}
