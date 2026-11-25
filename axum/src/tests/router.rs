//! What every route shares: the bearer token, API paths with no route, and the pages.

use super::*;

#[tokio::test]
async fn a_bearer_token_names_the_user_and_an_unknown_one_is_401() {
    let router = test_router().await;
    let (status, body) = send_as(
        &router.router,
        Some("nope"),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({ "error": "The server did not accept this token" })
    );

    let matched = code_matched(&router, "bob@example.com").await;
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in/confirm",
        Some(json!({ "pending": matched["pending"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "signed up");
    let token = body["token"].as_str().unwrap();
    let (_, bob) = send_as(
        &router.router,
        Some(token),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(bob["id"], body["user"]["id"]);
    let (_, alice) = get(&router, "/api/users/current").await;
    assert_eq!(
        alice["email"],
        json!("alice@example.com"),
        "alice's token is still hers"
    );
}

#[tokio::test]
async fn api_path_without_a_route_is_a_json_404_not_the_pages() {
    let router = test_router().await;
    let (status, body) = get(&router, "/api/no-such-thing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({ "error": "The server has no API route GET /api/no-such-thing" })
    );
    let (status, _) = send(&router, Method::POST, "/api", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = get(&router, "/communities").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!("<html>pages</html>"),
        "a page path is still a page"
    );
}

#[tokio::test]
async fn pages_answer_any_path_with_index_html() {
    let router = test_router().await;
    for path in ["/home", "/communities/new"] {
        let (status, body) = get(&router, path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!("<html>pages</html>"));
    }
}

#[tokio::test]
async fn public_pages_answer_at_their_file_names() {
    let router = test_router().await;
    let cookie = format!("token={}", router.token);
    for cookie in [None, Some(cookie.as_str())] {
        for (path, page) in [
            ("/", "public home"),
            ("/about", "public about"),
            ("/terms", "public terms"),
            ("/privacy", "public privacy"),
            ("/es", "public home es"),
            ("/es/about", "public about es"),
        ] {
            let (status, _headers, body) =
                send_with(&router.router, None, cookie, Method::GET, path, None).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, json!(format!("<html>{page}</html>")));
        }
    }
}
