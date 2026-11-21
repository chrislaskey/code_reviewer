//! The app's routes: posts, files, communities, the current user and community, and the sync.

use super::*;

#[tokio::test]
async fn posts_list_as_json() {
    let (router, post) = test_router().await;
    let (status, body) = get(&router, "/api/posts").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([post]));
    assert_eq!(post["owner"]["name"], json!("alice"));
    assert_eq!(
        post["owner"]["email"],
        Value::Null,
        "an email is the account's"
    );
    assert_eq!(post["communities"], json!([]));
}

#[tokio::test]
async fn current_user_is_the_tokens_user_and_nobody_without_one() {
    let (router, _) = test_router().await;
    let (status, body) = get(&router, "/api/current-user").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], json!("alice@example.com"));

    let (status, body) =
        send_as(&router.router, None, Method::GET, "/api/current-user", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({ "error": "Sign in first" }));
    let (status, _) = send_as(&router.router, None, Method::GET, "/api/posts", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn avatars_upload_then_update_with_field_errors_as_json() {
    let (router, _) = test_router().await;
    let wrong = json!({ "content_type": "text/html", "size": 0 });
    let (status, body) = send(&router, Method::POST, "/api/avatars/uploads", Some(wrong)).await;
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
    let (status, upload) = send(&router, Method::POST, "/api/avatars/uploads", Some(picked)).await;
    assert_eq!(status, StatusCode::CREATED);
    let key = upload["key"].as_str().expect("a key").to_string();
    assert_eq!(upload["content_type"], json!("image/png"));

    router.storage.put(&key, png).await.expect("the upload");
    let (status, user) = send(
        &router,
        Method::PUT,
        "/api/avatars",
        Some(json!({ "key": key })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["avatar_url"], json!(format!("memory:///{key}")));
    let (_, me) = get(&router, "/api/current-user").await;
    assert_eq!(me["avatar_url"], user["avatar_url"]);
}

#[tokio::test]
async fn posts_create_refuses_a_body_over_the_maximum_length() {
    let (router, _) = test_router().await;
    let too_long = "a".repeat(app_core::data::app_posts::MAX_BODY_LENGTH + 1);
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/posts",
        Some(json!({ "body": too_long, "community_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({ "error": "a post has 10000 characters at most" })
    );
}

#[tokio::test]
async fn communities_create_join_leave_and_a_post_in_one() {
    let (router, _) = test_router().await;
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

    let (status, created) = send(
        &router,
        Method::POST,
        "/api/posts",
        Some(json!({ "body": "roses", "community_ids": [community_id] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, shown) = get(
        &router,
        &format!("/api/posts/{}", created["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(shown["communities"][0]["name"], json!("Gardening"));

    let path = format!("/api/memberships/{community_id}");
    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = get(&router, "/api/communities").await;
    assert_eq!(listed[0]["membership"], Value::Null);
    let (_, shown) = get(
        &router,
        &format!("/api/posts/{}", created["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(shown["communities"], json!([]), "left with the member");

    let (status, membership) = send(&router, Method::PUT, &path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(membership["community_id"], json!(community_id));
    let (_, shown) = get(
        &router,
        &format!("/api/posts/{}", created["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(shown["communities"][0]["name"], json!("Gardening"));

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
    let (router, _) = test_router().await;
    let token = Some(router.token.as_str());
    let (status, _, personal) = send_with(
        &router.router,
        token,
        None,
        Method::GET,
        "/api/current-community",
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
        "/api/current-community",
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
        "/api/current-community",
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
        "/api/current-community",
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
        "/api/current-community",
        Some(json!({ "community_id": MISSING_ID })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn communities_show_update_and_delete() {
    let (router, _) = test_router().await;
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
async fn posts_get_as_json() {
    let (router, post) = test_router().await;
    let (status, body) = get(
        &router,
        &format!("/api/posts/{}", post["post"]["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, post);
}

#[tokio::test]
async fn posts_get_missing_is_404() {
    let (router, _) = test_router().await;
    let (status, body) = get(&router, &format!("/api/posts/{MISSING_ID}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({ "error": format!("post {MISSING_ID} not found") })
    );
}

#[tokio::test]
async fn posts_get_rejects_an_id_that_is_not_a_uuid() {
    let (router, _) = test_router().await;
    let (status, _) = get(&router, "/api/posts/not-a-uuid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn posts_create() {
    let (router, _) = test_router().await;
    let (status, created) = send(
        &router,
        Method::POST,
        "/api/posts",
        Some(json!({ "body": "a new post" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["body"], json!("a new post"));

    let (_, posts) = get(&router, "/api/posts").await;
    assert_eq!(posts[0]["post"], created);
}

#[tokio::test]
async fn posts_create_refuses_blank() {
    let (router, _) = test_router().await;
    let (status, body) = send(
        &router,
        Method::POST,
        "/api/posts",
        Some(json!({ "body": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, json!({ "error": "a post needs some text" }));
}

#[tokio::test]
async fn posts_update() {
    let (router, post) = test_router().await;
    let path = format!("/api/posts/{}", post["post"]["id"].as_str().unwrap());
    let (status, updated) = send(
        &router,
        Method::PUT,
        &path,
        Some(json!({ "body": "edited" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["body"], json!("edited"));
    assert_eq!(updated["inserted_at"], post["post"]["inserted_at"]);

    let (_, read) = get(&router, &path).await;
    assert_eq!(read["post"], updated);
}

#[tokio::test]
async fn posts_delete() {
    let (router, post) = test_router().await;
    let path = format!("/api/posts/{}", post["post"]["id"].as_str().unwrap());
    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = get(&router, &path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn files_upload_create_show_rename_and_delete() {
    let (router, _) = test_router().await;
    let bytes = b"some notes".to_vec();
    let sha256 = app_core::helpers::sha256::hex(&bytes);
    let picked = json!({ "content_type": "text/plain", "size": bytes.len(), "sha256": sha256 });
    let (status, upload) = send(&router, Method::POST, "/api/files/uploads", Some(picked)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(upload["uploaded"], json!(false));
    let id = upload["id"].as_str().expect("an id").to_string();
    let (_, communities) = get(&router, "/api/communities").await;
    let community_id = communities[0]["community"]["id"].clone();
    let new = json!({
        "id": id,
        "name": "notes.txt",
        "content_type": "text/plain",
        "size": bytes.len(),
        "sha256": sha256,
        "community_ids": [community_id],
    });

    let (status, body) = send(&router, Method::POST, "/api/files", Some(new.clone())).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("the file did not arrive; try again"));

    let (_, me) = get(&router, "/api/current-user").await;
    let key = format!("files/{}/{id}", me["id"].as_str().expect("an id"));
    router.storage.put(&key, bytes).await.expect("the upload");
    let (status, file) = send(&router, Method::POST, "/api/files", Some(new)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(file["name"], json!("notes.txt"));

    let (status, found) = get(&router, "/api/files/search?name=NOTES&limit=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found["total"], json!(1));
    assert_eq!(found["files"][0]["file"], file);
    assert_eq!(found["files"][0]["state"], json!("synced"));
    assert_eq!(found["files"][0]["communities"][0]["id"], community_id);
    let (_, found) = get(&router, "/api/files/search?name=other&limit=10").await;
    assert_eq!(found["total"], json!(0));
    let (status, _) = get(&router, "/api/files/search?limit=501").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let other = app_core::helpers::sha256::hex(b"other");
    let checked = json!({ "sha256s": [other, sha256] });
    let (status, same) = send(
        &router,
        Method::POST,
        "/api/files/duplicates",
        Some(checked),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(same.as_array().expect("a list").len(), 1);
    assert_eq!(same[0]["file"], file);
    let checked = json!({ "sha256s": ["nope"] });
    let (status, _) = send(
        &router,
        Method::POST,
        "/api/files/duplicates",
        Some(checked),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let path = format!("/api/files/{id}");
    let (status, headers, _) = send_with(
        &router.router,
        Some(&router.token),
        None,
        Method::GET,
        &format!("{path}/content"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers[header::LOCATION], format!("memory:///{key}"));
    assert_eq!(headers[header::CACHE_CONTROL], "private, max-age=3300");

    let renamed = json!({ "name": "renamed.txt", "community_ids": [community_id] });
    let (status, file) = send(&router, Method::PUT, &path, Some(renamed)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(file["name"], json!("renamed.txt"));
    let blank = json!({ "name": " ", "community_ids": [community_id] });
    let (status, body) = send(&router, Method::PUT, &path, Some(blank)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["errors"], json!({ "name": ["is blank"] }));

    let (status, _) = send(&router, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = get(&router, &path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&router, &format!("{path}/content")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sync_push_applies_an_event_and_pull_returns_the_row() {
    let (router, post) = test_router().await;
    let event = json!({
        "id": "01900000-0000-7000-8000-000000000001",
        "table": "app_posts",
        "row_id": post["post"]["id"],
        "type": "update",
        "base_version": 1,
        "base": { "body": "the first post" },
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
            "app_posts"
        ],
        "her row, her sign-up's community and the post"
    );
    assert_eq!(body["rows"][3]["row"]["body"], json!("pushed"));
    assert_eq!(body["next"], Value::Null);
    assert_eq!(body["resync"], json!(false));
    assert!(body["cursor"]["last_synced_at"].is_string());
}

#[tokio::test]
async fn sync_push_takes_a_request_up_to_the_most_a_device_sends_and_no_larger() {
    let (router, _) = test_router().await;
    let event = |body: String| {
        json!({
            "id": uuid::Uuid::now_v7(),
            "table": "app_posts",
            "row_id": uuid::Uuid::now_v7(),
            "type": "create",
            "base_version": 0,
            "base": {},
            "changes": { "body": body },
            "made_at": "2026-11-08T10:00:00Z"
        })
    };
    let longest = "a".repeat(app_core::data::app_posts::MAX_BODY_LENGTH);
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

    let over = "a".repeat(app_core::app::sync::MAX_PUSH_BYTES);
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
        "/api/posts",
        Some(json!({ "body": over, "community_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "2 MB elsewhere");
}

#[tokio::test]
async fn sync_push_rejects_a_malformed_request() {
    let (router, _) = test_router().await;
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
    let (router, _) = test_router().await;
    let (status, me) = get(&router, "/api/current-user").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], json!("alice"));
    assert_eq!(me["email"], json!("alice@example.com"));
    assert_eq!(me["is_staff"], json!(false));
}

#[tokio::test]
async fn current_user_update_changes_their_name() {
    let (router, _) = test_router().await;
    let (status, me) = send(
        &router,
        Method::PUT,
        "/api/current-user",
        Some(json!({ "name": " Alice B " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], json!("Alice B"));
    assert_eq!(me["email"], json!("alice@example.com"));
    let (_, me) = get(&router, "/api/current-user").await;
    assert_eq!(me["name"], json!("Alice B"));

    let (status, _) = send(
        &router,
        Method::PUT,
        "/api/current-user",
        Some(json!({ "name": " " })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
