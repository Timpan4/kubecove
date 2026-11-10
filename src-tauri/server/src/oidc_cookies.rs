use super::{AUTH_COOKIE, TRANSACTION_COOKIE, oidc_unavailable};
use crate::ApiError;
use axum::http::{HeaderMap, HeaderValue, header};

/// The single value of cookie `name`, or `None` when it is missing or repeated.
pub(super) fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut found = None;
    let cookies = headers.get_all(header::COOKIE).iter();
    for header in cookies {
        let value = header.to_str().ok()?;
        for entry in value.split(';').map(str::trim) {
            if let Some((key, value)) = entry.split_once('=')
                && key == name
            {
                if found.is_some() {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found
}

pub(super) fn cookie_header(value: &str) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(value).map_err(|_| oidc_unavailable())
}

pub(super) fn transaction_cookie(value: &str) -> String {
    format!("{TRANSACTION_COOKIE}={value}; Path=/; HttpOnly; Secure; SameSite=Lax")
}
pub(super) fn auth_cookie(value: &str) -> String {
    format!("{AUTH_COOKIE}={value}; Path=/; HttpOnly; Secure; SameSite=Strict")
}
pub fn clear_cookie_header(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

/// The request's sign-in credential: the authentication cookie or an
/// `Authorization: Bearer` header (ADR 0031). A request that carries both, or
/// a repeated cookie, presents none.
pub(super) fn presented_credential(headers: &HeaderMap) -> Option<&str> {
    let mut authorization = headers.get_all(header::AUTHORIZATION).iter();
    match (authorization.next(), authorization.next()) {
        (None, _) => cookie_value(headers, AUTH_COOKIE),
        (Some(value), None) if !names_cookie(headers, AUTH_COOKIE) => value
            .to_str()
            .ok()?
            .strip_prefix("Bearer ")
            .filter(|token| !token.is_empty()),
        _ => None,
    }
}

/// Whether the request names cookie `name`, whatever its value.
pub fn names_cookie(headers: &HeaderMap, name: &str) -> bool {
    headers.get_all(header::COOKIE).iter().any(|cookie| {
        cookie.to_str().is_ok_and(|cookie| {
            cookie.split(';').any(|entry| {
                entry
                    .trim()
                    .split_once('=')
                    .is_some_and(|(key, _)| key == name)
            })
        })
    })
}
