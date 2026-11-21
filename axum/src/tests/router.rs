//! What every route shares: the bearer token, API paths with no route, and the pages.

use super::*;

#[tokio::test]
async fn a_bearer_token_names_the_user_and_an_unknown_one_is_401() {
    let (router, post) = test_router().await;
    let (status, body) = send_as(
        &router.router,
        Some("nope"),
        Method::GET,
        "/api/posts",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({ "error": "The server did not accept this token" })
    );

    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-up",
        Some(json!({ "email": "bob@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let code = code_sent_to(&router.mailer, "bob@example.com");
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code",
        Some(json!({ "pending": body["pending"], "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in/new-password",
        Some(json!({ "pending": body["pending"], "password": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "skipped making a password");
    let token = body["token"].as_str().unwrap();
    let (status, created) = send_as(
        &router.router,
        Some(token),
        Method::POST,
        "/api/posts",
        Some(json!({ "body": "bob's" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["user_id"], body["user"]["id"]);
    let (_, bobs) = send_as(&router.router, Some(token), Method::GET, "/api/posts", None).await;
    assert_eq!(bobs[0]["post"], created);
    let (_, alices) = get(&router, "/api/posts").await;
    assert_eq!(alices, json!([post]), "alice does not see bob's post");
}

#[tokio::test]
async fn api_path_without_a_route_is_a_json_404_not_the_pages() {
    let (router, _) = test_router().await;
    let (status, body) = get(&router, "/api/no-such-thing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({ "error": "The server has no API route GET /api/no-such-thing" })
    );
    let (status, _) = send(&router, Method::POST, "/api", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = get(&router, "/posts").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!("<html>pages</html>"),
        "a page path is still a page"
    );
}

#[tokio::test]
async fn pages_answer_any_path_with_index_html() {
    let (router, _) = test_router().await;
    for path in ["/home", "/posts/new"] {
        let (status, body) = get(&router, path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!("<html>pages</html>"));
    }
}

#[tokio::test]
async fn public_pages_answer_at_their_file_names() {
    let (router, _) = test_router().await;
    let cookie = format!("token={}", router.token);
    for cookie in [None, Some(cookie.as_str())] {
        for (path, page) in [
            ("/", "public home"),
            ("/about", "public about"),
            ("/terms", "public terms"),
            ("/privacy", "public privacy"),
        ] {
            let (status, _headers, body) =
                send_with(&router.router, None, cookie, Method::GET, path, None).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, json!(format!("<html>{page}</html>")));
        }
    }
}
