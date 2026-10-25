use super::*;
use axum::http::{header, HeaderMap};
use std::{path::PathBuf, time::Duration};

impl Oidc {
    async fn activate(
        &self,
        credential: &OidcCredential,
        owner: &str,
        scope: Arc<SessionScope>,
    ) -> Result<(), ApiError> {
        self.ensure_fresh(&credential.0).await?;
        self.bind(credential, owner, scope, &CancellationToken::new())
    }
}

#[tokio::test]
async fn expired_sign_in_cannot_refresh_or_publish() {
    let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let (_, auth) = signed_in(&oidc).await;
    let scope = SessionScope::new(None);
    oidc.activate(&auth.credential, "owner", Arc::clone(&scope))
        .await
        .unwrap();
    let before = provider.refreshes();
    oidc.credentials
        .lock()
        .unwrap()
        .get_mut(&auth.credential.0)
        .unwrap()
        .expires_unix = unix_now();
    assert!(oidc.ensure_fresh(&auth.credential.0).await.is_err());
    assert!(auth.revoked.is_cancelled());
    assert!(scope.bearer().is_none());
    assert!(oidc.bound.lock().unwrap().is_empty());
    assert_eq!(provider.refreshes(), before);
}

#[tokio::test]
async fn sign_in_expiring_during_refresh_cannot_publish_the_response() {
    let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let (_, auth) = signed_in(&oidc).await;
    let scope = SessionScope::new(None);
    // Bind a valid token first, then force renewal through the network barrier.
    provider.set_claim_mode(mock::ClaimMode::Valid);
    oidc.activate(&auth.credential, "owner", Arc::clone(&scope))
        .await
        .unwrap();
    provider.refresh_started.notified().await;
    oidc.credentials
        .lock()
        .unwrap()
        .get_mut(&auth.credential.0)
        .unwrap()
        .tokens
        .as_mut()
        .unwrap()
        .id_expires_unix = unix_now();
    let gate = provider.refresh_gate.lock().await;
    let mut renewal = Box::pin(oidc.ensure_fresh(&auth.credential.0));
    tokio::select! {
        () = provider.refresh_started.notified() => {},
        _ = &mut renewal => panic!("refresh must wait for the provider"),
    }
    oidc.credentials
        .lock()
        .unwrap()
        .get_mut(&auth.credential.0)
        .unwrap()
        .expires_unix = unix_now();
    drop(gate);
    assert!(renewal.await.is_err());
    assert!(auth.revoked.is_cancelled());
    assert!(scope.bearer().is_none());
}

#[tokio::test]
async fn provider_outage_suspends_expired_token_and_recovers_without_new_login() {
    let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let (headers, auth) = signed_in(&oidc).await;
    let scope = SessionScope::new(None);
    oidc.activate(&auth.credential, "owner", Arc::clone(&scope))
        .await
        .unwrap();
    provider.set_claim_mode(mock::ClaimMode::RefreshUnavailable);
    oidc.ensure_fresh(&auth.credential.0).await.unwrap();
    assert!(scope.bearer().is_some(), "unexpired tokens remain usable");
    let expires = oidc.credentials.lock().unwrap()[&auth.credential.0]
        .tokens
        .as_ref()
        .unwrap()
        .id_expires_unix;
    let deadline = std::time::UNIX_EPOCH + Duration::from_secs(expires);
    tokio::time::sleep(
        deadline
            .duration_since(std::time::SystemTime::now())
            .unwrap_or_default(),
    )
    .await;
    // No refresh tick occurs between expiry and the next cached-client request.
    assert!(!auth.revoked.is_cancelled());
    assert!(oidc.verify(&headers).is_ok());
    assert!(scope.bearer().is_none());
    assert_eq!(oidc.bound.lock().unwrap().len(), 1);
    provider.set_claim_mode(mock::ClaimMode::Valid);
    oidc.ensure_fresh(&auth.credential.0).await.unwrap();
    assert!(
        scope.bearer().is_some(),
        "recovery republishes to the existing session"
    );
}

