//! What the server says it is, and the servers it lists.

use super::*;

#[tokio::test]
async fn servers_current_get_describes_the_server_to_anyone() {
    let (router, _) = test_router().await;
    let (status, _headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::GET,
        "/api/servers/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "app_identifier": app_core::app::identity::APP_IDENTIFIER,
            "protocol_version": app_core::sync::PROTOCOL_VERSION,
            "name": "test",
            "display_name": "Test Server",
        })
    );
}

#[tokio::test]
async fn servers_get_answers_only_the_name_and_the_origin_together() {
    let (router, _) = test_router().await;
    let ask = |path: &'static str| {
        let router = router.router.clone();
        async move { send_with(&router, None, None, Method::GET, path, None).await }
    };

    let (status, _, body) = ask("/api/servers/acme?url=https%3A%2F%2FAcme.Example.com%2F").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "name": "acme", "url": "https://acme.example.com" })
    );

    let (wrong_origin, _, wrong_origin_body) =
        ask("/api/servers/acme?url=https%3A%2F%2Fevil.example.com").await;
    let (unknown_name, _, unknown_name_body) =
        ask("/api/servers/evil?url=https%3A%2F%2Facme.example.com").await;
    assert_eq!(wrong_origin, StatusCode::NOT_FOUND);
    assert_eq!(unknown_name, StatusCode::NOT_FOUND);
    assert_eq!(
        wrong_origin_body, unknown_name_body,
        "neither half alone tells anything"
    );

    let (no_url, _, _) = ask("/api/servers/acme").await;
    assert_eq!(no_url, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sync_routes_refuse_another_protocol_version_426() {
    let (router, _) = test_router().await;
    let pull = |version: Option<&str>| {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/sync/pull")
            .header(header::AUTHORIZATION, format!("Bearer {}", router.token))
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(version) = version {
            request = request.header(app_core::sync::PROTOCOL_VERSION_HEADER, version);
        }
        let request = request.body(Body::from("{}")).expect("a request");
        let router = router.router.clone();
        async move { router.oneshot(request).await.expect("a response").status() }
    };
    let own = app_core::sync::PROTOCOL_VERSION.to_string();
    assert_eq!(pull(Some(&own)).await, StatusCode::OK);
    assert_eq!(pull(None).await, StatusCode::UPGRADE_REQUIRED);
    let newer = (app_core::sync::PROTOCOL_VERSION + 1).to_string();
    assert_eq!(pull(Some(&newer)).await, StatusCode::UPGRADE_REQUIRED);
}
