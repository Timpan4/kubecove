use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::{Value, json};

pub(crate) fn fixture() -> Value {
    serde_json::from_str(include_str!("access_test_key.json")).unwrap()
}

pub(crate) fn token(subject: &str) -> String {
    signed(
        &json!({
            "iss": "https://test.cloudflareaccess.com", "aud": ["test-audience"],
            "exp": jsonwebtoken::get_current_timestamp() + 3600,
            "sub": subject, "type": "app"
        }),
        Algorithm::RS256,
        "test-key",
    )
}

fn signed(claims: &Value, algorithm: Algorithm, kid: &str) -> String {
    let der = STANDARD
        .decode(fixture()["privateDer"].as_str().unwrap())
        .unwrap();
    let mut header = Header::new(algorithm);
    header.kid = Some(kid.into());
    encode(&header, claims, &EncodingKey::from_rsa_der(&der)).unwrap()
}

pub(crate) fn access() -> CloudflareAccess {
    let access = CloudflareAccess::new("test", "test-audience").unwrap();
    *access.keys.try_lock().unwrap() = Some(KeyCache {
        jwks: serde_json::from_value(json!({"keys": [fixture()["jwk"]]})).unwrap(),
        fresh_until: Instant::now() + Duration::from_secs(3600),
    });
    access
}

#[tokio::test]
async fn only_signed_unexpired_application_tokens_for_the_configured_issuer_and_audience_pass() {
    let access = access();
    let good = token("first-user");
    assert_eq!(access.verify(&good).await.unwrap().subject, "first-user");
    let now = jsonwebtoken::get_current_timestamp();
    let claims = json!({"iss":"https://test.cloudflareaccess.com","aud":["test-audience"],
        "exp":now+3600,"sub":"first-user","type":"app"});
    for (field, value) in [
        ("iss", json!("https://other.cloudflareaccess.com")),
        ("aud", json!(["other-application"])),
        ("exp", json!(now - 1)),
        ("nbf", json!(now + 3600)),
        ("sub", json!("")),
        ("type", json!("org")),
    ] {
        let mut invalid = claims.clone();
        invalid[field] = value;
        assert!(
            access
                .verify(&signed(&invalid, Algorithm::RS256, "test-key"))
                .await
                .is_err(),
            "{field}"
        );
    }
    for field in ["exp", "iss", "aud", "sub", "type"] {
        let mut invalid = claims.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(
            access
                .verify(&signed(&invalid, Algorithm::RS256, "test-key"))
                .await
                .is_err(),
            "missing {field}"
        );
    }
    assert!(
        access
            .verify(&signed(&claims, Algorithm::RS512, "test-key"))
            .await
            .is_err()
    );
    let (message, _) = good.rsplit_once('.').unwrap();
    assert!(access.verify(&format!("{message}.AAAA")).await.is_err());
}

#[test]
fn duplicate_assertions_and_plain_email_headers_are_not_credentials() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cf-access-authenticated-user-email",
        "email@timpan.me".parse().unwrap(),
    );
    assert!(assertion(&headers).is_err());
    headers.insert(ASSERTION_HEADER, "signed-jwt".parse().unwrap());
    assert_eq!(assertion(&headers).unwrap(), "signed-jwt");
    headers.append(ASSERTION_HEADER, "second-jwt".parse().unwrap());
    assert!(assertion(&headers).is_err());
}

#[test]
fn key_cache_uses_the_upstream_http_lifetime_and_age() {
    let mut headers = HeaderMap::new();
    assert_eq!(cache_lifetime(&headers), Duration::ZERO);
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=14400, must-revalidate".parse().unwrap(),
    );
    headers.insert(header::AGE, "10".parse().unwrap());
    assert_eq!(cache_lifetime(&headers), Duration::from_secs(14390));
    headers.insert(
        header::CACHE_CONTROL,
        "no-store, max-age=14400".parse().unwrap(),
    );
    assert_eq!(cache_lifetime(&headers), Duration::ZERO);
}

#[tokio::test]
async fn key_rotation_refreshes_unknown_ids_and_expired_cache_failures_deny_access() {
    use axum::{Json, Router, routing::get};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let requests = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&requests);
    let mut key = fixture()["jwk"].clone();
    key["kid"] = json!("rotated-key");
    let app = Router::new().route(
        "/certs",
        get(move || {
            count.fetch_add(1, Ordering::SeqCst);
            let key = key.clone();
            async move {
                (
                    [(header::CACHE_CONTROL, "max-age=14400")],
                    Json(json!({"keys":[key]})),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut access = access();
    // Only this test can replace the production HTTPS key endpoint.
    access.client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    access.certs_url = format!("http://{address}/certs");
    let claims = json!({"iss":"https://test.cloudflareaccess.com", "aud":["test-audience"],
        "exp":jsonwebtoken::get_current_timestamp()+3600, "sub":"user", "type":"app"});
    let rotated = signed(&claims, Algorithm::RS256, "rotated-key");
    assert!(access.verify(&rotated).await.is_ok());
    assert!(access.verify(&rotated).await.is_ok());
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        access
            .verify(&signed(&claims, Algorithm::RS256, "unknown-key"))
            .await
            .err()
            .unwrap()
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    access.keys.lock().await.as_mut().unwrap().fresh_until = Instant::now();
    access.certs_url = format!("http://{address}/unavailable");
    assert_eq!(
        access.verify(&rotated).await.err().unwrap().0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    server.abort();
    let _ = server.await;
}
