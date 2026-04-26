mod support;

use axum::http::{header, StatusCode};
use support::TestEnv;

#[tokio::test]
async fn root_returns_route_discovery_json() {
    let env = TestEnv::new("api-root-discovery").await;
    let app = env.boot().await;

    let response = app.get_json("/").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_content_type_starts_with(&response, "application/json");

    let body = response.json();
    assert_eq!(body["name"], "fauni-search");
    let routes = body["routes"]
        .as_array()
        .expect("route discovery should include routes array");
    assert!(routes.iter().any(|route| route == "GET /"));
    assert!(routes.iter().any(|route| route == "GET /routes"));
    assert!(routes.iter().any(|route| route == "GET /openapi.json"));
}

#[tokio::test]
async fn route_discovery_moves_to_routes_json() {
    let env = TestEnv::new("server-hosted-web-routes").await;
    let app = env.boot().await;

    let response = app.get_json("/routes").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_content_type_starts_with(&response, "application/json");

    let body = response.json();
    assert_eq!(body["name"], "fauni-search");
    let routes = body["routes"]
        .as_array()
        .expect("route discovery should include routes array");
    assert!(routes.iter().any(|route| route == "GET /"));
    assert!(routes.iter().any(|route| route == "GET /routes"));
    assert!(routes.iter().any(|route| route == "GET /openapi.json"));
}

#[tokio::test]
async fn app_api_server_does_not_serve_web_assets_or_spa_fallback() {
    let env = TestEnv::new("api-root-no-web-assets").await;
    let app = env.boot().await;

    let spa_response = app.get_json("/workspace/search").await;
    assert_eq!(spa_response.status, StatusCode::NOT_FOUND);

    let asset_response = app.get_json("/assets/index-test.js").await;
    assert_eq!(asset_response.status, StatusCode::NOT_FOUND);

    for path in ["/openapi.json", "/health", "/runtime/status"] {
        let response = app.get_json(path).await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "{path} should stay available"
        );
        assert_content_type_starts_with(&response, "application/json");
    }

    let unknown_api_response = app.get_json("/settings").await;
    assert_eq!(
        unknown_api_response.status,
        StatusCode::NOT_FOUND,
        "unknown API-family paths should stay 404 on the App API server"
    );
}

fn assert_content_type_starts_with(response: &support::TestResponse, expected_prefix: &str) {
    let content_type = response
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with(expected_prefix),
        "expected content type to start with {expected_prefix:?}, got {content_type:?}"
    );
}
