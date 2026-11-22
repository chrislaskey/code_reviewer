//! Every route: the JSON API under `/api`, grouped by app as core is,
//! with the ops app's under `/api/ops`, see `ops`, what the server says
//! it is under `/api/servers`, see `servers`, the public website's
//! pages, see `public_pages`, the ops app's pages, then the Svelte pages.
//! A path none of them and the built files don't know gets `index.html`,
//! so the pages' router (`svelte/src/lib/router.js`) shows the page at
//! that path, and every page is reloadable and linkable.

use std::path::Path;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{Method, StatusCode, Uri};
use axum::middleware;
use axum::routing::{any, delete, get, post, put};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::accounts::{
    app_store_notifications, checkouts, deletions, password, products, purchases,
    store_transactions, stripe_events, subscriptions, trial_users,
};
use crate::api_error::ApiError;
use crate::app::{
    avatars, communities, current_community, current_user, files, memberships, posts,
};
use crate::state::AppState;
use crate::sync;
use crate::{accounts, ops, public_pages, rate_limits, servers};

pub fn router(svelte_dist: &Path, ops_dist: &Path, state: AppState) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));
    Router::new()
        .merge(app_api())
        .merge(sync_api())
        .merge(accounts_api(&state))
        .merge(servers_api(&state))
        .merge(ops::api(state.clone()))
        .route("/api", any(api_not_found))
        .route("/api/{*path}", any(api_not_found))
        .with_state(state.clone())
        .merge(public_pages::router(svelte_dist))
        .merge(ops::pages(ops_dist, state.clone()))
        .fallback_service(pages)
        .layer(TraceLayer::new_for_http())
}

/// The app's API, `app`.
fn app_api() -> Router<AppState> {
    Router::new()
        .route(
            "/api/users/current",
            get(current_user::get).put(current_user::update),
        )
        .route(
            "/api/communities/current",
            get(current_community::get).put(current_community::update),
        )
        .route("/api/users/current/avatar", put(avatars::update))
        .route(
            "/api/users/current/avatar/uploads",
            post(avatars::uploads::create),
        )
        .route("/api/posts", get(posts::list).post(posts::create))
        .route(
            "/api/posts/{id}",
            get(posts::get).put(posts::update).delete(posts::delete),
        )
        .route("/api/files", get(files::list).post(files::create))
        .route("/api/files/uploads", post(files::uploads::create))
        // A read, but a POST: hundreds of checksums do not fit a URL.
        .route("/api/files/duplicates", post(files::duplicates::list))
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
            "/api/communities/{community_id}/membership",
            post(memberships::create).delete(memberships::delete),
        )
}

/// The sync API, `sync`, for the devices: every route requires the
/// device's protocol version to be this server's.
fn sync_api() -> Router<AppState> {
    Router::new()
        .route(
            "/api/sync/push",
            post(sync::push).layer(DefaultBodyLimit::max(app_core::sync::MAX_PUSH_BYTES)),
        )
        .route("/api/sync/pull", post(sync::pull))
        .route(
            "/api/sync/bytes/{table}/uploads",
            post(sync::bytes::uploads::create),
        )
        .route(
            "/api/sync/bytes/{table}/{id}/content",
            get(sync::bytes::content),
        )
        .route("/api/sync/backfill", post(sync::backfill))
        .route_layer(middleware::from_fn(sync::protocol_version_required))
}

/// What this server says it is, and the servers it lists, rate limited
/// per client IP so names cannot be guessed quickly, see `servers`.
fn servers_api(state: &AppState) -> Router<AppState> {
    let listed = Router::new()
        .route("/api/servers/{name}", get(servers::get))
        .layer(rate_limits::per_ip_layer(state.trust_proxy_headers));
    Router::new()
        .route("/api/servers/current", get(servers::current::get))
        .merge(listed)
}

/// The accounts app's API, `accounts`: signing in and out, rate limited
/// per client IP, then the account and what it buys.
fn accounts_api(state: &AppState) -> Router<AppState> {
    let sign_in = Router::new()
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
        .route("/api/accounts/current/password", put(password::update))
        .route("/api/accounts/device-token", post(accounts::device_token))
        .route("/api/accounts/sign-out", delete(accounts::sign_out))
        .route(
            "/api/accounts/deletions/code",
            post(deletions::code::create),
        )
        .route("/api/accounts/deletions", post(deletions::create))
        .route(
            "/api/accounts/trial",
            get(trial_users::get).post(trial_users::create),
        )
        .layer(rate_limits::per_ip_layer(state.trust_proxy_headers));
    Router::new()
        .merge(sign_in)
        .route("/api/accounts/webhooks/stripe", post(stripe_events::create))
        .route(
            "/api/accounts/webhooks/app-store",
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
            post(subscriptions::portal::create),
        )
}

/// An API path no route has: a JSON 404, so a page asking a server that
/// lacks the route sees an error and not the pages' HTML.
pub(crate) async fn api_not_found(method: Method, uri: Uri) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        format!("The server has no API route {method} {}", uri.path()),
    )
}
