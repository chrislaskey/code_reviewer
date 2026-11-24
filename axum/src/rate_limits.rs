//! Rate limits on the accounts routes, with `governor`, each a bucket per
//! key in this process's memory, see "Rate limits" in
//! `docs/design/accounts-sign-in.md`: the emails sent per address, so
//! nobody has the server mail an address over and over; the password
//! tries per address, so nobody guesses one, while the emailed code
//! stays a way in; the new codes per pending sign-in; the trial users
//! made per client IP; and, as a
//! layer on the routes, the requests per client IP. The buckets are
//! forgotten on a restart, which is fine for these; `forget_old` keeps
//! the maps from growing with every address ever tried. Behind a
//! reverse proxy the client IP is the last entry of the
//! `X-Forwarded-For` header, the one the proxy wrote, trusted only when
//! `TRUST_PROXY_HEADERS` says the proxy is the only way in, since
//! anyone can send the header, and the entries before the last are
//! whatever the client sent.

use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{Extensions, HeaderMap, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use serde_json::json;
use tower_governor::GovernorLayer;
use tower_governor::errors::GovernorError;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;

use crate::api_error::ApiError;
use crate::state::AppState;

/// Emails an address is sent in an hour, codes and the rest.
pub const EMAILS_PER_HOUR_PER_ADDRESS: u32 = 5;
/// Password tries an address gets in `PASSWORD_TRIES_MINUTES`.
pub const PASSWORD_TRIES_PER_ADDRESS: u32 = 5;
/// The minutes the password tries of an address come back over.
pub const PASSWORD_TRIES_MINUTES: u64 = 15;
/// New codes a pending sign-in gets in a minute.
pub const RESENDS_PER_MINUTE_PER_SIGN_IN: u32 = 1;
/// Trial users a client IP makes in an hour: enough for a classroom
/// behind one address, too few for a script to fill the cap quickly.
pub const TRIAL_USERS_PER_HOUR_PER_IP: u32 = 10;
/// Requests to the accounts routes a client IP makes in a minute.
pub const ACCOUNT_REQUESTS_PER_MINUTE_PER_IP: u32 = 30;

const TOO_MANY: &str = "Too many attempts; try again in a few minutes";
const TOO_MANY_PASSWORDS: &str =
    "Too many tries. Try your password again in a few minutes, or log in with an emailed code now.";
const TOO_MANY_PASSWORDS_WITHOUT_CODE: &str =
    "Too many tries. Try your password again in a few minutes.";

/// The limits the accounts handlers check before calling core.
#[derive(Clone)]
pub struct RateLimits {
    limiters: Arc<Limiters>,
}

struct Limiters {
    emails: DefaultKeyedRateLimiter<String>,
    passwords: DefaultKeyedRateLimiter<String>,
    resends: DefaultKeyedRateLimiter<String>,
    trial_users: DefaultKeyedRateLimiter<IpAddr>,
}

impl RateLimits {
    pub fn new() -> RateLimits {
        RateLimits {
            limiters: Arc::new(Limiters {
                emails: RateLimiter::keyed(Quota::per_hour(nonzero(EMAILS_PER_HOUR_PER_ADDRESS))),
                passwords: RateLimiter::keyed(password_quota()),
                resends: RateLimiter::keyed(Quota::per_minute(nonzero(
                    RESENDS_PER_MINUTE_PER_SIGN_IN,
                ))),
                trial_users: RateLimiter::keyed(Quota::per_hour(nonzero(
                    TRIAL_USERS_PER_HOUR_PER_IP,
                ))),
            }),
        }
    }

    /// One more email to the address, or 429.
    pub fn check_email(&self, email: &str) -> Result<(), ApiError> {
        let key = email.trim().to_lowercase();
        self.limiters
            .emails
            .check_key(&key)
            .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, TOO_MANY))
    }

    /// One more password try for the address, or 429, which points to
    /// the emailed code when the server offers one.
    pub fn check_password(&self, email: &str, sign_in_code_offered: bool) -> Result<(), ApiError> {
        let key = email.trim().to_lowercase();
        let message = if sign_in_code_offered {
            TOO_MANY_PASSWORDS
        } else {
            TOO_MANY_PASSWORDS_WITHOUT_CODE
        };
        self.limiters
            .passwords
            .check_key(&key)
            .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, message))
    }

    /// One more code for the pending sign-in, or 429.
    pub fn check_resend(&self, pending: &str) -> Result<(), ApiError> {
        self.limiters
            .resends
            .check_key(&pending.to_string())
            .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, TOO_MANY))
    }

    /// One more trial user from the client IP, or 429.
    pub fn check_trial_user(&self, client_ip: IpAddr) -> Result<(), ApiError> {
        self.limiters
            .trial_users
            .check_key(&client_ip)
            .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, TOO_MANY))
    }

    /// Drops the buckets that are full again, for a task once an hour.
    pub fn forget_old(&self) {
        self.limiters.emails.retain_recent();
        self.limiters.passwords.retain_recent();
        self.limiters.resends.retain_recent();
        self.limiters.trial_users.retain_recent();
    }
}

impl Default for RateLimits {
    fn default() -> RateLimits {
        RateLimits::new()
    }
}

