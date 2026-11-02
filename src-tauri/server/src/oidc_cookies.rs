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
