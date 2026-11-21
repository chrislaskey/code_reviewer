//! Every route: the JSON API under `/api`, with the ops app's under
//! `/api/ops`, see `ops`, the public website's pages, see `public`, the
//! ops app's pages, then the Svelte pages. A path none of them and the built files
//! don't know gets `index.html`, so the pages'
//! router (`svelte/src/lib/router.js`) shows the page at that path, and
//! every page is reloadable and linkable.

use std::path::Path;

use app_core::accounts::stripe::Stripe;
use app_core::deployment::Deployment;
use app_core::mailer::Mailer;
use axum::Router;
use axum::extract::{DefaultBodyLimit, FromRef};
use axum::http::{Method, StatusCode, Uri};
use axum::routing::{any, delete, get, post, put};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api_error::ApiError;
use crate::checkouts::ServerUrl;
use crate::rate_limits::{self, RateLimits};
use crate::stripe_events::StripeSigningSecret;
use crate::{
    accounts, app_store_notifications, avatars, checkouts, communities, current_community,
    current_user, deletions, files, memberships, ops, posts, products, public, purchases,
    store_transactions, stripe_events, subscriptions, sync,
};

/// What every handler may take from the server: the database with its
/// buckets, where its email goes, and the rate limits; and whether the
/// client IP is the proxy's header, see `rate_limits`.
#[derive(Clone)]
pub struct AppState {
    pub deployment: Deployment,
    pub mailer: Mailer,
    pub rate_limits: RateLimits,
    pub trust_proxy_headers: bool,
    pub trial_users_max: TrialUsersMax,
    pub stripe_signing_secret: StripeSigningSecret,
    pub stripe: Option<Stripe>,
    pub server_url: ServerUrl,
}

/// How many trial users the server holds at once, `TRIAL_USERS_MAX`; 0,
/// the default, turns trials off, see `app_core::trial_users`.
#[derive(Clone, Copy)]
pub struct TrialUsersMax(pub i64);

impl FromRef<AppState> for TrialUsersMax {
    fn from_ref(state: &AppState) -> TrialUsersMax {
        state.trial_users_max
    }
}

impl FromRef<AppState> for Deployment {
    fn from_ref(state: &AppState) -> Deployment {
        state.deployment.clone()
    }
}

impl FromRef<AppState> for Mailer {
    fn from_ref(state: &AppState) -> Mailer {
        state.mailer.clone()
    }
}

impl FromRef<AppState> for RateLimits {
    fn from_ref(state: &AppState) -> RateLimits {
        state.rate_limits.clone()
    }
}

