//! The ops app's routes, for staff only.

use super::*;

#[tokio::test]
async fn ops_is_a_404_to_anyone_who_is_not_staff() {
    let router = test_router().await;
    let cookie = format!("token={}", router.token);
    let (_, missing) = get(&router, "/api/nothing-here").await;
    for path in ["/api/ops/current-staff", "/api/ops/nothing-here"] {
        let (status, _) = send_as(&router.router, None, Method::GET, path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}, nobody signed in");
        let (status, body) = get(&router, path).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{path}, signed in, not staff"
        );
        assert_eq!(
            body["error"].as_str().map(|error| error.replace(path, "")),
            missing["error"]
                .as_str()
                .map(|error| error.replace("/api/nothing-here", "")),
            "the same answer as a path with no route"
        );
    }
    for path in ["/ops", "/ops/users", "/ops/assets/ops.js"] {
        for cookie in [None, Some(cookie.as_str())] {
            let (status, _headers, body) =
                send_with(&router.router, None, cookie, Method::GET, path, None).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(body, Value::Null, "{path}");
        }
    }
}

#[tokio::test]
async fn ops_answers_staff_with_its_api_its_pages_and_its_files() {
    let router = test_router().await;
    let staff = app_core::ops::staff::create(&router.deployment, "alice@example.com")
        .await
        .expect("alice is staff");
    let (status, body) = get(&router, "/api/ops/current-staff").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user_id"], json!(staff.user_id));
    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["is_staff"], json!(true));

    let cookie = format!("token={}", router.token);
    for (path, answer) in [
        ("/ops", "<html>ops</html>"),
        ("/ops/users", "<html>ops</html>"),
        ("/ops/assets/ops.js", "ops script"),
    ] {
        let (status, _headers, body) =
            send_with(&router.router, None, Some(&cookie), Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body, json!(answer), "{path}");
    }
}
