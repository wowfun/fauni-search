mod support;

use axum::http::{header, StatusCode};
use std::fs;
use support::TestEnv;

#[tokio::test]
async fn root_serves_built_web_index_and_assets() {
    let env = TestEnv::new("server-hosted-web-root").await;
    let app = env.boot().await;

    let response = app.get_json("/").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_content_type_starts_with(&response, "text/html");

    let html = response.text();
    let expected_index = fs::read_to_string(env.repo_path("ui/dist/index.html"))
        .expect("ui/dist/index.html should exist for server-hosted Web tests");
    assert_eq!(html, expected_index);
    assert!(html.contains("<div id=\"app\"></div>"));

    let asset_path = first_asset_path(&html);
    let asset_response = app.get_json(&asset_path).await;
    assert_eq!(asset_response.status, StatusCode::OK);
    assert!(
        !asset_response.bytes().is_empty(),
        "served Web asset should not be empty"
    );
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
async fn spa_fallback_does_not_shadow_api_routes() {
    let env = TestEnv::new("server-hosted-web-fallback").await;
    let app = env.boot().await;

    let spa_response = app.get_json("/workspace/search").await;
    assert_eq!(spa_response.status, StatusCode::OK);
    assert_content_type_starts_with(&spa_response, "text/html");
    assert!(spa_response.text().contains("<div id=\"app\"></div>"));

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
        "unknown API-family paths should not fall back to Web HTML"
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

fn first_asset_path(html: &str) -> String {
    let start = html
        .find("/assets/")
        .expect("built index should reference at least one /assets path");
    let rest = &html[start..];
    let end = rest
        .find('"')
        .expect("asset reference should terminate with a quote");
    rest[..end].to_string()
}