pub fn router(svelte_dist: &Path, ops_dist: &Path, state: AppState) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));
    let accounts = Router::new()
        .route("/api/accounts/sign-up", post(accounts::sign_up))
        .route("/api/accounts/sign-in", post(accounts::sign_in))
        .route("/api/accounts/code", post(accounts::code))
        .route("/api/accounts/code/resend", post(accounts::code_resend))
        .route(
            "/api/accounts/sign-in/password",
            post(accounts::sign_in_password),
        )
        .route(
            "/api/accounts/sign-in/new-password",
            post(accounts::sign_in_new_password),
        )
        .route(
            "/api/accounts/sign-in/reset-password",
            post(accounts::sign_in_reset_password),
        )
        .route(
            "/api/accounts/second-factors",
            get(accounts::second_factors),
        )
        .route("/api/accounts/password", put(accounts::password_update))
        .route("/api/accounts/device-token", post(accounts::device_token))
        .route("/api/accounts/sign-out", delete(accounts::sign_out))
        .route("/api/accounts/deletions/code", post(deletions::code_create))
        .route("/api/accounts/deletions", post(deletions::create))
        .route(
            "/api/accounts/trial",
            get(accounts::trial_get).post(accounts::trial_create),
        )
        .layer(rate_limits::per_ip_layer(state.trust_proxy_headers));

    Router::new()
        .merge(accounts)
        .merge(ops::api(state.clone()))
        .route(
            "/api/current-user",
            get(current_user::get).put(current_user::update),
        )
        .route(
            "/api/current-community",
            get(current_community::get).put(current_community::update),
        )
        .route("/api/accounts/stripe-webhooks", post(stripe_events::create))
        .route(
            "/api/accounts/app-store-notifications",
            post(app_store_notifications::create),
        )
        .route(
            "/api/accounts/store-transactions",
            post(store_transactions::create),
        )
        .route("/api/accounts/products", get(products::list))
        .route("/api/accounts/products/{id}", get(products::get))
        .route("/api/accounts/checkouts", post(checkouts::create))
        .route("/api/accounts/checkouts/{id}", get(checkouts::get))
        .route("/api/accounts/purchases", get(purchases::list))
        .route("/api/accounts/purchases/{id}", get(purchases::get))
        .route("/api/accounts/subscriptions", get(subscriptions::list))
        .route(
            "/api/accounts/subscriptions/{id}",
            get(subscriptions::get).put(subscriptions::update),
        )
        .route(
            "/api/accounts/subscriptions/{id}/portal",
            post(subscriptions::portal_create),
        )
        .route("/api/avatars", put(avatars::update))
        .route("/api/avatars/uploads", post(avatars::create_upload))
        .route("/api/posts", get(posts::list).post(posts::create))
        .route(
            "/api/posts/{id}",
            get(posts::get).put(posts::update).delete(posts::delete),
        )
        .route("/api/files", post(files::create))
        .route("/api/files/search", get(files::search))
        .route("/api/files/upload", post(files::create_upload))
        .route("/api/files/duplicates", post(files::list_duplicates))
        .route(
            "/api/files/{id}",
            get(files::get).put(files::update).delete(files::delete),
        )
        .route("/api/files/{id}/content", get(files::content))
        .route(
            "/api/communities",
            get(communities::list).post(communities::create),
        )
        .route(
            "/api/communities/{id}",
            get(communities::get)
                .put(communities::update)
                .delete(communities::delete),
        )
        .route(
            "/api/memberships/{community_id}",
            put(memberships::join).delete(memberships::leave),
        )
        .route(
            "/api/sync/push",
            post(sync::push).layer(DefaultBodyLimit::max(app_core::sync::MAX_PUSH_BYTES)),
        )
        .route("/api/sync/pull", post(sync::pull))
        .route("/api/sync/backfill", post(sync::backfill))
        .route("/api", any(api_not_found))
        .route("/api/{*path}", any(api_not_found))
        .with_state(state.clone())
        .merge(public::router(svelte_dist))
        .merge(ops::pages(ops_dist, state.clone()))
        .fallback_service(pages)
        .layer(TraceLayer::new_for_http())
}

