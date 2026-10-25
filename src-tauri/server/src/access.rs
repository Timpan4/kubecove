use crate::ApiError;
use axum::http::{header, HeaderMap, StatusCode};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use kubecove_lib::models::AppErrorKind;
use serde::Deserialize;
use std::time::{Duration, SystemTime};
use tokio::{sync::Mutex, time::Instant};

pub const ASSERTION_HEADER: &str = "cf-access-jwt-assertion";

#[derive(Clone)]
pub struct AccessIdentity {
    pub subject: String,
    pub expires_at: Instant,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
    #[serde(rename = "type")]
    token_type: String,
}

pub struct CloudflareAccess {
    client: reqwest::Client,
    certs_url: String,
    validation: Validation,
    keys: Mutex<Option<KeyCache>>,
}

struct KeyCache {
    jwks: JwkSet,
    fresh_until: Instant,
}

impl CloudflareAccess {
    pub fn new(team: &str, audience: &str) -> Result<Self, String> {
        // The team name forms one DNS label; it cannot select an arbitrary key server.
        if team.is_empty()
            || team.len() > 63
            || team.starts_with('-')
            || team.ends_with('-')
            || !team
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || audience.trim().is_empty()
        {
            return Err(
                "Cloudflare Access requires a valid team name and application AUD tag".into(),
            );
        }
        let issuer = format!("https://{team}.cloudflareaccess.com");
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&issuer]);
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.validate_nbf = true;
        validation.leeway = 0;
        crate::security::install_tls_provider();
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "failed to initialize Cloudflare Access key client")?;
        Ok(Self {
            client,
            certs_url: format!("{issuer}/cdn-cgi/access/certs"),
            validation,
            keys: Mutex::new(None),
        })
    }

    pub async fn verify(&self, token: &str) -> Result<AccessIdentity, ApiError> {
        let header = decode_header(token).map_err(|_| invalid_assertion())?;
        if header.alg != Algorithm::RS256 {
            return Err(invalid_assertion());
        }
        let kid = header.kid.ok_or_else(invalid_assertion)?;
        let key = {
            // Serialize refreshes and never fall back to stale keys on fetch failure.
            let mut cache = self.keys.lock().await;
            if !cache.as_ref().is_some_and(|cache| {
                cache.fresh_until > Instant::now() && cache.jwks.find(&kid).is_some()
            }) {
                *cache = Some(self.fetch_keys().await?);
            }
            let jwk = cache
                .as_ref()
                .and_then(|cache| cache.jwks.find(&kid))
                .ok_or_else(invalid_assertion)?;
            DecodingKey::from_jwk(jwk).map_err(|_| invalid_assertion())?
        };
        let claims = decode::<Claims>(token, &key, &self.validation)
            .map_err(|_| invalid_assertion())?
            .claims;
        if claims.sub.is_empty() || claims.token_type != "app" {
            return Err(invalid_assertion());
        }
        let remaining = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(claims.exp))
            .ok_or_else(invalid_assertion)?
            .duration_since(SystemTime::now())
            .map_err(|_| invalid_assertion())?;
        let expires_at = Instant::now()
            .checked_add(remaining)
            .ok_or_else(invalid_assertion)?;
        Ok(AccessIdentity {
            subject: claims.sub,
            expires_at,
        })
    }

    async fn fetch_keys(&self) -> Result<KeyCache, ApiError> {
        let response = self
            .client
            .get(&self.certs_url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|_| keys_unavailable())?;
        let lifetime = cache_lifetime(response.headers());
        let jwks: JwkSet = response.json().await.map_err(|_| keys_unavailable())?;
        if jwks.keys.is_empty() {
            return Err(keys_unavailable());
        }
        Ok(KeyCache {
            jwks,
            fresh_until: Instant::now()
                .checked_add(lifetime)
                .ok_or_else(keys_unavailable)?,
        })
    }
}

pub fn assertion(headers: &HeaderMap) -> Result<&str, ApiError> {
    let mut assertions = headers.get_all(ASSERTION_HEADER).iter();
    let token = assertions
        .next()
        .and_then(|value| value.to_str().ok())
        .filter(|token| !token.is_empty());
    if assertions.next().is_some() {
        return Err(invalid_assertion());
    }
    token.ok_or_else(invalid_assertion)
}

fn invalid_assertion() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "Cloudflare Access sign-in is required or has expired",
        AppErrorKind::Forbidden,
    )
}

fn keys_unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "Cloudflare Access signing keys are unavailable",
        AppErrorKind::Internal,
    )
}

fn cache_lifetime(headers: &HeaderMap) -> Duration {
    let Some(control) = headers
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
    else {
        return Duration::ZERO;
    };
    let mut max_age = 0;
    for directive in control.split(',').map(str::trim) {
        if directive.eq_ignore_ascii_case("no-cache") || directive.eq_ignore_ascii_case("no-store")
        {
            return Duration::ZERO;
        }
        if let Some((name, value)) = directive.split_once('=') {
            if name.eq_ignore_ascii_case("max-age") {
                max_age = value.trim_matches('"').parse::<u64>().unwrap_or(0);
            }
        }
    }
    let age = headers
        .get(header::AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    Duration::from_secs(max_age.saturating_sub(age))
}

#[cfg(test)]
#[path = "access_tests.rs"]
pub(crate) mod tests;