#[tokio::test]
async fn unusable_refresh_retains_rotation_for_recovery() {
    for mode in [
        mock::ClaimMode::MissingIdToken,
        mock::ClaimMode::NonceMismatch,
    ] {
        let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
            .await
            .unwrap();
        provider.set_claim_mode(mock::ClaimMode::ShortLived);
        let (headers, auth) = signed_in(&oidc).await;
        let scope = SessionScope::new(None);
        oidc.activate(&auth.credential, "owner", Arc::clone(&scope))
            .await
            .unwrap();
        oidc.credentials
            .lock()
            .unwrap()
            .get_mut(&auth.credential.0)
            .unwrap()
            .tokens
            .as_mut()
            .unwrap()
            .id_expires_unix = unix_now();
        provider.set_claim_mode(mode);
        oidc.ensure_fresh(&auth.credential.0).await.unwrap();
        assert!(scope.bearer().is_none());
        assert!(oidc.verify(&headers).is_ok());
        provider.set_claim_mode(mock::ClaimMode::Valid);
        oidc.ensure_fresh(&auth.credential.0).await.unwrap();
        assert!(
            scope.bearer().is_some(),
            "rotated refresh token must permit recovery after {mode:?}"
        );
        assert!(!auth.revoked.is_cancelled());
    }
}

#[tokio::test]
async fn a_slow_refresh_does_not_block_another_sign_in() {
    use futures_util::FutureExt;
    let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    let (_, alice) = signed_in(&oidc).await;
    let (_, bob) = signed_in(&oidc).await;
    oidc.credentials
        .lock()
        .unwrap()
        .get_mut(&alice.credential.0)
        .unwrap()
        .tokens
        .as_mut()
        .unwrap()
        .id_expires_unix = unix_now();
    let gate = provider.refresh_gate.lock().await;
    let mut renewal = Box::pin(oidc.ensure_fresh(&alice.credential.0));
    tokio::select! {
        () = provider.refresh_started.notified() => {},
        _ = &mut renewal => panic!("Alice's renewal must wait for the provider"),
    }
    let scope = SessionScope::new(None);
    assert!(
        oidc.activate(&bob.credential, "bob-owner", Arc::clone(&scope))
            .now_or_never()
            .is_some_and(|result| result.is_ok()),
        "Bob's valid sign-in must bind while Alice waits for refresh"
    );
    assert!(scope.bearer().is_some());
    drop(gate);
    renewal.await.unwrap();
}

