//! The app's routes: communities, the current user and community, and the sync;
//! reviews, traces and sessions in `host`.

use super::*;
use app_core::sync::test_notes;

mod host;

#[tokio::test]
async fn current_user_is_the_tokens_user_and_nobody_without_one() {
    let router = test_router().await;
    let (status, body) = get(&router, "/api/users/current").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], json!("alice@example.com"));

    let (status, body) = send_as(
        &router.router,
        None,
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({ "error": "Sign in first" }));
    let (status, _) = send_as(&router.router, None, Method::GET, "/api/communities", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn avatars_upload_then_update_with_field_errors_as_json() {
    let router = test_router().await;
    let wrong = json!({ "content_type": "text/html", "size": 0 });
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/users/current/avatar/uploads",
        Some(wrong),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["errors"],
        json!({
            "content_type": ["must be a PNG, JPEG, GIF or WebP picture"],
            "size": ["is empty"],
        })
    );

    let png = b"\x89PNG\r\n\x1a\nthe rest".to_vec();
    let picked = json!({ "content_type": "image/png", "size": png.len() });
    let (status, upload) = send(
        &router,
        Method::POST,
        "/api/users/current/avatar/uploads",
        Some(picked),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let key = upload["key"].as_str().expect("a key").to_string();
    assert_eq!(upload["content_type"], json!("image/png"));

    router.storage.put(&key, png).await.expect("the upload");
    let (status, user) = send(
        &router,
        Method::PUT,
        "/api/users/current/avatar",
        Some(json!({ "key": key })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["avatar_url"], json!(format!("memory:///{key}")));
    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["avatar_url"], user["avatar_url"]);
}

#[tokio::test]
async fn communities_create_join_and_leave() {
    let router = test_router().await;
    let (status, community) = send(
        &router,
        Method::POST,
        "/api/communities",
        Some(json!({ "name": "Gardening" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let community_id = community["id"].as_str().unwrap().to_string();

    let (status, listed) = get(&router, "/api/communities").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["community"], community);
    assert_eq!(listed[0]["membership"]["role"], json!("member"));

    let path = format!("/api/communities/{community_id}/membership");
    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = get(&router, "/api/communities").await;
    assert_eq!(listed[0]["membership"], Value::Null);

    let (status, membership) = send(&router, Method::POST, &path, None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(membership["community_id"], json!(community_id));

    let (status, _) = send(
        &router,
        Method::POST,
        "/api/sync/backfill",
        Some(json!({ "community_id": community_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &router,
        Method::POST,
        "/api/sync/backfill",
        Some(json!({ "community_id": MISSING_ID })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn current_community_settles_on_the_first_and_keeps_a_choice_in_its_cookie() {
    let router = test_router().await;
    let token = Some(router.token.as_str());
    let (status, _, personal) = send_with(
        &router.router,
        token,
        None,
        Method::GET,
        "/api/communities/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(personal["name"], json!("Personal"), "her sign-up's");

    let (_, weeding) = send(
        &router,
        Method::POST,
        "/api/communities",
        Some(json!({ "name": "Weeding" })),
    )
    .await;
    let (_, baking) = send(
        &router,
        Method::POST,
        "/api/communities",
        Some(json!({ "name": "Baking" })),
    )
    .await;
    let (_, headers, first) = send_with(
        &router.router,
        token,
        None,
        Method::GET,
        "/api/communities/current",
        None,
    )
    .await;
    assert_eq!(first, baking, "the first by name");
    let set = headers[header::SET_COOKIE].to_str().unwrap();
    assert!(set.starts_with(&format!(
        "current_community_id={}",
        baking["id"].as_str().unwrap()
    )));

    let (status, headers, chosen) = send_with(
        &router.router,
        token,
        None,
        Method::PUT,
        "/api/communities/current",
        Some(json!({ "community_id": weeding["id"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(chosen, weeding);
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let (_, headers, kept) = send_with(
        &router.router,
        token,
        Some(&cookie),
        Method::GET,
        "/api/communities/current",
        None,
    )
    .await;
    assert_eq!(kept, weeding);
    assert!(
        headers.get(header::SET_COOKIE).is_none(),
        "nothing to store"
    );

    let (status, _, _) = send_with(
        &router.router,
        token,
        None,
        Method::PUT,
        "/api/communities/current",
        Some(json!({ "community_id": MISSING_ID })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn communities_show_update_and_delete() {
    let router = test_router().await;
    let (_, community) = send(
        &router,
        Method::POST,
        "/api/communities",
        Some(json!({ "name": "Gardening" })),
    )
    .await;
    let path = format!("/api/communities/{}", community["id"].as_str().unwrap());

    let (status, shown) = get(&router, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shown["community"], community);
    assert_eq!(shown["membership"]["role"], json!("member"));

    let (status, updated) = send(
        &router,
        Method::PUT,
        &path,
        Some(json!({ "name": "Weeding" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], json!("Weeding"));
    let (status, body) = send(&router, Method::PUT, &path, Some(json!({ "name": " " }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["errors"], json!({ "name": ["is blank"] }));

    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = get(&router, &path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, listed) = get(&router, "/api/communities").await;
    assert_eq!(listed.as_array().unwrap().len(), 1, "her sign-up's is left");
    assert_eq!(listed[0]["community"]["name"], json!("Personal"));
    let (status, _) = get(&router, &format!("/api/communities/{MISSING_ID}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sync_push_applies_an_event_and_pull_returns_the_row() {
    let router = test_router().await;
    let alice = app_core::app::users::get_by_token(&router.deployment, &router.token)
        .await
        .expect("alice");
    let note = test_notes::create(&router.deployment, &alice, "the first note", None)
        .await
        .expect("a note");
    let event = json!({
        "id": "01900000-0000-7000-8000-000000000001",
        "table": test_notes::TABLE,
        "row_id": note.id,
        "type": "update",
        "base_version": 1,
        "base": { "body": "the first note" },
        "changes": { "body": "pushed" },
        "made_at": "2026-11-08T10:00:00Z"
    });
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/sync/push",
        Some(json!({ "events": [event] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["events"][0]["status"], json!("applied"));
    assert_eq!(body["events"][0]["version"], json!(2));
    assert_eq!(body["events"][0]["server"]["row"]["body"], json!("pushed"));

    let (status, body) = send(&router, Method::POST, "/api/sync/pull", Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let tables: Vec<&str> = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["table"].as_str().unwrap())
        .collect();
    assert_eq!(
        tables,
        [
            "app_users",
            "app_communities",
            "app_users_communities",
            test_notes::TABLE
        ],
        "her row, her sign-up's community and the note"
    );
    assert_eq!(body["rows"][3]["row"]["body"], json!("pushed"));
    assert_eq!(body["next"], Value::Null);
    assert_eq!(body["resync"], json!(false));
    assert!(body["cursor"]["global_sync_version"].is_string());
}

#[tokio::test]
async fn sync_push_takes_a_request_up_to_the_most_a_device_sends_and_no_larger() {
    let router = test_router().await;
    let event = |body: String| {
        json!({
            "id": uuid::Uuid::now_v7(),
            "table": test_notes::TABLE,
            "row_id": uuid::Uuid::now_v7(),
            "type": "create",
            "base_version": 0,
            "base": {},
            "changes": { "body": body },
            "made_at": "2026-11-08T10:00:00Z"
        })
    };
    let longest = "a".repeat(test_notes::MAX_BODY_LENGTH);
    let four_megabytes: Vec<Value> = (0..400).map(|_| event(longest.clone())).collect();
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/sync/push",
        Some(json!({ "events": four_megabytes })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "over the 2 MB of the other routes");
    assert_eq!(body["events"].as_array().expect("answers").len(), 400);

    let over = "a".repeat(app_core::sync::MAX_PUSH_BYTES);
    let (status, _) = send(
        &router,
        Method::POST,
        "/api/sync/push",
        Some(json!({ "events": [event(over.clone())] })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, _) = send(
        &router,
        Method::POST,
        "/api/communities",
        Some(json!({ "name": over })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "2 MB elsewhere");
}

#[tokio::test]
async fn sync_push_rejects_a_malformed_request() {
    let router = test_router().await;
    let (status, _) = send(
        &router,
        Method::POST,
        "/api/sync/push",
        Some(json!({ "events": [{ "id": "x" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn current_user_get_shows_their_email_and_whether_they_are_staff() {
    let router = test_router().await;
    let (status, me) = get(&router, "/api/users/current").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], json!("alice"));
    assert_eq!(me["email"], json!("alice@example.com"));
    assert_eq!(me["is_staff"], json!(false));
}

#[tokio::test]
async fn current_user_update_changes_their_name() {
    let router = test_router().await;
    let (status, me) = send(
        &router,
        Method::PUT,
        "/api/users/current",
        Some(json!({ "name": " Alice B " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], json!("Alice B"));
    assert_eq!(me["email"], json!("alice@example.com"));
    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["name"], json!("Alice B"));

    let (status, _) = send(
        &router,
        Method::PUT,
        "/api/users/current",
        Some(json!({ "name": " " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
