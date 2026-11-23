//! The language a request asked for: its `Accept-Language` matched to
//! one the app is in, `helpers::locales::resolve`, English when none is
//! or it sent none. What a mail goes in when the person chose no
//! language, see `app_core::accounts::preferences::mail_locale_in`.

use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::header::ACCEPT_LANGUAGE;
use axum::http::request::Parts;

use app_core::helpers::locales;

/// The language the request asked for, one of `app::locales::SUPPORTED`.
pub struct RequestLocale(pub &'static str);

impl<S> FromRequestParts<S> for RequestLocale
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<RequestLocale, Infallible> {
        let header = parts
            .headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        Ok(RequestLocale(locales::resolve(header)))
    }
}
