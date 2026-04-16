mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::TestEnv;

#[tokio::test]
async fn jobs_endpoints_expose_import_jobs_and_library_filtering() {
    let env = TestEnv::new("jobs-api").await;
    let first_pdf = env.write_test_pdf("fixtures/jobs/first.pdf", 1);
    let second_pdf = env.write_test_pdf("fixtures/jobs/second.pdf", 1);
    let app = env.boot().await;

    let first_library = app
        .post_json(
            "/libraries",
            json!({
                "name": "jobs-first",
                "config": { "enabled_index_lines": ["multivector"] }
            }),
        )
        .await
        .json();
    let first_library_id = first_library["data"]["id"].as_str().unwrap();

    let second_library = app
        .post_json(
            "/libraries",
            json!({
                "name": "jobs-second",
                "config": { "enabled_index_lines": ["multivector"] }
            }),
        )
        .await
        .json();
    let second_library_id = second_library["data"]["id"].as_str().unwrap();

    let first_import = app
        .post_json(
            &format!("/libraries/{first_library_id}/imports"),
            json!({
                "paths": [first_pdf.to_string_lossy().to_string()]
            }),
        )
        .await
        .json();
    let first_job_id = first_import["data"]["job_handle"].as_str().unwrap();

    let second_import = app
        .post_json(
            &format!("/libraries/{second_library_id}/imports"),
            json!({
                "paths": [second_pdf.to_string_lossy().to_string()]
            }),
        )
        .await
        .json();
    let second_job_id = second_import["data"]["job_handle"].as_str().unwrap();

    let filtered_jobs = app
        .get_json(&format!("/jobs?library_id={first_library_id}"))
        .await;
    assert_eq!(filtered_jobs.status, StatusCode::OK);
    let filtered_body = filtered_jobs.json();
    let jobs = filtered_body["data"]["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_id"], first_job_id);
    assert_eq!(jobs[0]["library_id"], first_library_id);
    assert_eq!(jobs[0]["kind"], "import");

    let all_jobs = app.get_json("/jobs").await;
    assert_eq!(all_jobs.status, StatusCode::OK);
    let all_jobs_body = all_jobs.json();
    let all_job_ids = all_jobs_body["data"]["jobs"].as_array().unwrap();
    assert_eq!(all_job_ids.len(), 2);
    assert!(all_job_ids.iter().any(|job| job["job_id"] == first_job_id));
    assert!(all_job_ids.iter().any(|job| job["job_id"] == second_job_id));

    let first_job = app.get_json(&format!("/jobs/{first_job_id}")).await;
    assert_eq!(first_job.status, StatusCode::OK);
    let first_job_body = first_job.json();
    assert_eq!(first_job_body["data"]["job_id"], first_job_id);
    assert_eq!(first_job_body["data"]["library_id"], first_library_id);
    assert_eq!(first_job_body["data"]["kind"], "import");

    let status = first_job_body["data"]["status"].as_str().unwrap();
    assert!(matches!(status, "queued" | "running" | "failed"));
}
