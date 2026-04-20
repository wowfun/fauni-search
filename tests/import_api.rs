mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn import_paths_partially_accepts_and_queues_a_job() {
    let env = TestEnv::new("import-api-partial").await;
    let pdf_path = env.write_test_pdf("fixtures/import/report.pdf", 2);
    let txt_path = env.write_bytes("fixtures/import/nope.txt", b"nope");
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "imports"
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
                    pdf_path.to_string_lossy().to_string(),
                    txt_path.to_string_lossy().to_string()
                ]
            }),
        )
        .await;
    assert_eq!(import.status, StatusCode::OK);
    let body = import.json();
    let job_id = body["data"]["job_handle"].as_str().unwrap();

    assert_eq!(body["data"]["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["accepted"][0]["kind"], "document_page");
    assert_eq!(
        body["data"]["accepted"][0]["visual_units"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        body["data"]["accepted"][0]["visual_units"][0]["locator"]["page"],
        1
    );
    assert_eq!(
        body["data"]["accepted"][0]["visual_units"][1]["locator"]["page"],
        2
    );
    assert_eq!(body["data"]["rejected"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["data"]["rejected"][0]["reason_code"],
        "unsupported_type"
    );
    assert_eq!(body["data"]["job"]["job_id"], job_id);
    assert_eq!(body["data"]["job"]["status"], "queued");
    assert_eq!(body["data"]["job"]["phase"], "intake");
}
