mod support;

use axum::http::{header, StatusCode};
use support::TestEnv;

#[tokio::test]
async fn openapi_api_json_exposes_public_app_contract() {
    let env = TestEnv::new("openapi-api").await;
    let app = env.boot().await;

    let response = app.get_json("/openapi.json").await;

    assert_eq!(response.status, StatusCode::OK);
    let content_type = response
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("application/json"),
        "expected JSON response content type, got {content_type:?}"
    );

    let body = response.json();
    assert_eq!(body["openapi"], "3.1.0");
    assert!(body["info"].is_object(), "OpenAPI info should be present");
    let paths = body["paths"]
        .as_object()
        .expect("OpenAPI paths should be an object");

    for path in [
        "/openapi.json",
        "/health",
        "/routes",
        "/runtime/status",
        "/libraries",
        "/libraries/{library_id}",
        "/libraries/{library_id}/query-assets/images",
        "/libraries/{library_id}/visual-units/{visual_unit_id}/preview",
        "/jobs",
        "/jobs/{job_id}/retry",
        "/search/text",
        "/search/image",
        "/search/video",
        "/search/document",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path {path}");
    }

    assert!(
        !paths.contains_key("/"),
        "human route discovery root must not be in the public App OpenAPI contract"
    );
    assert!(
        !paths.keys().any(|path| path.starts_with("/assets")),
        "static Web assets must not be in the public App OpenAPI contract"
    );

    let legacy_runtime_path = format!("{}-{}", "/runtime", "health");
    assert!(
        !paths.contains_key(&legacy_runtime_path),
        "legacy runtime health route must not be in the public App OpenAPI contract"
    );
    assert!(
        !paths.contains_key("/embed"),
        "sidecar /embed must not be in the public App OpenAPI contract"
    );
    assert!(
        !paths.contains_key("/capabilities"),
        "sidecar /capabilities must not be in the public App OpenAPI contract"
    );

    let preview_response = &paths["/libraries/{library_id}/visual-units/{visual_unit_id}/preview"]
        ["get"]["responses"]["200"];
    let preview_response_json =
        serde_json::to_string(preview_response).expect("preview response should serialize");
    assert!(
        !preview_response_json.contains("SuccessEnvelope"),
        "preview media responses must not be wrapped in SuccessEnvelope"
    );
}
