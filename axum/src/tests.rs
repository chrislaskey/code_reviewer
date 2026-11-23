//! The router's tests, one module per app, over an in-memory database:
//! each request goes through every route and extractor as a real one
//! does. The helpers here are what every module shares.

mod accounts;
mod app;
mod ops;
mod router;
mod servers;

use std::fs;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use app_core::accounts::Session;
use app_core::servers::settings::{ServerSettings, SettingsDocument};
use app_core::services::storage::Storage;

use app_core::deployment::Deployment;
use app_core::services::mailer::Mailer;
use app_core::services::stripe::Stripe;
use axum::Router;

use crate::config::stripe::StripeSigningSecret;
use crate::config::{ServerUrl, TrialUsersMax};
use crate::rate_limits::RateLimits;
use crate::router::router;
use crate::state::AppState;

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
    test_router_with(TrialUsersMax(2), false, ServerSettings::default()).await
}

/// `test_router` holding so many trial users, behind a reverse proxy or
/// not, with these settings.
async fn test_router_with(
    trial_users_max: TrialUsersMax,
    trust_proxy_headers: bool,
    settings: ServerSettings,
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
    fs::create_dir_all(dist.join("public/es")).expect("a language's dir");
    fs::write(
        dist.join("public/es/index.html"),
        "<html>public home es</html>",
    )
    .expect("es home");
    fs::write(
        dist.join("public/es/about.html"),
        "<html>public about es</html>",
    )
    .expect("es about");
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
    let post = app_core::app::posts::create(&deployment, &alice.user, "the first post", &[])
        .await
        .expect("a post");
    let post = app_core::app::posts::get(&deployment, &alice.user, post.id)
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
        server_description: test_server_description(&settings),
        server_settings: std::sync::Arc::new(SettingsDocument::of(settings)),
        listed_servers: std::sync::Arc::new(test_listed_servers()),
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

/// The server the tests describe: `test`, "Test Server".
fn test_server_description(settings: &ServerSettings) -> app_core::servers::ServerDescription {
    let name = app_core::servers::ServerName::parse("test").expect("a name");
    app_core::servers::ServerDescription::current(name, "Test Server", settings)
}

/// The one server the tests list, `acme` at `https://acme.example.com`.
fn test_listed_servers() -> app_core::servers::ListedServers {
    app_core::servers::ListedServers::parse("acme=https://acme.example.com").expect("listed")
}

/// A user signed up on the server and finished with their code.
async fn signed_up(deployment: &Deployment, mailer: &Mailer, email: &str) -> Session {
    let sent = app_core::accounts::sign_in(
        deployment,
        mailer,
        email,
        true,
        app_core::helpers::locales::DEFAULT,
    )
    .await
    .expect("signed in");
    let code = code_sent_to(mailer, email);
    let matched = app_core::accounts::sign_in_with_code(deployment, &sent.pending, &code)
        .await
        .expect("the code matched");
    assert!(matched.new_account);
    app_core::accounts::sign_in_confirm(deployment, &matched.pending)
        .await
        .expect("signed up")
}

/// The code the last email to the address carried.
fn code_sent_to(mailer: &Mailer, email: &str) -> String {
    let mail = mailer.last_sent_to(email).expect("an email");
    app_core::services::mailer::mails::code_in(&mail).expect("a code in it")
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
        // A device's request, which carries its protocol version
        request = request
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(
                app_core::sync::PROTOCOL_VERSION_HEADER,
                app_core::sync::PROTOCOL_VERSION.to_string(),
            );
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

/// The `Set-Cookie` of a sign-in step, when it set one.
async fn set_cookie_of(router: &Router, path: &str, body: Value) -> Option<String> {
    let (status, headers, _) = send_with(router, None, None, Method::POST, path, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{path}");
    headers
        .get(header::SET_COOKIE)
        .map(|cookie| cookie.to_str().expect("a cookie").to_string())
}

/// A sign-in of the email up to its last step: what the code answered.
async fn code_matched(router: &TestRouter, email: &str) -> Value {
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
    body
}
