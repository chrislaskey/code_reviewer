//! The accounts app's routes: signing up and in, trials, rate limits, deletions and purchasing.

use super::*;
use crate::rate_limits;

#[tokio::test]
async fn accounts_sign_in_of_a_new_email_the_code_and_the_last_step_then_sign_out_over_the_cookie()
{
    let (router, _) = test_router().await;
    let bob = json!({ "email": "Bob@example.com" });
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in",
        Some(bob.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let pending = body["pending"].as_str().expect("the pending token");
    let code = code_sent_to(&router.mailer, "bob@example.com");

    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code",
        Some(json!({ "pending": pending, "code": "000000" })),
    )
    .await;
    if code != "000000" {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .starts_with("That code is wrong")
        );
    }
    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code/resend",
        Some(json!({ "pending": pending })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let code = code_sent_to(&router.mailer, "bob@example.com");
    let (status, headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/code",
        Some(json!({ "pending": pending, "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["new_account"], json!(true), "a sign-up");
    assert!(headers.get(header::SET_COOKIE).is_none(), "not finished");
    let pending = body["pending"].as_str().expect("the pending token");
    let (status, headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/sign-in/confirm",
        Some(json!({ "pending": pending })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], json!("bob@example.com"));
    assert_eq!(body["user"]["name"], json!("bob"));
    let set_cookie = headers[header::SET_COOKIE].to_str().expect("a cookie");
    assert!(set_cookie.starts_with(&format!("token={}", body["token"].as_str().unwrap())));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Secure"), "no Host, so not localhost");
    let cookie = set_cookie.split(';').next().unwrap().to_string();

    let (status, _, me) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], json!("bob@example.com"));
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::PUT,
        "/api/accounts/current/password",
        Some(json!({ "password": "short" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::PUT,
        "/api/accounts/current/password",
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, password) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/accounts/current/password",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(password, json!({ "is_set": true }));
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::PUT,
        "/api/accounts/current/password",
        Some(json!({ "password": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "no current one asked");
    assert_eq!(
        router
            .mailer
            .last_sent_to("bob@example.com")
            .unwrap()
            .subject,
        "Your password was changed"
    );
    let (_, _, posts) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/posts",
        None,
    )
    .await;
    assert_eq!(posts, json!([]), "bob does not see alice's post");

    let (status, headers, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::DELETE,
        "/api/accounts/sign-out",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let cleared = headers[header::SET_COOKIE].to_str().expect("a cookie");
    assert!(cleared.starts_with("token=;"), "{cleared}");
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let body = code_matched(&router, "bob@example.com").await;
    assert_eq!(body["new_account"], json!(false), "a sign-in now");
    assert!(body.get("token").is_none());

    for (email, password) in [
        ("bob@example.com", PASSWORD),
        ("nobody@example.com", NEW_PASSWORD),
    ] {
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in/password",
            Some(json!({ "email": email, "password": password })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{email}");
        assert_eq!(
            body,
            json!({ "error": "That email and password don't match" })
        );
    }
    let (status, headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/sign-in/password",
        Some(json!({ "email": "Bob@example.com", "password": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], json!("bob@example.com"));
    assert!(headers.get(header::SET_COOKIE).is_some());
}

#[tokio::test]
async fn accounts_password_tries_are_limited_per_address_and_the_code_still_works() {
    let (router, _) = test_router().await;
    for _ in 0..rate_limits::PASSWORD_TRIES_PER_ADDRESS {
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in/password",
            Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in/password",
        Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("log in with an emailed code")
    );
    let body = code_matched(&router, "bob@example.com").await;
    assert!(body["pending"].is_string(), "the code is not limited by it");
}

#[tokio::test]
async fn accounts_keep_signed_in_makes_the_cookie_outlast_the_browser_on_every_last_step() {
    let (router, _) = test_router().await;
    let sixty_days = "Max-Age=5184000";

    let body = code_matched(&router, "bob@example.com").await;
    let cookie = set_cookie_of(
        &router.router,
        "/api/accounts/sign-in/confirm",
        json!({ "pending": body["pending"], "keep_signed_in": true }),
    )
    .await
    .expect("a cookie");
    assert!(cookie.contains(sixty_days), "{cookie}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let session = cookie.split(';').next().unwrap().to_string();
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&session),
        Method::PUT,
        "/api/accounts/current/password",
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let cookie = set_cookie_of(
        &router.router,
        "/api/accounts/sign-in/password",
        json!({ "email": "bob@example.com", "password": PASSWORD, "keep_signed_in": true }),
    )
    .await
    .expect("a cookie");
    assert!(cookie.contains(sixty_days), "{cookie}");

    let cookie = set_cookie_of(
        &router.router,
        "/api/accounts/sign-in/password",
        json!({ "email": "bob@example.com", "password": PASSWORD }),
    )
    .await
    .expect("a cookie");
    assert!(!cookie.contains("Max-Age"), "until the browser closes");

    let (_verifier, challenge) = app_core::accounts::verifier_and_challenge();
    let cookie = set_cookie_of(
        &router.router,
        "/api/accounts/sign-in/password",
        json!({ "email": "bob@example.com", "password": PASSWORD, "challenge": challenge }),
    )
    .await;
    assert!(cookie.is_none(), "a device's sign-in sets no cookie");

    let (_verifier, challenge) = app_core::accounts::verifier_and_challenge();
    let body = code_matched(&router, "alice@example.com").await;
    let cookie = set_cookie_of(
        &router.router,
        "/api/accounts/sign-in/confirm",
        json!({ "pending": body["pending"], "challenge": challenge, "keep_signed_in": true }),
    )
    .await;
    assert!(cookie.is_none(), "a device's sign-in sets no cookie");
}

#[tokio::test]
async fn accounts_trial_makes_a_user_whose_cookie_lasts_two_days_and_who_signs_up_as_themselves() {
    let (router, _) = test_router().await;
    let (status, available) = send_as(
        &router.router,
        None,
        Method::GET,
        "/api/accounts/trial",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(available, json!({ "available": true }));

    let (status, headers, trial) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/trial",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        trial["user"]["name"]
            .as_str()
            .unwrap()
            .starts_with("guest_")
    );
    let set_cookie = headers[header::SET_COOKIE].to_str().expect("a cookie");
    assert!(set_cookie.contains("Max-Age=172800"), "{set_cookie}");
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Strict"));
    let cookie = set_cookie.split(';').next().unwrap().to_string();

    let (status, _, me) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["kind"], json!("trial"));
    assert!(me["trial_expires_at"].is_string());
    assert_eq!(me["id"], trial["user"]["id"]);

    let (status, headers, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::POST,
        "/api/accounts/trial",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "already in: nothing made");
    assert!(headers.get(header::SET_COOKIE).is_none());

    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::POST,
        "/api/files/uploads",
        Some(json!({ "content_type": "text/plain", "size": 5, "sha256": "0".repeat(64) })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no files on a trial"
    );
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::POST,
        "/api/users/current/avatar/uploads",
        Some(json!({ "content_type": "image/png", "size": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "and no picture");

    let (status, _, sent) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::POST,
        "/api/accounts/sign-in",
        Some(json!({ "email": "dana@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let code = code_sent_to(&router.mailer, "dana@example.com");
    let (status, _, matched) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::POST,
        "/api/accounts/code",
        Some(json!({ "pending": sent["pending"], "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(matched["new_account"], json!(true));
    let (status, headers, session) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/sign-in/confirm",
        Some(json!({ "pending": matched["pending"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["user"]["id"], trial["user"]["id"], "the same user");
    let set_cookie = headers[header::SET_COOKIE].to_str().expect("a cookie");
    assert!(!set_cookie.contains("Max-Age"), "as the checkbox said");
    let signed_up = set_cookie.split(';').next().unwrap().to_string();
    let (_, _, me) = send_with(
        &router.router,
        None,
        Some(&signed_up),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(me["kind"], json!("full"));
    assert_eq!(me["trial_expires_at"], Value::Null);
    assert_eq!(me["email"], json!("dana@example.com"));
    assert_eq!(me["name"], json!("dana"));
    let (status, _, _) = send_with(
        &router.router,
        None,
        Some(&cookie),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the trial's session is gone"
    );

    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["kind"], json!("full"), "anyone who signed up");
}

#[tokio::test]
async fn accounts_sign_in_makes_no_account_and_trials_are_refused_when_the_settings_turn_sign_ups_off()
 {
    let closed = ServerSettings {
        features: app_core::app::features::Features {
            accounts_sign_up: false,
            ..app_core::app::features::Features::default()
        },
    };
    let (router, _) = test_router_with(TrialUsersMax(2), false, closed).await;
    let ask = |method: Method, path: &'static str, body: Option<Value>| {
        let router = router.router.clone();
        async move { send_as(&router, None, method, path, body).await }
    };
    let bob = json!({ "email": "bob@example.com" });
    let (status, body) = ask(Method::POST, "/api/accounts/sign-in", Some(bob)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "answered as any sign-in");
    assert!(body["pending"].is_string());
    let mail = router
        .mailer
        .last_sent_to("bob@example.com")
        .expect("mailed");
    assert!(mail.subject.starts_with("No "), "{}", mail.subject);
    let (status, _) = ask(Method::POST, "/api/accounts/trial", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, available) = ask(Method::GET, "/api/accounts/trial", None).await;
    assert_eq!(available, json!({ "available": false }));
    let body = code_matched(&router, "alice@example.com").await;
    assert_eq!(
        body["new_account"],
        json!(false),
        "an account signs in all the same"
    );
}

#[tokio::test]
async fn accounts_sign_in_codes_are_refused_and_a_password_signs_in_when_the_settings_turn_codes_off()
 {
    let password_only = ServerSettings {
        features: app_core::app::features::Features {
            accounts_sign_up: false,
            accounts_sign_in_code: false,
            ..app_core::app::features::Features::default()
        },
    };
    let (router, _) = test_router_with(TrialUsersMax(2), false, password_only).await;
    app_core::accounts::users::create(&router.deployment, "dana@example.com", PASSWORD)
        .await
        .expect("created");
    let ask = |path: &'static str, body: Value| {
        let router = router.router.clone();
        async move { send_as(&router, None, Method::POST, path, Some(body)).await }
    };
    let refused = json!({ "error": "This server signs in with a password only" });
    for (path, body) in [
        (
            "/api/accounts/sign-in",
            json!({ "email": "dana@example.com" }),
        ),
        (
            "/api/accounts/code",
            json!({ "pending": "pending", "code": "000000" }),
        ),
        ("/api/accounts/code/resend", json!({ "pending": "pending" })),
    ] {
        let (status, body) = ask(path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(body, refused, "{path}");
    }
    assert!(
        router.mailer.last_sent_to("dana@example.com").is_none(),
        "nothing mailed"
    );

    let (status, body) = ask(
        "/api/accounts/sign-in/password",
        json!({ "email": "dana@example.com", "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], json!("dana@example.com"));

    for _ in 0..rate_limits::PASSWORD_TRIES_PER_ADDRESS {
        ask(
            "/api/accounts/sign-in/password",
            json!({ "email": "eli@example.com", "password": PASSWORD }),
        )
        .await;
    }
    let (status, body) = ask(
        "/api/accounts/sign-in/password",
        json!({ "email": "eli@example.com", "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body,
        json!({ "error": "Too many tries. Try your password again in a few minutes." })
    );
}

#[tokio::test]
async fn accounts_trial_is_refused_at_the_cap() {
    let (router, _) = test_router().await;
    for _ in 0..2 {
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/trial",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/trial",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body,
        json!({ "error": "Trials are full right now; sign up instead" })
    );
    let (_, available) = send_as(
        &router.router,
        None,
        Method::GET,
        "/api/accounts/trial",
        None,
    )
    .await;
    assert_eq!(available, json!({ "available": false }));
}

/// "Try it now" through a reverse proxy that wrote the header.
async fn trial_create_forwarded_for(router: &TestRouter, forwarded_for: &str) -> StatusCode {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/accounts/trial")
        .header("x-forwarded-for", forwarded_for)
        .body(Body::empty())
        .expect("a request");
    let response = router
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("a response");
    response.status()
}

#[tokio::test]
async fn accounts_trial_is_limited_per_client_by_the_address_the_proxy_wrote() {
    let (router, _) = test_router_with(TrialUsersMax(100), true, ServerSettings::default()).await;
    for _ in 0..rate_limits::TRIAL_USERS_PER_HOUR_PER_IP {
        let status = trial_create_forwarded_for(&router, "203.0.113.9").await;
        assert_eq!(status, StatusCode::CREATED);
    }
    assert_eq!(
        trial_create_forwarded_for(&router, "203.0.113.9").await,
        StatusCode::TOO_MANY_REQUESTS,
        "the eleventh in the hour"
    );
    assert_eq!(
        trial_create_forwarded_for(&router, "198.51.100.7, 203.0.113.9").await,
        StatusCode::TOO_MANY_REQUESTS,
        "an address the client wrote in front changes nothing"
    );
    assert_eq!(
        trial_create_forwarded_for(&router, "203.0.113.9, 203.0.113.10").await,
        StatusCode::CREATED,
        "another client"
    );
}

#[tokio::test]
async fn trial_users_are_refused_past_their_writes_and_see_only_their_communities() {
    let (router, _) = test_router().await;
    let trial = app_core::accounts::trial_users::create(&router.deployment, 2)
        .await
        .expect("a trial");
    let (status, communities) = send_as(
        &router.router,
        Some(&trial.token),
        Method::GET,
        "/api/communities",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&Value> = communities
        .as_array()
        .expect("a list")
        .iter()
        .map(|listed| &listed["community"]["name"])
        .collect();
    assert_eq!(names, [&json!("Guest")], "not Alice's Personal");

    for number in 2..app_core::accounts::trial_users::MAX_WRITES {
        app_core::app::posts::create(
            &router.deployment,
            &trial.user,
            &format!("post {number}"),
            &[],
        )
        .await
        .expect("within the maximum");
    }
    let (status, body) = send_as(
        &router.router,
        Some(&trial.token),
        Method::POST,
        "/api/posts",
        Some(json!({ "body": "one more", "community_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body,
        json!({ "error": "This trial has reached its limit. Sign up to keep going." })
    );
}

#[tokio::test]
async fn accounts_routes_are_rate_limited_per_address_per_sign_in_and_per_client() {
    let (router, _) = test_router().await;
    let alice = json!({ "email": "alice@example.com" });
    let mut pending = Value::Null;
    for _ in 0..rate_limits::EMAILS_PER_HOUR_PER_ADDRESS {
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(alice.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        pending = body["pending"].clone();
    }
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in",
        Some(alice),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the sixth email in an hour"
    );
    assert_eq!(
        body,
        json!({ "error": "Too many attempts; try again in a few minutes" })
    );

    let resend = json!({ "pending": pending });
    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code/resend",
        Some(resend.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "counted as an email to alice too"
    );
    let (router, _) = test_router().await;
    let (_, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in",
        Some(json!({ "email": "alice@example.com" })),
    )
    .await;
    let resend = json!({ "pending": body["pending"] });
    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code/resend",
        Some(resend.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/code/resend",
        Some(resend),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "one a minute per sign-in"
    );

    let (router, _) = test_router().await;
    let garbage = json!({ "code": "nope", "verifier": "nope" });
    for _ in 0..rate_limits::ACCOUNT_REQUESTS_PER_MINUTE_PER_IP {
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/device-token",
            Some(garbage.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/device-token",
        Some(garbage),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the thirty-first in a minute"
    );
    assert!(headers.get(header::RETRY_AFTER).is_some());
    assert_eq!(
        body,
        json!({ "error": "Too many attempts; try again in a few minutes" })
    );
    let (status, _) = send_as(
        &router.router,
        None,
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "other routes are not limited"
    );
}

#[tokio::test]
async fn accounts_a_device_signs_in_through_the_browser_and_exchanges_its_code() {
    let (router, _) = test_router().await;
    let (verifier, challenge) = app_core::accounts::verifier_and_challenge();
    let body = code_matched(&router, "alice@example.com").await;
    let (status, headers, body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/sign-in/confirm",
        Some(json!({ "pending": body["pending"], "challenge": challenge })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers.get(header::SET_COOKIE).is_none(),
        "no cookie for a device sign-in"
    );
    let device_code = body["code"].as_str().expect("the device code");
    assert!(body.get("token").is_none());

    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/device-token",
        Some(json!({ "code": device_code, "verifier": "not the verifier" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, headers, session) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/device-token",
        Some(json!({ "code": device_code, "verifier": verifier })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::SET_COOKIE).is_none());
    assert_eq!(session["email"], json!("alice@example.com"));
    let token = session["token"].as_str().unwrap();
    let (status, me) = send_as(
        &router.router,
        Some(token),
        Method::GET,
        "/api/users/current",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], json!("alice@example.com"));
    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/device-token",
        Some(json!({ "code": device_code, "verifier": verifier })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a device code works once");
}

#[tokio::test]
async fn accounts_deletions_delete_with_the_mailed_code_then_the_token_is_gone() {
    let (router, _) = test_router().await;
    let token = Some(router.token.as_str());
    let (status, _headers, body) = send_with(
        &router.router,
        token,
        None,
        Method::POST,
        "/api/accounts/deletions",
        Some(json!({ "code": "not a code" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let (status, _headers, _body) = send_with(
        &router.router,
        token,
        None,
        Method::POST,
        "/api/accounts/deletions/code",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let code = code_sent_to(&router.mailer, "alice@example.com");
    let (status, _headers, _body) = send_with(
        &router.router,
        token,
        None,
        Method::POST,
        "/api/accounts/deletions",
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _headers, body) =
        send_with(&router.router, token, None, Method::GET, "/api/posts", None).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["code"], json!("account_deleted"));
}

#[tokio::test]
async fn app_store_routes_are_not_found_without_the_app_store() {
    let (router, _) = test_router().await;
    let (status, _headers, _body) = send_with(
        &router.router,
        Some(&router.token),
        None,
        Method::POST,
        "/api/accounts/store-transactions",
        Some(json!({ "store": "app_store", "signed_transaction": "a.b.c" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _headers, _body) = send_with(
        &router.router,
        None,
        None,
        Method::POST,
        "/api/accounts/webhooks/app-store",
        Some(json!({ "signedPayload": "a.b.c" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A webhook delivery: the body as Stripe sends it, and its signature
/// header when there is one.
async fn stripe_webhook_send(
    router: &Router,
    body: &str,
    signature: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/accounts/webhooks/stripe")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(signature) = signature {
        request = request.header("stripe-signature", signature);
    }
    let request = request
        .body(Body::from(body.to_string()))
        .expect("a request");
    let response = router.clone().oneshot(request).await.expect("a response");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("a body")
        .to_bytes();
    (status, serde_json::from_slice(&body).expect("json"))
}

fn stripe_event_body(id: &str) -> String {
    json!({
        "id": id,
        "object": "event",
        "api_version": "2026-09-30.endive",
        "created": 1_790_000_000,
        "data": { "object": { "id": "obj_test", "object": "test_object" } },
        "livemode": false,
        "pending_webhooks": 1,
        "request": { "id": null, "idempotency_key": null },
        "type": "customer.created",
    })
    .to_string()
}

fn stripe_signature(body: &str, secret: &str) -> String {
    stripe_webhook::Webhook::generate_test_header(body, secret, None)
}

#[tokio::test]
async fn stripe_webhooks_create_records_a_signed_event_once() {
    let (router, _) = test_router().await;
    let body = stripe_event_body("evt_1");
    let signature = stripe_signature(&body, STRIPE_SIGNING_SECRET);

    let (status, answer) = stripe_webhook_send(&router.router, &body, Some(&signature)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer, json!({ "received": true, "repeat": false }));
    let (status, answer) = stripe_webhook_send(&router.router, &body, Some(&signature)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer, json!({ "received": true, "repeat": true }));
    let event = app_core::accounts::stripe_events::get(&router.deployment, "evt_1")
        .await
        .expect("read");
    assert!(event.is_some());
}

#[tokio::test]
async fn stripe_webhooks_create_refuses_what_stripe_did_not_sign() {
    let (router, _) = test_router().await;
    let body = stripe_event_body("evt_1");
    let forged = stripe_signature(&body, "whsec_another_secret");
    for signature in [Some(forged.as_str()), None] {
        let (status, answer) = stripe_webhook_send(&router.router, &body, signature).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(answer["error"].is_string());
    }
    let event = app_core::accounts::stripe_events::get(&router.deployment, "evt_1")
        .await
        .expect("read");
    assert_eq!(event, None);
}

#[tokio::test]
async fn stripe_webhooks_create_is_not_found_without_a_secret() {
    let (router, _) = test_router().await;
    let state = AppState {
        deployment: router.deployment.clone(),
        mailer: router.mailer.clone(),
        rate_limits: RateLimits::new(),
        trust_proxy_headers: false,
        trial_users_max: TrialUsersMax(0),
        stripe_signing_secret: StripeSigningSecret(None),
        stripe: Some(router.stripe.clone()),
        server_url: ServerUrl("http://server.test".to_string()),
        server_description: test_server_description(&ServerSettings::default()),
        server_settings: std::sync::Arc::new(SettingsDocument::of(ServerSettings::default())),
        listed_servers: std::sync::Arc::new(test_listed_servers()),
        server_state: std::sync::Arc::default(),
    };
    let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
    let without_stripe = super::router(&dist, &dist.join("ops"), state);
    let body = stripe_event_body("evt_1");
    let signature = stripe_signature(&body, STRIPE_SIGNING_SECRET);
    let (status, _) = stripe_webhook_send(&without_stripe, &body, Some(&signature)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn accounts_purchasing_routes_are_refused_when_the_settings_turn_purchasing_off() {
    let selling_nothing = ServerSettings {
        features: app_core::app::features::Features {
            accounts_purchasing: false,
            ..app_core::app::features::Features::default()
        },
    };
    let (router, _) = test_router_with(TrialUsersMax(2), false, selling_nothing).await;
    let refused = json!({ "error": "This server sells nothing" });
    for path in [
        "/api/accounts/products",
        "/api/accounts/purchases",
        "/api/accounts/subscriptions",
    ] {
        let (status, body) = get(&router, path).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(body, refused, "{path}");
    }
    let (status, body) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/webhooks/stripe",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, refused);
    let (status, _) = get(&router, "/api/accounts/current/preferences").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the rest of the account all the same"
    );
}

#[tokio::test]
async fn accounts_products_list_and_get_show_the_catalog() {
    let (router, _) = test_router().await;
    router
        .stripe
        .memory_product_put(app_core::services::stripe::StripeProduct {
            id: "prod_license".to_string(),
            name: "License".to_string(),
            description: None,
            active: true,
        });
    router
        .stripe
        .memory_price_put(app_core::services::stripe::StripePrice {
            id: "price_license".to_string(),
            product_id: "prod_license".to_string(),
            lookup_key: Some("license".to_string()),
            amount: Some(4900),
            currency: "usd".to_string(),
            interval: None,
            active: true,
        });
    app_core::accounts::products::sync(&router.deployment, &router.stripe)
        .await
        .expect("synced");

    let (status, list) = get(&router, "/api/accounts/products").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["name"], json!("License"));
    assert_eq!(list[0]["kind"], json!("one_time"));
    assert_eq!(list[0]["prices"][0]["amount"], json!(4900));
    let id = list[0]["id"].as_str().expect("an id");
    let (status, shown) = get(&router, &format!("/api/accounts/products/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shown["name"], list[0]["name"]);
    assert_eq!(shown["owned"], json!(false));
    let (status, _) = get(&router, &format!("/api/accounts/products/{MISSING_ID}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send_as(
        &router.router,
        None,
        Method::GET,
        "/api/accounts/products",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accounts_checkouts_create_then_get_records_the_purchase() {
    use app_core::services::stripe::{
        StripeCheckoutSession, StripeCheckoutSessionStatus, StripePaymentStatus, StripePrice,
        StripeProduct,
    };
    let (router, _) = test_router().await;
    router.stripe.memory_product_put(StripeProduct {
        id: "prod_license".to_string(),
        name: "License".to_string(),
        description: None,
        active: true,
    });
    router.stripe.memory_price_put(StripePrice {
        id: "price_license".to_string(),
        product_id: "prod_license".to_string(),
        lookup_key: Some("license".to_string()),
        amount: Some(4900),
        currency: "usd".to_string(),
        interval: None,
        active: true,
    });
    app_core::accounts::products::sync(&router.deployment, &router.stripe)
        .await
        .expect("synced");
    let (_, list) = get(&router, "/api/accounts/products").await;
    let price_id = list[0]["prices"][0]["id"].clone();

    let (status, created) = send(
        &router,
        Method::POST,
        "/api/accounts/checkouts",
        Some(json!({ "price_id": price_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        created["url"]
            .as_str()
            .expect("a url")
            .starts_with("https://")
    );
    let id = created["checkout"]["id"].as_str().expect("an id");
    let session_id = created["checkout"]["stripe_session_id"]
        .as_str()
        .expect("a session");

    router
        .stripe
        .memory_checkout_session_put(StripeCheckoutSession {
            id: session_id.to_string(),
            url: None,
            client_reference_id: Some(id.to_string()),
            status: StripeCheckoutSessionStatus::Complete,
            payment_status: StripePaymentStatus::Paid,
            payment_intent_id: Some("pi_1".to_string()),
            subscription_id: None,
            amount_total: Some(4900),
            currency: Some("usd".to_string()),
        });
    let (status, shown) = get(&router, &format!("/api/accounts/checkouts/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shown["status"], json!("paid"));
    assert_eq!(shown["product_name"], json!("License"));
    let purchase_id = shown["purchase_id"].as_str().expect("a purchase");

    let (status, purchases) = get(&router, "/api/accounts/purchases").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purchases[0]["id"], json!(purchase_id));
    assert_eq!(purchases[0]["product_name"], json!("License"));
    let (status, purchase) = get(&router, &format!("/api/accounts/purchases/{purchase_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purchase["store"], json!("stripe"));

    let (status, again) = send(
        &router,
        Method::POST,
        "/api/accounts/checkouts",
        Some(json!({ "price_id": price_id })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(again["error"], json!("You have this already"));
}

#[tokio::test]
async fn accounts_subscriptions_and_the_plan_on_the_current_user() {
    use app_core::services::stripe::{
        StripeCheckoutSession, StripeCheckoutSessionStatus, StripePaymentStatus, StripePrice,
        StripeProduct, StripeSubscription,
    };
    let (router, _) = test_router().await;
    router.stripe.memory_product_put(StripeProduct {
        id: "prod_single".to_string(),
        name: "Single user subscription".to_string(),
        description: None,
        active: true,
    });
    router.stripe.memory_price_put(StripePrice {
        id: "price_monthly".to_string(),
        product_id: "prod_single".to_string(),
        lookup_key: Some("single_user_monthly".to_string()),
        amount: Some(900),
        currency: "usd".to_string(),
        interval: Some("month".to_string()),
        active: true,
    });
    app_core::accounts::products::sync(&router.deployment, &router.stripe)
        .await
        .expect("synced");
    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["plan"], Value::Null);

    let (_, list) = get(&router, "/api/accounts/products").await;
    let price_id = list[0]["prices"][0]["id"].clone();
    let (_, created) = send(
        &router,
        Method::POST,
        "/api/accounts/checkouts",
        Some(json!({ "price_id": price_id })),
    )
    .await;
    let id = created["checkout"]["id"].as_str().expect("an id");
    let session_id = created["checkout"]["stripe_session_id"]
        .as_str()
        .expect("a session");
    let customer_id = match &router.stripe {
        Stripe::Memory(memory) => memory.lock().expect("the memory").checkout_sessions_asked[0]
            .customer_id
            .clone(),
        Stripe::Api(_) => unreachable!("tests use memory"),
    };
    let subscription = |status: &str| StripeSubscription {
        id: "sub_1".to_string(),
        customer_id: customer_id.clone(),
        item_id: "si_1".to_string(),
        price_id: "price_monthly".to_string(),
        status: status.to_string(),
        current_period_end: Some(1_800_000_000),
        cancel_at_period_end: false,
        ended_at: None,
        schedule_id: None,
        scheduled_price_id: None,
        scheduled_at: None,
    };
    router
        .stripe
        .memory_subscription_put(subscription("active"));
    router
        .stripe
        .memory_checkout_session_put(StripeCheckoutSession {
            id: session_id.to_string(),
            url: None,
            client_reference_id: Some(id.to_string()),
            status: StripeCheckoutSessionStatus::Complete,
            payment_status: StripePaymentStatus::Paid,
            payment_intent_id: None,
            subscription_id: Some("sub_1".to_string()),
            amount_total: Some(900),
            currency: Some("usd".to_string()),
        });
    let (_, shown) = get(&router, &format!("/api/accounts/checkouts/{id}")).await;
    assert_eq!(shown["status"], json!("paid"));
    assert_eq!(shown["kind"], json!("plan"));

    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["plan"]["grant"], json!("single_user_subscription"));
    let subscription_id = me["plan"]["subscription_id"].clone();
    let (status, subscriptions) = get(&router, "/api/accounts/subscriptions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(subscriptions[0]["id"], subscription_id);
    assert_eq!(subscriptions[0]["status"], json!("active"));
    assert_eq!(subscriptions[0]["price"]["interval"], json!("month"));
    let subscription_path = format!(
        "/api/accounts/subscriptions/{}",
        subscription_id.as_str().expect("an id")
    );
    let (status, _) = get(&router, &subscription_path).await;
    assert_eq!(status, StatusCode::OK);

    let (status, canceled) = send(
        &router,
        Method::PUT,
        &subscription_path,
        Some(json!({ "action": "cancel" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canceled["cancel_at_period_end"], json!(true));
    let (status, resumed) = send(
        &router,
        Method::PUT,
        &subscription_path,
        Some(json!({ "action": "resume" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resumed["cancel_at_period_end"], json!(false));
    let (status, portal) = send(
        &router,
        Method::POST,
        &format!("{subscription_path}/portal"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        portal["url"]
            .as_str()
            .expect("a url")
            .starts_with("https://")
    );
    let (status, portal) = send(
        &router,
        Method::POST,
        &format!("{subscription_path}/portal"),
        Some(json!({ "return_to": "app" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let url = portal["url"].as_str().expect("a url");
    assert!(
        url.ends_with("http://server.test/accounts/back-to-app"),
        "{url}"
    );

    router
        .stripe
        .memory_subscription_put(subscription("past_due"));
    let body = json!({
        "id": "evt_1",
        "object": "event",
        "created": 1_790_000_000,
        "data": { "object": { "id": "sub_1", "object": "subscription" } },
        "livemode": false,
        "pending_webhooks": 1,
        "type": "customer.subscription.updated",
    })
    .to_string();
    let signature = stripe_signature(&body, STRIPE_SIGNING_SECRET);
    let (status, _) = stripe_webhook_send(&router.router, &body, Some(&signature)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, me) = get(&router, "/api/users/current").await;
    assert_eq!(me["plan"], Value::Null);
    assert_eq!(me["payment_failed_subscription_id"], subscription_id);
}

#[tokio::test]
async fn accounts_mails_go_in_the_persons_language_else_the_requests_else_english() {
    let (router, _) = test_router().await;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/accounts/sign-in")
        .header(header::ACCEPT_LANGUAGE, "es-MX,es;q=0.9,en;q=0.8")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "email": "bob@example.com" }).to_string(),
        ))
        .expect("a request");
    let response = router
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("a response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mail = router
        .mailer
        .last_sent_to("bob@example.com")
        .expect("mailed");
    assert_eq!(
        mail.subject, "Tu código de Starter Pack",
        "the request's language"
    );

    let (status, _) = send_as(
        &router.router,
        None,
        Method::POST,
        "/api/accounts/sign-in",
        Some(json!({ "email": "carol@example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let mail = router
        .mailer
        .last_sent_to("carol@example.com")
        .expect("mailed");
    assert_eq!(
        mail.subject, "Your Starter Pack code",
        "English when the request names none"
    );

    let (status, _) = send(
        &router,
        Method::PUT,
        "/api/accounts/current/preferences",
        Some(json!({ "key": "locale", "value": "es" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&router, Method::POST, "/api/accounts/deletions/code", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let mail = router
        .mailer
        .last_sent_to("alice@example.com")
        .expect("mailed");
    assert!(
        mail.subject.starts_with("Tu código"),
        "her choice: {}",
        mail.subject
    );
}

#[tokio::test]
async fn accounts_current_user_from_bearer_token_takes_the_header_and_never_the_cookie() {
    use crate::user_auth::CurrentUserFromBearerToken;
    let (router, _) = test_router().await;
    let probe = Router::new()
        .route(
            "/probe",
            axum::routing::get(
                |CurrentUserFromBearerToken(user): CurrentUserFromBearerToken| async move {
                    axum::Json(json!({ "id": user.id }))
                },
            ),
        )
        .with_state(router.deployment.clone());
    let ask = |token: Option<&str>, cookie: Option<&str>| {
        let probe = probe.clone();
        let token = token.map(str::to_string);
        let cookie = cookie.map(str::to_string);
        async move {
            let (status, _, body) = send_with(
                &probe,
                token.as_deref(),
                cookie.as_deref(),
                Method::GET,
                "/probe",
                None,
            )
            .await;
            (status, body)
        }
    };
    let (status, body) = ask(Some(&router.token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["id"].is_string());
    let cookie = format!("token={}", router.token);
    let (status, body) = ask(None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the cookie is not read");
    assert_eq!(body, json!({ "error": "Sign in first" }));
    let (status, _) = ask(Some("not-a-token"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accounts_current_user_from_cookie_takes_the_cookie_and_never_the_header() {
    use crate::user_auth::CurrentUserFromCookie;
    let (router, _) = test_router().await;
    let probe = Router::new()
        .route(
            "/probe",
            axum::routing::get(
                |CurrentUserFromCookie(user): CurrentUserFromCookie| async move {
                    axum::Json(json!({ "id": user.id }))
                },
            ),
        )
        .with_state(router.deployment.clone());
    let ask = |token: Option<&str>, cookie: Option<&str>| {
        let probe = probe.clone();
        let token = token.map(str::to_string);
        let cookie = cookie.map(str::to_string);
        async move {
            let (status, _, body) = send_with(
                &probe,
                token.as_deref(),
                cookie.as_deref(),
                Method::GET,
                "/probe",
                None,
            )
            .await;
            (status, body)
        }
    };
    let cookie = format!("token={}", router.token);
    let (status, body) = ask(None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["id"].is_string());
    let (status, body) = ask(Some(&router.token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the header is not read");
    assert_eq!(body, json!({ "error": "Sign in first" }));
    let (status, _) = ask(None, Some("token=not-a-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