#[tokio::test]
async fn close_during_resume_cannot_restore_session_credentials() {
    use axum::{extract::State, Extension};
    use futures_util::FutureExt;
    let (oidc, _provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    let (_, auth) = signed_in(&oidc).await;
    let oidc = Arc::new(oidc);
    let state = Arc::new(crate::AppState {
        token: String::new(),
        policy: crate::security::NetworkPolicy {
            hosts: vec![],
            origins: vec![],
            secure: true,
        },
        sessions: Arc::new(crate::sessions::Sessions::new(Duration::from_mins(5))),
        credential_store: String::new(),
        access: None,
        oidc: Some(Arc::clone(&oidc)),
    });
    let open = crate::login(
        State(Arc::clone(&state)),
        Some(Extension(auth.identity.clone())),
        Some(Extension(auth.credential.clone())),
        HeaderMap::new(),
        Ok(Json(crate::Login {
            token: None,
            resume_token: None,
        })),
    )
    .await
    .unwrap();
    let body = axum::body::to_bytes(open.into_body(), usize::MAX)
        .await
        .unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let resume = info["resumeToken"].as_str().unwrap().to_owned();
    let owner = state.sessions.owner(&resume).await.unwrap();
    let refresh_gate = Arc::clone(&oidc.credentials.lock().unwrap()[&auth.credential.0].refreshing);
    let refreshing = refresh_gate.lock().await;
    let mut resuming = Box::pin(crate::login(
        State(Arc::clone(&state)),
        Some(Extension(auth.identity.clone())),
        Some(Extension(auth.credential.clone())),
        HeaderMap::new(),
        Ok(Json(crate::Login {
            token: None,
            resume_token: Some(resume.clone()),
        })),
    ));
    assert!(resuming.as_mut().now_or_never().is_none());
    state.sessions.close(&resume).await.unwrap();
    tokio::task::yield_now().await;
    assert!(oidc.bound.lock().unwrap().is_empty());
    assert!(owner.backend.scope.bearer().is_none());
    drop(refreshing);
    assert!(resuming.await.is_err());
    assert!(owner.backend.scope.bearer().is_none());
    assert!(oidc.bound.lock().unwrap().is_empty());
}

#[test]
fn config_rejects_http_and_issuer_credentials_or_components() {
    for issuer in [
        "http://id.example",
        "https://user:pass@id.example",
        "https://@id.example",
        "https://id.example?tenant=one",
        "https://id.example/#fragment",
    ] {
        assert!(
            OidcConfig::new(
                issuer.to_owned(),
                "client".into(),
                None,
                Duration::from_secs(30),
            )
            .is_err(),
            "accepted issuer {issuer}"
        );
    }
}

#[test]
fn config_rejects_empty_client_and_nonpositive_timeout() {
    assert!(OidcConfig::new(
        "https://id.example".into(),
        " ".into(),
        None,
        Duration::from_secs(30),
    )
    .is_err());
    assert!(OidcConfig::new(
        "https://id.example".into(),
        "client".into(),
        None,
        Duration::ZERO,
    )
    .is_err());
}

#[test]
fn public_origin_must_be_an_exact_https_origin() {
    for origin in [
        "http://app.example",
        "https://user:pass@app.example",
        "https://@app.example",
        "https://app.example/path",
        "https://app.example?x=1",
        "https://app.example/#frag",
    ] {
        assert!(validate_public_origin(origin).is_err(), "accepted {origin}");
    }
    assert_eq!(
        validate_public_origin("https://app.example").unwrap(),
        "https://app.example"
    );
}

#[test]
fn transaction_cookie_is_signed_expiring_and_bound_to_its_state() {
    let key = [7; 32];
    let transaction = Transaction {
        state: "state-1".into(),
        nonce: "nonce-1".into(),
        verifier: "verifier-1".into(),
        expires_at: unix_now() + 60,
    };
    let token = sign_transaction(&transaction, &key).unwrap();
    assert_eq!(
        verify_transaction(&token, &key, unix_now()).unwrap().state,
        "state-1"
    );
    assert!(verify_transaction(&format!("{token}x"), &key, unix_now()).is_err());
    assert!(verify_transaction(&token, &[8; 32], unix_now()).is_err());
    assert!(verify_transaction(&token, &key, transaction.expires_at).is_err());
    assert!(!equal_secret("state-1", "state-2"));
    assert!(equal_secret("state-1", "state-1"));
}

#[test]
fn oidc_cookies_are_host_scoped_and_clearing_is_explicit() {
    assert_eq!(AUTH_COOKIE, "__Host-kubecove_oidc");
    let transaction = transaction_cookie("signed");
    assert!(transaction.contains("HttpOnly"));
    assert!(transaction.contains("Secure"));
    assert!(transaction.contains("SameSite=Lax"));
    assert!(transaction.contains("Path=/"));
    assert!(!transaction.contains("Domain="));
    let auth = auth_cookie("opaque");
    assert!(auth.contains("HttpOnly"));
    assert!(auth.contains("Secure"));
    assert!(auth.contains("SameSite=Strict"));
    assert!(clear_cookie_header(TRANSACTION_COOKIE).contains("Max-Age=0"));
    assert!(clear_cookie_header(AUTH_COOKIE).contains("Max-Age=0"));
}

#[tokio::test]
async fn local_provider_completes_authorization_code_pkce_nonce_and_cookie_login() {
    let app_origin = "http://localhost:43129";
    let (oidc, provider) = mock::fixture(app_origin).await.unwrap();
    let start = oidc.start().unwrap();
    assert_eq!(start.status(), StatusCode::OK);
    let transaction_cookie = start
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body = axum::body::to_bytes(start.into_body(), usize::MAX)
        .await
        .unwrap();
    let auth_url = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["authorizationUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let authorization = client.get(auth_url).send().await.unwrap();
    let callback_url = authorization
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    let callback = openidconnect::url::Url::parse(callback_url).unwrap();
    let query = callback
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let response = oidc
        .callback(
            &{
                let mut headers = HeaderMap::new();
                headers.insert(header::COOKIE, transaction_cookie.parse().unwrap());
                headers
            },
            Callback {
                code: query.get("code").cloned(),
                state: query.get("state").cloned(),
                error: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/");
    let set_cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>();
    assert!(set_cookies
        .iter()
        .any(|value| value.starts_with(AUTH_COOKIE)));
    assert!(set_cookies
        .iter()
        .any(|value| value.starts_with(TRANSACTION_COOKIE) && value.contains("Max-Age=0")));
    let auth_cookie = set_cookies
        .iter()
        .find(|value| value.starts_with(AUTH_COOKIE))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, auth_cookie.parse().unwrap());
    let session = oidc.verify(&headers).unwrap();
    assert_eq!(
        session.identity.subject,
        format!("{}\u{1f}mock-user", provider.base_url)
    );
    assert!(!session.revoked.is_cancelled());
}

#[test]
fn discovery_metadata_requires_exact_issuer_https_endpoints_and_token_endpoint() {
    let valid = test_metadata("https://id.example").unwrap();
    assert!(validate_metadata("https://id.example", &valid, false).is_ok());
    assert!(validate_metadata("https://attacker.example", &valid, false).is_err());
    let mut value = serde_json::to_value(valid).unwrap();
    value.as_object_mut().unwrap().remove("token_endpoint");
    let no_token_endpoint: CoreProviderMetadata = serde_json::from_value(value).unwrap();
    assert!(validate_metadata("https://id.example", &no_token_endpoint, false).is_err());
    let mut value = serde_json::to_value(test_metadata("https://id.example").unwrap()).unwrap();
    value["jwks_uri"] = serde_json::json!("http://id.example/jwks");
    let insecure: CoreProviderMetadata = serde_json::from_value(value).unwrap();
    assert!(validate_metadata("https://id.example", &insecure, false).is_err());
}

#[test]
fn root_issuer_with_trailing_slash_is_valid() {
    assert!(OidcConfig::new(
        "https://identity.example/".into(),
        "client".into(),
        None,
        Duration::from_secs(1)
    )
    .is_ok());
}

#[tokio::test]
async fn signed_mock_provider_rejects_invalid_id_token_claims() {
    let (oidc, provider) = mock::fixture("http://localhost:43132").await.unwrap();
    for mode in [
        mock::ClaimMode::WrongIssuer,
        mock::ClaimMode::WrongAudience,
        mock::ClaimMode::WrongSignature,
        mock::ClaimMode::Expired,
        mock::ClaimMode::NonceMismatch,
        mock::ClaimMode::WrongAuthorizedParty,
        mock::ClaimMode::MultiAudience,
        mock::ClaimMode::FutureIssuedAt,
        mock::ClaimMode::FutureNotBefore,
        mock::ClaimMode::UnsupportedAlgorithm,
        mock::ClaimMode::MissingIdToken,
        mock::ClaimMode::EmptySubject,
    ] {
        provider.set_claim_mode(mode);
        let (headers, callback) = pending_callback(&oidc).await;
        let response = oidc.callback(&headers, callback).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "mode: {mode:?}");
        assert!(response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|value| value.to_str().unwrap().contains("Max-Age=0")));
        assert!(!response.headers().contains_key(header::LOCATION));
    }
}

#[tokio::test]
async fn state_cookie_is_browser_bound_and_authorization_code_replay_fails() {
    let (oidc, provider) = mock::fixture("http://localhost:43133").await.unwrap();
    let (first_headers, _first_callback) = pending_callback(&oidc).await;
    let (_second_headers, second_callback) = pending_callback(&oidc).await;
    let before = provider.exchanges();
    let wrong_state = oidc
        .callback(&first_headers, second_callback)
        .await
        .unwrap();
    assert_eq!(wrong_state.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        provider.exchanges(),
        before,
        "state must fail before code exchange"
    );

    let (headers, callback) = pending_callback(&oidc).await;
    let success = oidc.callback(&headers, callback.clone()).await.unwrap();
    assert_eq!(success.status(), StatusCode::SEE_OTHER);
    let replay = oidc.callback(&headers, callback).await.unwrap();
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    assert!(replay
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|value| value.to_str().unwrap().contains("Max-Age=0")));
}