/// An API path no route has: a JSON 404, so a page asking a server that
/// lacks the route sees an error and not the pages' HTML.
pub(crate) async fn api_not_found(method: Method, uri: Uri) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        format!("The server has no API route {method} {}", uri.path()),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use app_core::accounts::{Session, SignInStep};
    use app_core::storage::Storage;

    use super::*;

    const MISSING_ID: &str = "01900000-0000-7000-8000-000000000000";
    const PASSWORD: &str = "correct horse battery";
    const NEW_PASSWORD: &str = "another horse battery";
    const STRIPE_SIGNING_SECRET: &str = "whsec_test_secret";

    /// A router over an in-memory database with Alice signed up and one
    /// post of hers, and a stand-in `index.html`. Every request `send`
    /// makes is hers, with her token. The emails go to `mailer`, where
    /// a test reads the code, and uploads to `storage`, in memory, where
    /// a test puts the file a page would have.
    struct TestRouter {
        router: Router,
        deployment: Deployment,
        token: String,
        mailer: Mailer,
        storage: Storage,
        stripe: Stripe,
    }

    async fn test_router() -> (TestRouter, Value) {
        test_router_with(TrialUsersMax(2), false).await
    }

    /// `test_router` holding so many trial users, and behind a reverse
    /// proxy or not.
    async fn test_router_with(
        trial_users_max: TrialUsersMax,
        trust_proxy_headers: bool,
    ) -> (TestRouter, Value) {
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        fs::create_dir_all(&dist).expect("a temp dir");
        fs::write(dist.join("index.html"), "<html>pages</html>").expect("index.html");
        let ops_dist = dist.join("ops");
        fs::create_dir_all(ops_dist.join("assets")).expect("an ops dir");
        fs::write(ops_dist.join("ops.html"), "<html>ops</html>").expect("ops.html");
        fs::write(ops_dist.join("assets/ops.js"), "ops script").expect("ops.js");
        fs::create_dir_all(dist.join("public")).expect("a public dir");
        fs::write(dist.join("public/index.html"), "<html>public home</html>").expect("the home");
        fs::write(dist.join("public/about.html"), "<html>public about</html>").expect("about");
        fs::write(dist.join("public/terms.html"), "<html>public terms</html>").expect("terms");
        fs::write(
            dist.join("public/privacy.html"),
            "<html>public privacy</html>",
        )
        .expect("privacy");
        let database = app_core::database::Database::open_sqlite_in_memory()
            .await
            .expect("a database");
        let storage = Storage::memory();
        let deployment = Deployment::new(database, app_core::deployment::Role::Server)
            .with_file_store(app_core::deployment::FileStore::Bucket(storage.clone()))
            .with_avatar_bucket(storage.clone());
        let mailer = Mailer::memory();
        let stripe = Stripe::memory();
        let alice = signed_up(&deployment, &mailer, "alice@example.com").await;
        let post = app_core::posts::create(&deployment, &alice.user, "the first post", &[])
            .await
            .expect("a post");
        let post = app_core::posts::get(&deployment, &alice.user, post.id)
            .await
            .expect("the post");
        let post = serde_json::to_value(post).expect("json");
        let state = AppState {
            deployment: deployment.clone(),
            mailer: mailer.clone(),
            rate_limits: RateLimits::new(),
            trust_proxy_headers,
            trial_users_max,
            stripe_signing_secret: StripeSigningSecret(Some(std::sync::Arc::new(
                secrecy::SecretString::from(STRIPE_SIGNING_SECRET),
            ))),
            stripe: Some(stripe.clone()),
            server_url: ServerUrl("http://server.test".to_string()),
        };
        let test_router = TestRouter {
            router: router(&dist, &ops_dist, state),
            deployment,
            token: alice.token,
            mailer,
            storage,
            stripe,
        };
        (test_router, post)
    }

    /// A user signed up on the server and finished with their code.
    async fn signed_up(deployment: &Deployment, mailer: &Mailer, email: &str) -> Session {
        let sent = app_core::accounts::sign_up(deployment, mailer, email)
            .await
            .expect("signed up");
        let code = code_sent_to(mailer, email);
        let step = app_core::accounts::sign_in_with_code(deployment, &sent.pending, &code)
            .await
            .expect("the code matched");
        let SignInStep::Next { pending, .. } = step else {
            panic!("a new account is offered a password");
        };
        app_core::accounts::sign_in_with_new_password(deployment, &pending, None)
            .await
            .expect("skipped making a password")
    }

    /// The code the last email to the address carried.
    fn code_sent_to(mailer: &Mailer, email: &str) -> String {
        let mail = mailer.last_sent_to(email).expect("an email");
        app_core::mails::code_in(&mail).expect("a code in it")
    }

    async fn send(
        router: &TestRouter,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_as(&router.router, Some(&router.token), method, path, body).await
    }

    async fn send_as(
        router: &Router,
        token: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, _headers, body) = send_with(router, token, None, method, path, body).await;
        (status, body)
    }

    /// A request with a bearer token, or a `Cookie` header, or neither,
    /// answered with its status, its headers and its body.
    async fn send_with(
        router: &Router,
        token: Option<&str>,
        cookie: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, header::HeaderMap, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("a request");
        let response = router.clone().oneshot(request).await.expect("a response");
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("a body")
            .to_bytes();
        let body = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into()))
        };
        (status, headers, body)
    }

    async fn get(router: &TestRouter, path: &str) -> (StatusCode, Value) {
        send(router, Method::GET, path, None).await
    }

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
        let (status, upload) =
            send(&router, Method::POST, "/api/avatars/uploads", Some(picked)).await;
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
    async fn accounts_sign_up_then_the_code_sign_in_and_sign_out_over_the_cookie() {
        let (router, _) = test_router().await;
        let bob = json!({ "email": "Bob@example.com" });
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
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
        assert_eq!(body["next"], json!("new_password"), "a first sign-in");
        assert!(headers.get(header::SET_COOKIE).is_none(), "not finished");
        let pending = body["pending"].as_str().expect("the pending token");
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in/new-password",
            Some(json!({ "pending": pending, "password": "short" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body,
            json!({ "error": "a password needs at least 12 characters" })
        );
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/sign-in/new-password",
            Some(json!({ "pending": pending, "password": PASSWORD })),
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
            "/api/current-user",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["email"], json!("bob@example.com"));
        let (status, _, factors) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::GET,
            "/api/accounts/second-factors",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(factors, json!(["password"]));
        let (status, _, body) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::PUT,
            "/api/accounts/password",
            Some(json!({ "current": "wrong horse battery", "password": NEW_PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, json!({ "error": "That password is wrong" }));
        let (status, _, _) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::PUT,
            "/api/accounts/password",
            Some(json!({ "current": PASSWORD, "password": NEW_PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
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
            "/api/current-user",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(bob),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(body["pending"].is_string());
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
        assert_eq!(body["next"], json!("password"), "bob has a password");
        assert!(body.get("token").is_none());
        let pending = body["pending"].clone();
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in/password",
            Some(json!({ "pending": pending, "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "the old one");
        assert_eq!(body, json!({ "error": "That password is wrong" }));
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/sign-in/reset-password",
            Some(json!({ "pending": pending, "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "forgotten, so reset");
        assert_eq!(body["email"], json!("bob@example.com"));
        assert!(headers.get(header::SET_COOKIE).is_some());
        assert_eq!(
            router
                .mailer
                .last_sent_to("bob@example.com")
                .unwrap()
                .subject,
            "Your password was changed"
        );
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "bob@example.com" })),
        )
        .await;
        let code = code_sent_to(&router.mailer, "bob@example.com");
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code })),
        )
        .await;
        assert_eq!(body["can_reset_password"], json!(true));
        let pending = body["pending"].clone();
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/sign-in/password",
            Some(json!({ "pending": pending, "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["email"], json!("bob@example.com"));
        assert!(headers.get(header::SET_COOKIE).is_some());

        let sent_before = router.mailer.sent().len();
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "bob@example.com" })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "a taken email is answered as a new one"
        );
        assert!(body["pending"].is_string());
        assert_eq!(router.mailer.sent().len(), sent_before + 1);
        assert_eq!(
            router
                .mailer
                .last_sent_to("bob@example.com")
                .unwrap()
                .subject,
            "You already have an account"
        );
    }

    /// The `Set-Cookie` of a sign-in step, when it set one.
    async fn set_cookie_of(router: &Router, path: &str, body: Value) -> Option<String> {
        let (status, headers, _) =
            send_with(router, None, None, Method::POST, path, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        headers
            .get(header::SET_COOKIE)
            .map(|cookie| cookie.to_str().expect("a cookie").to_string())
    }

    /// A sign-in of the email up to its password step: the pending token.
    async fn signed_in_to_the_password(router: &TestRouter, email: &str) -> Value {
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": email })),
        )
        .await;
        let code = code_sent_to(&router.mailer, email);
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code })),
        )
        .await;
        body["pending"].clone()
    }

    #[tokio::test]
    async fn accounts_keep_signed_in_makes_the_cookie_outlast_the_browser_on_every_step() {
        let (router, _) = test_router().await;
        let sixty_days = "Max-Age=5184000";

        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "alice@example.com" })),
        )
        .await;
        let code = code_sent_to(&router.mailer, "alice@example.com");
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/code",
            json!({ "pending": body["pending"], "code": code, "keep_signed_in": true }),
        )
        .await
        .expect("the code finished");
        assert!(cookie.contains(sixty_days), "{cookie}");
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));

        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "bob@example.com" })),
        )
        .await;
        let code = code_sent_to(&router.mailer, "bob@example.com");
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code, "keep_signed_in": true })),
        )
        .await;
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/sign-in/new-password",
            json!({ "pending": body["pending"], "password": PASSWORD, "keep_signed_in": true }),
        )
        .await
        .expect("a cookie");
        assert!(cookie.contains(sixty_days), "{cookie}");

        let pending = signed_in_to_the_password(&router, "bob@example.com").await;
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/sign-in/password",
            json!({ "pending": pending, "password": PASSWORD, "keep_signed_in": true }),
        )
        .await
        .expect("a cookie");
        assert!(cookie.contains(sixty_days), "{cookie}");

        let pending = signed_in_to_the_password(&router, "bob@example.com").await;
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/sign-in/reset-password",
            json!({ "pending": pending, "password": NEW_PASSWORD, "keep_signed_in": true }),
        )
        .await
        .expect("a cookie");
        assert!(cookie.contains(sixty_days), "{cookie}");

        let pending = signed_in_to_the_password(&router, "bob@example.com").await;
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/sign-in/password",
            json!({ "pending": pending, "password": NEW_PASSWORD }),
        )
        .await
        .expect("a cookie");
        assert!(!cookie.contains("Max-Age"), "until the browser closes");

        let (_verifier, challenge) = app_core::accounts::verifier_and_challenge();
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "alice@example.com" })),
        )
        .await;
        let code = code_sent_to(&router.mailer, "alice@example.com");
        let cookie = set_cookie_of(
            &router.router,
            "/api/accounts/code",
            json!({
                "pending": body["pending"],
                "code": code,
                "challenge": challenge,
                "keep_signed_in": true,
            }),
        )
        .await;
        assert!(cookie.is_none(), "a device's sign-in sets no cookie");
    }

    #[tokio::test]
    async fn accounts_trial_makes_a_user_whose_cookie_lasts_two_days_and_who_signs_up_as_themselves()
     {
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
                .starts_with("trial_")
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
            "/api/current-user",
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
            "/api/files/upload",
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
            "/api/avatars/uploads",
            Some(json!({ "content_type": "image/png", "size": 5 })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "and no picture");

        let (status, _, sent) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "dana@example.com" })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let code = code_sent_to(&router.mailer, "dana@example.com");
        let (status, _, step) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": sent["pending"], "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(step["next"], json!("new_password"));
        let (status, headers, session) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/sign-in/new-password",
            Some(json!({ "pending": step["pending"], "password": null })),
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
            "/api/current-user",
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
            "/api/current-user",
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "the trial's session is gone"
        );

        let (_, me) = get(&router, "/api/current-user").await;
        assert_eq!(me["kind"], json!("full"), "anyone who signed up");
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
        let (router, _) = test_router_with(TrialUsersMax(100), true).await;
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
        let trial = app_core::trial_users::create(&router.deployment, 2)
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
        assert_eq!(names, [&json!("Trial")], "not Alice's Personal");

        for number in 2..app_core::trial_users::MAX_WRITES {
            app_core::posts::create(
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
    async fn posts_create_refuses_a_body_over_the_maximum_length() {
        let (router, _) = test_router().await;
        let too_long = "a".repeat(app_core::posts::MAX_BODY_LENGTH + 1);
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
        let (status, _) =
            send_as(&router.router, None, Method::GET, "/api/current-user", None).await;
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
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "alice@example.com" })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let code = code_sent_to(&router.mailer, "alice@example.com");
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code, "challenge": challenge })),
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
            "/api/current-user",
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
        let sha256 = app_core::files::sha256_hex(&bytes);
        let picked = json!({ "content_type": "text/plain", "size": bytes.len(), "sha256": sha256 });
        let (status, upload) = send(&router, Method::POST, "/api/files/upload", Some(picked)).await;
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
        let other = app_core::files::sha256_hex(b"other");
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
        let longest = "a".repeat(app_core::posts::MAX_BODY_LENGTH);
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
    async fn pages_answer_any_path_with_index_html() {
        let (router, _) = test_router().await;
        for path in ["/home", "/posts/new"] {
            let (status, body) = get(&router, path).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, json!("<html>pages</html>"));
        }
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

    #[tokio::test]
    async fn ops_is_a_404_to_anyone_who_is_not_staff() {
        let (router, _) = test_router().await;
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
        let (router, _) = test_router().await;
        let staff = app_core::ops::staff::create(&router.deployment, "alice@example.com")
            .await
            .expect("alice is staff");
        let (status, body) = get(&router, "/api/ops/current-staff").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user_id"], json!(staff.user_id));
        let (_, me) = get(&router, "/api/current-user").await;
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
            "/api/accounts/app-store-notifications",
            Some(json!({ "signedPayload": "a.b.c" })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
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

    /// A webhook delivery: the body as Stripe sends it, and its signature
    /// header when there is one.
    async fn stripe_webhook_send(
        router: &Router,
        body: &str,
        signature: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/accounts/stripe-webhooks")
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
        };
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        let without_stripe = super::router(&dist, &dist.join("ops"), state);
        let body = stripe_event_body("evt_1");
        let signature = stripe_signature(&body, STRIPE_SIGNING_SECRET);
        let (status, _) = stripe_webhook_send(&without_stripe, &body, Some(&signature)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn accounts_products_list_and_get_show_the_catalog() {
        let (router, _) = test_router().await;
        router
            .stripe
            .memory_product_put(app_core::accounts::stripe::StripeProduct {
                id: "prod_license".to_string(),
                name: "License".to_string(),
                description: None,
                active: true,
            });
        router
            .stripe
            .memory_price_put(app_core::accounts::stripe::StripePrice {
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
        use app_core::accounts::stripe::{
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
        let (status, purchase) =
            get(&router, &format!("/api/accounts/purchases/{purchase_id}")).await;
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
        use app_core::accounts::stripe::{
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
        let (_, me) = get(&router, "/api/current-user").await;
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

        let (_, me) = get(&router, "/api/current-user").await;
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
        let (_, me) = get(&router, "/api/current-user").await;
        assert_eq!(me["plan"], Value::Null);
        assert_eq!(me["payment_failed_subscription_id"], subscription_id);
    }
}
