use axum::http::{header, HeaderMap};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::io;
use subtle::ConstantTimeEq;

pub const COOKIE_NAME: &str = "kubecove_session";

pub fn random_token() -> io::Result<String> {
    // ADR 0020 requires 32 bytes from the OS CSPRNG.
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn equal_secret(left: &str, right: &str) -> bool {
    bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

/// Selects rustls' ring provider before the first TLS client is built. Only the first
/// installation in the process takes effect, so later calls are harmless.
pub fn install_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

pub fn authenticated(headers: &HeaderMap, token: &str) -> bool {
    if headers.contains_key(header::AUTHORIZATION) {
        return bearer(headers).is_some_and(|value| equal_secret(value, token));
    }
    headers.get_all(header::COOKIE).iter().any(|cookie| {
        cookie.to_str().is_ok_and(|cookie| {
            cookie.split(';').any(|entry| {
                entry
                    .trim()
                    .split_once('=')
                    .is_some_and(|(name, value)| name == COOKIE_NAME && equal_secret(value, token))
            })
        })
    })
}

#[derive(Clone)]
pub struct NetworkPolicy {
    pub hosts: Vec<String>,
    pub origins: Vec<String>,
    pub secure: bool,
}

impl NetworkPolicy {
    pub fn content_security_policy(&self, host: &str) -> String {
        let websocket_scheme = if self.secure { "wss" } else { "ws" };
        format!("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' {websocket_scheme}://{host}; img-src 'self' data:; font-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
    }

    pub fn check(&self, headers: &HeaderMap, check_origin: bool, token: &str) -> bool {
        let mut hosts = headers.get_all(header::HOST).iter();
        let Some(host) = hosts.next().and_then(|host| host.to_str().ok()) else {
            return false;
        };
        if hosts.next().is_some() || !self.hosts.iter().any(|allowed| allowed == host) {
            return false;
        }
        if !check_origin {
            return true;
        }
        let mut origins = headers.get_all(header::ORIGIN).iter();
        match origins.next() {
            Some(origin) => {
                origins.next().is_none()
                    && origin
                        .to_str()
                        .is_ok_and(|origin| self.origins.iter().any(|allowed| allowed == origin))
            }
            // Native clients cannot use a browser cookie to bypass Origin checks.
            None => bearer(headers).is_some_and(|value| equal_secret(value, token)),
        }
    }

    pub fn cookie(&self, token: &str) -> String {
        format!(
            "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict{}",
            if self.secure { "; Secure" } else { "" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> NetworkPolicy {
        NetworkPolicy {
            hosts: vec!["localhost:1234".into()],
            origins: vec!["http://localhost:1234".into()],
            secure: false,
        }
    }

    fn browser_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "localhost:1234".parse().unwrap());
        headers.insert(header::ORIGIN, "http://localhost:1234".parse().unwrap());
        headers
    }

    #[test]
    fn missing_and_wrong_tokens_never_authenticate() {
        let mut headers = browser_headers();
        assert!(!authenticated(&headers, "secret"));
        headers.insert(header::COOKIE, "kubecove_session=wrong".parse().unwrap());
        assert!(!authenticated(&headers, "secret"));
        headers.insert(header::COOKIE, "kubecove_session=secret".parse().unwrap());
        assert!(authenticated(&headers, "secret"));
        headers.insert(header::AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(!authenticated(&headers, "secret"));
    }

    #[test]
    fn host_rebinding_duplicate_hosts_and_cross_origin_are_rejected() {
        let mut headers = browser_headers();
        assert!(policy().check(&headers, true, "secret"));
        headers.insert(header::HOST, "evil.example:1234".parse().unwrap());
        assert!(!policy().check(&headers, true, "secret"));
        headers = browser_headers();
        headers.append(header::HOST, "localhost:1234".parse().unwrap());
        assert!(!policy().check(&headers, true, "secret"));
        headers = browser_headers();
        headers.insert(header::ORIGIN, "http://evil.example".parse().unwrap());
        assert!(!policy().check(&headers, true, "secret"));
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(!policy().check(&headers, true, "secret"));
    }

    #[test]
    fn only_native_bearer_can_omit_origin() {
        let mut headers = browser_headers();
        headers.remove(header::ORIGIN);
        headers.insert(header::COOKIE, "kubecove_session=secret".parse().unwrap());
        assert!(!policy().check(&headers, true, "secret"));
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(policy().check(&headers, true, "secret"));
    }

    #[test]
    fn tls_cookie_cannot_be_read_by_scripts_or_sent_cross_site() {
        let mut policy = policy();
        policy.secure = true;
        let cookie = policy.cookie("secret");
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.ends_with("; Secure"));
    }
}