async fn signed_in(oidc: &Oidc) -> (HeaderMap, crate::oidc::AuthSession) {
    let (headers, callback) = pending_callback(oidc).await;
    let response = oidc.callback(&headers, callback).await.unwrap();
    let cookie = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with(AUTH_COOKIE))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    let session = oidc.verify(&headers).unwrap();
    (headers, session)
}

#[tokio::test]
async fn forwarded_id_token_follows_session_refresh_and_revocation() {
    use kubecove_lib::commands::SessionScope;
    let (oidc, provider) = mock::fixture_with("http://localhost:43134", Some("kubernetes"))
        .await
        .unwrap();
    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let (headers, session) = signed_in(&oidc).await;
    // The session outlives the two-second ID token.
    assert!(session.identity.expires_at > Instant::now() + Duration::from_hours(1));
    let (_, other) = signed_in(&oidc).await;

    // The login token expires inside the refresh margin, so activation renews it.
    let first_scope = SessionScope::new(None);
    oidc.activate(&session.credential, "owner-1", Arc::clone(&first_scope))
        .await
        .unwrap();
    assert_eq!(provider.refreshes(), 1);
    let first = first_scope.bearer().unwrap();
    assert!(first.starts_with("Bearer ey"));

    // A second sign-in's session carries its own token beside the first.
    provider.set_claim_mode(mock::ClaimMode::Valid);
    let other_scope = SessionScope::new(None);
    oidc.activate(&other.credential, "owner-other", Arc::clone(&other_scope))
        .await
        .unwrap();
    let other_token = other_scope.bearer().unwrap();
    assert_ne!(other_token, first);
    assert_eq!(first_scope.bearer().as_deref(), Some(first.as_str()));

    oidc.release_owner("owner-0");
    assert_eq!(first_scope.bearer().as_deref(), Some(first.as_str()));
    oidc.release_owner("owner-1");
    assert_eq!(first_scope.bearer(), None);
    assert_eq!(other_scope.bearer(), Some(other_token.clone()));

    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let second_scope = SessionScope::new(None);
    oidc.activate(&session.credential, "owner-2", Arc::clone(&second_scope))
        .await
        .unwrap();
    assert_eq!(provider.refreshes(), 3);
    assert_ne!(second_scope.bearer().unwrap(), first);

    // A refusal while the token still works keeps the session.
    provider.set_claim_mode(mock::ClaimMode::RefreshRefused);
    oidc.ensure_fresh(&session.credential.0).await.unwrap();
    assert!(second_scope.bearer().is_some());

    // A refusal after it expired revokes that sign-in and stops its
    // forwarding, and only its.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(oidc.ensure_fresh(&session.credential.0).await.is_err());
    assert!(session.revoked.is_cancelled());
    assert!(oidc.verify(&headers).is_err());
    assert_eq!(second_scope.bearer(), None);
    assert_eq!(other_scope.bearer(), Some(other_token));
}

