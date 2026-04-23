mod support;

use axum::http::StatusCode;
use serde_json::json;
use std::env;
use support::TestEnv;
use tokio::time::{sleep, Duration};

async fn wait_for_job_terminal(
    app: &support::TestApp,
    job_id: &str,
    expected_statuses: &[&str],
) -> serde_json::Value {
    for _ in 0..30 {
        let snapshot = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(snapshot.status, StatusCode::OK);
        let body = snapshot.json();
        let status = body["data"]["status"].as_str().unwrap_or_default();
        if expected_statuses.contains(&status) {
            return body;
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("job did not reach expected terminal status for {job_id}");
}

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
                "display_name": "jobs-first"
            }),
        )
        .await
        .json();
    let first_library_id = first_library["data"]["id"].as_str().unwrap();

    let second_library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "jobs-second"
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
    assert_eq!(first_job_body["data"]["retryable"], true);

    let status = first_job_body["data"]["status"].as_str().unwrap();
    assert!(matches!(status, "queued" | "running" | "failed"));
}

#[tokio::test]
async fn cancel_job_endpoint_cancels_a_running_import_job() {
    let env = TestEnv::new_with_qdrant("jobs-api-cancel").await;
    env::set_var("FAUNI_TEST_SIDECAR_EMBED_DELAY_MS", "250");
    let pdf = env.write_test_pdf("fixtures/jobs/cancel.pdf", 1);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "jobs-cancel"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let import = app
        .post_json(
            &format!("/libraries/{library_id}/imports"),
            json!({
                "paths": [pdf.to_string_lossy().to_string()]
            }),
        )
        .await
        .json();
    let job_id = import["data"]["job_handle"].as_str().unwrap().to_string();

    let mut running = None;
    for _ in 0..10 {
        let snapshot = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(snapshot.status, StatusCode::OK);
        let body = snapshot.json();
        let status = body["data"]["status"].as_str().unwrap_or_default();
        if status == "running" {
            running = Some(body);
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }

    let running = running.expect("job should become running before cancellation");
    assert_eq!(running["data"]["cancelable"], true);

    let cancel = app.post_empty(&format!("/jobs/{job_id}/cancel")).await;
    assert_eq!(cancel.status, StatusCode::OK);
    let cancel_body = cancel.json();
    assert_eq!(cancel_body["data"]["job_id"], job_id);
    assert_eq!(cancel_body["data"]["status"], "running");
    assert_eq!(cancel_body["data"]["phase"], "cancel_requested");
    assert_eq!(cancel_body["data"]["cancelable"], false);
    assert!(cancel_body["data"]["current_attempt"]["summary"]
        .as_str()
        .unwrap_or_default()
        .contains("Cancellation requested"));

    for _ in 0..20 {
        let snapshot = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(snapshot.status, StatusCode::OK);
        let body = snapshot.json();
        if body["data"]["status"] == "canceled" {
            assert_eq!(body["data"]["phase"], "canceled");
            return;
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("job did not reach canceled state after cancellation request");
}

#[tokio::test]
async fn resume_job_endpoint_reopens_a_canceled_import_job() {
    let env = TestEnv::new_with_qdrant("jobs-api-resume").await;
    env::set_var("FAUNI_TEST_SIDECAR_EMBED_DELAY_MS", "250");
    let pdf = env.write_test_pdf("fixtures/jobs/resume.pdf", 1);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "jobs-resume"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let import = app
        .post_json(
            &format!("/libraries/{library_id}/imports"),
            json!({
                "paths": [pdf.to_string_lossy().to_string()]
            }),
        )
        .await
        .json();
    let job_id = import["data"]["job_handle"].as_str().unwrap().to_string();

    let mut running = None;
    for _ in 0..10 {
        let snapshot = app.get_json(&format!("/jobs/{job_id}")).await;
        assert_eq!(snapshot.status, StatusCode::OK);
        let body = snapshot.json();
        let status = body["data"]["status"].as_str().unwrap_or_default();
        if status == "running" {
            running = Some(body);
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }

    let running = running.expect("job should become running before cancellation");
    assert_eq!(running["data"]["cancelable"], true);

    let cancel = app.post_empty(&format!("/jobs/{job_id}/cancel")).await;
    assert_eq!(cancel.status, StatusCode::OK);
    let canceled = wait_for_job_terminal(&app, &job_id, &["canceled"]).await;
    assert_eq!(canceled["data"]["status"], "canceled");

    let resume = app.post_empty(&format!("/jobs/{job_id}/resume")).await;
    assert_eq!(resume.status, StatusCode::OK);
    let resume_body = resume.json();
    assert_eq!(resume_body["data"]["job_id"], job_id);
    assert_eq!(resume_body["data"]["status"], "queued");
    assert_eq!(resume_body["data"]["cancelable"], true);
    assert_eq!(resume_body["data"]["current_attempt"]["attempt"], 2);

    let resumed_terminal = wait_for_job_terminal(&app, &job_id, &["completed"]).await;
    assert_eq!(resumed_terminal["data"]["status"], "completed");
    assert_eq!(resumed_terminal["data"]["current_attempt"]["attempt"], 2);
}

#[tokio::test]
async fn retry_job_endpoint_requeues_a_canceled_manual_source_action() {
    let env = TestEnv::new_with_qdrant("jobs-api-retry").await;
    env::set_var("FAUNI_TEST_SIDECAR_EMBED_DELAY_MS", "250");
    let root_dir = env.create_dir("fixtures/jobs/retry-root");
    env.write_test_pdf("fixtures/jobs/retry-root/report.pdf", 1);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "jobs-retry"
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

    let refresh = app
        .post_empty(&format!(
            "/libraries/{library_id}/source-roots/{source_root_id}/refresh"
        ))
        .await;
    assert_eq!(refresh.status, StatusCode::OK);
    let refresh_body = refresh.json();
    let job_id = refresh_body["data"]["job_handle"]
        .as_str()
        .unwrap()
        .to_string();

    let cancel = app.post_empty(&format!("/jobs/{job_id}/cancel")).await;
    assert_eq!(cancel.status, StatusCode::OK);

    let canceled = wait_for_job_terminal(&app, &job_id, &["canceled"]).await;
    assert_eq!(canceled["data"]["status"], "canceled");
    assert_eq!(canceled["data"]["retryable"], true);

    let retry = app.post_empty(&format!("/jobs/{job_id}/retry")).await;
    assert_eq!(retry.status, StatusCode::OK);
    let retry_body = retry.json();
    let retried_job_id = retry_body["data"]["job_id"].as_str().unwrap();
    assert_ne!(retried_job_id, job_id);
    assert_eq!(retry_body["data"]["status"], "queued");
    assert_eq!(retry_body["data"]["cancelable"], true);
    assert_eq!(retry_body["data"]["retryable"], true);
    assert_eq!(retry_body["data"]["retried_from_job_id"], job_id);
    assert_eq!(retry_body["data"]["current_attempt"]["attempt"], 2);

    let retried_terminal = wait_for_job_terminal(&app, retried_job_id, &["completed"]).await;
    assert_eq!(retried_terminal["data"]["status"], "completed");
    assert_eq!(retried_terminal["data"]["kind"], "refresh");
    assert_eq!(retried_terminal["data"]["retried_from_job_id"], job_id);
    assert_eq!(retried_terminal["data"]["current_attempt"]["attempt"], 2);
}

#[tokio::test]
async fn retry_job_endpoint_requeues_a_canceled_import_without_duplicating_sources() {
    let env = TestEnv::new_with_qdrant("jobs-api-import-retry").await;
    env::set_var("FAUNI_TEST_SIDECAR_EMBED_DELAY_MS", "250");
    let pdf = env.write_test_pdf("fixtures/jobs/retry-import.pdf", 1);
    let app = env.boot().await;

    let library = app
        .post_json(
            "/libraries",
            json!({
                "display_name": "jobs-import-retry"
            }),
        )
        .await
        .json();
    let library_id = library["data"]["id"].as_str().unwrap();

    let import = app
        .post_json(
            &format!("/libraries/{library_id}/imports"),
            json!({
                "paths": [pdf.to_string_lossy().to_string()]
            }),
        )
        .await
        .json();
    let job_id = import["data"]["job_handle"].as_str().unwrap().to_string();

    let cancel = app.post_empty(&format!("/jobs/{job_id}/cancel")).await;
    assert_eq!(cancel.status, StatusCode::OK);

    let canceled = wait_for_job_terminal(&app, &job_id, &["canceled"]).await;
    assert_eq!(canceled["data"]["status"], "canceled");
    assert_eq!(canceled["data"]["kind"], "import");
    assert_eq!(canceled["data"]["retryable"], true);

    let retry = app.post_empty(&format!("/jobs/{job_id}/retry")).await;
    assert_eq!(retry.status, StatusCode::OK);
    let retry_body = retry.json();
    let retried_job_id = retry_body["data"]["job_id"].as_str().unwrap();
    assert_ne!(retried_job_id, job_id);
    assert_eq!(retry_body["data"]["kind"], "import");
    assert_eq!(retry_body["data"]["retryable"], true);
    assert_eq!(retry_body["data"]["retried_from_job_id"], job_id);
    assert_eq!(retry_body["data"]["current_attempt"]["attempt"], 2);

    let retried_terminal = wait_for_job_terminal(&app, retried_job_id, &["completed"]).await;
    assert_eq!(retried_terminal["data"]["status"], "completed");
    assert_eq!(retried_terminal["data"]["kind"], "import");
    assert_eq!(retried_terminal["data"]["retried_from_job_id"], job_id);
    assert_eq!(retried_terminal["data"]["current_attempt"]["attempt"], 2);

    let sources = app
        .get_json(&format!("/libraries/{library_id}/sources"))
        .await;
    assert_eq!(sources.status, StatusCode::OK);
    let sources_body = sources.json();
    assert_eq!(sources_body["data"]["sources"].as_array().unwrap().len(), 1);
}