/// The layer on the accounts routes: `ACCOUNT_REQUESTS_PER_MINUTE_PER_IP`
/// per client IP, answered 429 as the API answers any error.
pub fn per_ip_layer(
    trust_proxy_headers: bool,
) -> GovernorLayer<ClientIp, NoOpMiddleware<QuantaInstant>, Body> {
    let mut builder = GovernorConfigBuilder::default().key_extractor(ClientIp {
        trust_proxy_headers,
    });
    builder
        .period(Duration::from_secs(60) / ACCOUNT_REQUESTS_PER_MINUTE_PER_IP)
        .burst_size(ACCOUNT_REQUESTS_PER_MINUTE_PER_IP);
    let config = builder
        .finish()
        .unwrap_or_else(|| unreachable!("a period and a burst size were given"));
    GovernorLayer::new(config).error_handler(too_many_requests)
}

fn too_many_requests(error: GovernorError) -> Response {
    match error {
        GovernorError::TooManyRequests { wait_time, .. } => (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, wait_time.to_string())],
            Json(json!({ "error": TOO_MANY })),
        )
            .into_response(),
        other => {
            tracing::error!("rate limit: {other}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": other.to_string() })),
            )
                .into_response()
        }
    }
}

/// The client's IP as the key, see `client_ip`.
#[derive(Clone, Debug)]
pub struct ClientIp {
    trust_proxy_headers: bool,
}

impl KeyExtractor for ClientIp {
    type Key = IpAddr;

    fn extract<T>(&self, request: &Request<T>) -> Result<IpAddr, GovernorError> {
        Ok(client_ip(
            self.trust_proxy_headers,
            request.headers(),
            request.extensions(),
        ))
    }
}

/// The client's IP, for a handler that limits by it.
pub struct ClientIpAddr(pub IpAddr);

impl FromRequestParts<AppState> for ClientIpAddr {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<ClientIpAddr, ApiError> {
        Ok(ClientIpAddr(client_ip(
            state.trust_proxy_headers,
            &parts.headers,
            &parts.extensions,
        )))
    }
}

/// The client's IP: the last entry of the proxy's `X-Forwarded-For`
/// when trusted, which the proxy wrote, where the entries before it
/// came from the client; else the peer address the server was started
/// with; else, with neither, one shared key, so a server missing its
/// connect info limits everyone together rather than nobody.
fn client_ip(trust_proxy_headers: bool, headers: &HeaderMap, extensions: &Extensions) -> IpAddr {
    if trust_proxy_headers
        && let Some(forwarded) = headers
            .get_all("x-forwarded-for")
            .iter()
            .next_back()
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit(',').next())
            .and_then(|last| last.trim().parse::<IpAddr>().ok())
    {
        return forwarded;
    }
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::from([0, 0, 0, 0]), |info| info.0.ip())
}

/// `PASSWORD_TRIES_PER_ADDRESS` at once, each coming back over its share
/// of `PASSWORD_TRIES_MINUTES`.
fn password_quota() -> Quota {
    let every = Duration::from_secs(PASSWORD_TRIES_MINUTES * 60) / PASSWORD_TRIES_PER_ADDRESS;
    Quota::with_period(every)
        .unwrap_or_else(|| unreachable!("a period of minutes is not zero"))
        .allow_burst(nonzero(PASSWORD_TRIES_PER_ADDRESS))
}

fn nonzero(count: u32) -> NonZeroU32 {
    NonZeroU32::new(count).unwrap_or(NonZeroU32::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limits_count_emails_and_passwords_per_address_resends_per_sign_in_and_trial_users_per_client()
     {
        let limits = RateLimits::new();
        for _ in 0..EMAILS_PER_HOUR_PER_ADDRESS {
            limits
                .check_email(" Alice@Example.com ")
                .expect("within the hour's five");
        }
        assert!(
            limits.check_email("alice@example.com").is_err(),
            "the same address"
        );
        limits
            .check_email("bob@example.com")
            .expect("another address");
        for _ in 0..PASSWORD_TRIES_PER_ADDRESS {
            limits
                .check_password(" Alice@Example.com ", true)
                .expect("within the address's tries");
        }
        assert!(
            limits.check_password("alice@example.com", true).is_err(),
            "the same address"
        );
        limits
            .check_password("bob@example.com", true)
            .expect("another address");
        limits.check_resend("pending-1").expect("the first");
        assert!(
            limits.check_resend("pending-1").is_err(),
            "within the minute"
        );
        limits.check_resend("pending-2").expect("another sign-in");
        let client: IpAddr = "203.0.113.9".parse().unwrap();
        for _ in 0..TRIAL_USERS_PER_HOUR_PER_IP {
            limits.check_trial_user(client).expect("within the hour's");
        }
        assert!(limits.check_trial_user(client).is_err(), "the same client");
        limits
            .check_trial_user("203.0.113.10".parse().unwrap())
            .expect("another client");
        limits.forget_old();
    }

    #[test]
    fn rate_limits_key_by_the_peer_or_the_trusted_header() {
        let peer: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        let request = Request::builder()
            .header("x-forwarded-for", "198.51.100.7")
            .header("x-forwarded-for", "10.0.0.2, 203.0.113.9")
            .extension(ConnectInfo(peer))
            .body(())
            .unwrap();
        let untrusting = ClientIp {
            trust_proxy_headers: false,
        };
        assert_eq!(untrusting.extract(&request).unwrap(), peer.ip());
        let trusting = ClientIp {
            trust_proxy_headers: true,
        };
        assert_eq!(
            trusting.extract(&request).unwrap(),
            "203.0.113.9".parse::<IpAddr>().unwrap(),
            "the last entry, the proxy's; the others are the client's to write"
        );
        let bare = Request::builder().body(()).unwrap();
        assert_eq!(
            untrusting.extract(&bare).unwrap(),
            IpAddr::from([0, 0, 0, 0]),
            "one shared key without connect info"
        );
    }
}