#[tokio::test]
async fn forwarding_requires_the_kubernetes_audience_and_trusts_no_other() {
    let (oidc, provider) = mock::fixture_with("http://localhost:43136", Some("kubernetes"))
        .await
        .unwrap();
    for (mode, signs_in) in [
        (mock::ClaimMode::Valid, true),
        (mock::ClaimMode::WithoutExtraAudience, false),
        (mock::ClaimMode::MultiAudience, false),
    ] {
        provider.set_claim_mode(mode);
        let (headers, callback) = pending_callback(&oidc).await;
        let response = oidc.callback(&headers, callback).await.unwrap();
        assert_eq!(
            response.status() == StatusCode::SEE_OTHER,
            signs_in,
            "{mode:?}"
        );
    }
}

#[tokio::test]
async fn sign_in_without_forwarding_keeps_no_tokens_and_ends_with_the_id_token() {
    let (oidc, _provider) = mock::fixture("http://localhost:43135").await.unwrap();
    let (headers, callback) = pending_callback(&oidc).await;
    let response = oidc.callback(&headers, callback).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let credentials = oidc.credentials.lock().unwrap();
    let credential = credentials.values().next().unwrap();
    assert!(credential.tokens.is_none());
    assert!(credential.expires_unix <= unix_now() + 300);
}

async fn pending_callback(oidc: &Oidc) -> (HeaderMap, Callback) {
    let start = oidc.start().unwrap();
    let cookie = start
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body = axum::body::to_bytes(start.into_body(), usize::MAX)
        .await
        .unwrap();
    let auth_url = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["authorizationUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let authorization = client.get(auth_url).send().await.unwrap();
    let location = authorization
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    let query = openidconnect::url::Url::parse(location)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    (
        headers,
        Callback {
            code: query.get("code").cloned(),
            state: query.get("state").cloned(),
            error: None,
        },
    )
}

#[tokio::test]
async fn provider_denial_clears_transaction_without_echoing_provider_error() {
    let (oidc, _provider) = mock::fixture("http://localhost:43130").await.unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{TRANSACTION_COOKIE}=signed-value")
            .parse()
            .unwrap(),
    );
    let response = oidc
        .callback(
            &headers,
            Callback {
                code: None,
                state: None,
                error: Some("provider-secret-detail".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|value| value.to_str().unwrap().contains("Max-Age=0")));
}

#[tokio::test]
async fn revoked_and_expired_credentials_are_not_verified() {
    let config = OidcConfig::new(
        "https://id.example".into(),
        "client".into(),
        Some(PathBuf::from("secret")),
        Duration::from_secs(30),
    )
    .unwrap();
    let oidc = Oidc::new_for_test(config, "https://app.example").unwrap();
    let session = oidc.insert_identity_for_test("https://id.example", "subject", unix_now() + 60);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{AUTH_COOKIE}={session}").parse().unwrap(),
    );
    assert_eq!(
        oidc.verify(&headers).unwrap().identity.subject,
        "https://id.example\u{1f}subject"
    );
    oidc.revoke_for_test(&session);
    assert!(oidc.verify(&headers).is_err());
}

#[tokio::test]
async fn callback_errors_are_generic_and_clear_transaction_cookie() {
    let (oidc, _provider) = mock::fixture("http://localhost:43131").await.unwrap();
    let response = oidc
        .callback(
            &HeaderMap::new(),
            Callback {
                code: Some("must-not-be-echoed".into()),
                state: Some("wrong".into()),
                error: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!response.headers().contains_key(header::LOCATION));
    assert!(response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|value| {
                value.starts_with(TRANSACTION_COOKIE) && value.contains("Max-Age=0")
            })
        }));
}
