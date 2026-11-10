// Failure modes: a redirect URI that is not an exact loopback callback, a
// missing or plain challenge, a code used twice or after its deadline, a code
// presented with the wrong verifier or redirect URI, a replay that leaves the
// first credential usable, a provider denial that strands the desktop, and a
// native transaction that sets a browser cookie.
use super::*;
use crate::oidc::{AUTH_COOKIE, Callback, mock, tests::pending_callback};
use axum::http::header;
use std::collections::HashMap as Map;

// RFC 7636 appendix B.
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const STATE: &str = "desktop-state-desktop-state-desktop-state-x";
const REDIRECT: &str = "http://127.0.0.1:50123/kubecove/callback";

fn start(redirect: &str, challenge: Option<&str>, method: Option<&str>) -> NativeStart {
    NativeStart {
        redirect_uri: Some(redirect.into()),
        code_challenge: challenge.map(str::to_owned),
        code_challenge_method: method.map(str::to_owned),
        state: Some(STATE.into()),
    }
}

fn query(location: &str) -> Map<String, String> {
    Url::parse(location)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

fn location(response: &Response) -> String {
    response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned()
}

/// Runs the browser half of a native sign-in and returns the provider's
/// callback, its transaction cookie, and the provider state.
async fn native_callback(oidc: &Oidc) -> (HeaderMap, Callback, String) {
    let response = oidc
        .native_start(start(REDIRECT, Some(CHALLENGE), Some("S256")))
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let provider_state = query(&location(&response))["state"].clone();
    let authorization = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(location(&response))
        .send()
        .await
        .unwrap();
    let callback = query(authorization.headers()[header::LOCATION].to_str().unwrap());
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    (
        headers,
        Callback {
            code: callback.get("code").cloned(),
            state: callback.get("state").cloned(),
            error: None,
        },
        provider_state,
    )
}

/// A completed native callback's single-use code.
async fn native_code(oidc: &Oidc) -> String {
    let (headers, callback, _) = native_callback(oidc).await;
    let response = oidc.callback(&headers, callback).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let loopback = location(&response);
    assert!(loopback.starts_with(REDIRECT), "redirects to the desktop");
    let parameters = query(&loopback);
    assert_eq!(parameters["state"], STATE);
    parameters["code"].clone()
}

async fn exchange(
    oidc: &Oidc,
    code: &str,
    verifier: &str,
    redirect: &str,
) -> Result<String, ApiError> {
    let response = oidc.native_token(NativeToken {
        code: code.into(),
        code_verifier: verifier.into(),
        redirect_uri: redirect.into(),
    })?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Ok(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["credential"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn bearer(credential: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {credential}").parse().unwrap(),
    );
    headers
}

#[test]
fn native_start_accepts_only_exact_loopback_callbacks_with_s256() {
    for redirect in [
        "http://localhost:50123/kubecove/callback",
        "https://127.0.0.1:50123/kubecove/callback",
        "http://127.0.0.1:50123/other",
        "http://127.0.0.1:50123/kubecove/callback/",
        "http://127.0.0.1:50123/kubecove/callback?next=x",
        "http://127.0.0.1/kubecove/callback",
        "http://127.0.0.1:0/kubecove/callback",
        "http://127.0.0.1:050123/kubecove/callback",
        "http://127.0.0.1:+5012/kubecove/callback",
        "http://127.0.0.1:70000/kubecove/callback",
        "http://127.0.0.1:50123@evil.example/kubecove/callback",
        "http://10.0.0.1:50123/kubecove/callback",
        "http://[::2]:50123/kubecove/callback",
    ] {
        assert!(
            start(redirect, Some(CHALLENGE), Some("S256"))
                .validate()
                .is_none(),
            "accepted {redirect}"
        );
    }
    for (challenge, method) in [
        (None, Some("S256")),
        (Some(CHALLENGE), None),
        (Some(CHALLENGE), Some("plain")),
        (Some(VERIFIER), Some("plain")),
        (Some("too-short"), Some("S256")),
        (
            Some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM"),
            Some("S256"),
        ),
    ] {
        assert!(
            start(REDIRECT, challenge, method).validate().is_none(),
            "accepted {challenge:?} {method:?}"
        );
    }
    let mut without_state = start(REDIRECT, Some(CHALLENGE), Some("S256"));
    without_state.state = None;
    assert!(without_state.validate().is_none());
    for redirect in [REDIRECT, "http://[::1]:1/kubecove/callback"] {
        assert!(
            start(redirect, Some(CHALLENGE), Some("S256"))
                .validate()
                .is_some()
        );
    }
}

#[tokio::test]
async fn browser_and_native_transactions_share_one_callback() {
    let (oidc, _provider) = mock::fixture("http://localhost:43160").await.unwrap();
    let (headers, callback) = pending_callback(&oidc).await;
    let browser = oidc.callback(&headers, callback).await.unwrap();
    assert_eq!(location(&browser), "/");
    assert!(
        browser
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|value| value.to_str().unwrap().starts_with(AUTH_COOKIE))
    );

    let (headers, callback, _) = native_callback(&oidc).await;
    let native = oidc.callback(&headers, callback).await.unwrap();
    assert!(location(&native).starts_with(REDIRECT));
    assert!(
        native
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .all(|value| !value
                .to_str()
                .unwrap()
                .starts_with(&format!("{AUTH_COOKIE}="))),
        "a native sign-in sets no browser credential"
    );
    let code = query(&location(&native))["code"].clone();
    let credential = exchange(&oidc, &code, VERIFIER, REDIRECT).await.unwrap();
    assert!(oidc.verify(&bearer(&credential)).is_ok());
}

#[tokio::test]
async fn code_is_single_use_and_replay_revokes_its_credential() {
    let (oidc, _provider) = mock::fixture("http://localhost:43161").await.unwrap();
    let code = native_code(&oidc).await;
    let credential = exchange(&oidc, &code, VERIFIER, REDIRECT).await.unwrap();
    let session = oidc.verify(&bearer(&credential)).unwrap();
    assert!(exchange(&oidc, &code, VERIFIER, REDIRECT).await.is_err());
    assert!(session.revoked.is_cancelled());
    assert!(oidc.verify(&bearer(&credential)).is_err());
}

#[tokio::test]
async fn replay_after_the_login_deadline_still_revokes_its_credential() {
    let (mut oidc, _provider) = mock::fixture("http://localhost:43165").await.unwrap();
    // A short deadline, so the test can pass it. Two seconds leaves the
    // exchange below at least one whole second before the deadline.
    oidc.config.login_timeout = std::time::Duration::from_secs(2);
    let started = unix_now();
    let code = native_code(&oidc).await;
    let credential = exchange(&oidc, &code, VERIFIER, REDIRECT).await.unwrap();
    let session = oidc.verify(&bearer(&credential)).unwrap();
    // The deadline is at most `started + 1 + 2`.
    while unix_now() < started + 3 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(exchange(&oidc, &code, VERIFIER, REDIRECT).await.is_err());
    assert!(session.revoked.is_cancelled());
    assert!(oidc.verify(&bearer(&credential)).is_err());
}

#[tokio::test]
async fn wrong_verifier_or_redirect_consumes_the_code() {
    let (oidc, _provider) = mock::fixture("http://localhost:43162").await.unwrap();
    let wrong_verifier = "x".repeat(43);
    let other_redirect = "http://127.0.0.1:50124/kubecove/callback";
    for (verifier, redirect) in [
        (wrong_verifier.as_str(), REDIRECT),
        (VERIFIER, other_redirect),
        ("short", REDIRECT),
    ] {
        let code = native_code(&oidc).await;
        assert!(exchange(&oidc, &code, verifier, redirect).await.is_err());
        assert!(
            exchange(&oidc, &code, VERIFIER, REDIRECT).await.is_err(),
            "a refused code cannot be retried"
        );
    }
    assert!(
        oidc.credentials.lock().unwrap().is_empty(),
        "refused codes leave no usable credential"
    );
}

#[tokio::test]
async fn expired_code_is_refused_and_its_credential_dropped() {
    let (oidc, _provider) = mock::fixture("http://localhost:43163").await.unwrap();
    let code = native_code(&oidc).await;
    let unexchanged = native_code(&oidc).await;
    for pending in oidc.codes.lock().unwrap().pending.values_mut() {
        pending.deadline = unix_now();
    }
    assert!(exchange(&oidc, &code, VERIFIER, REDIRECT).await.is_err());
    assert!(
        exchange(&oidc, &unexchanged, VERIFIER, REDIRECT)
            .await
            .is_err()
    );
    assert!(oidc.credentials.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_denial_reaches_the_loopback_listener() {
    let (oidc, _provider) = mock::fixture("http://localhost:43164").await.unwrap();
    let (headers, _, provider_state) = native_callback(&oidc).await;
    let denied = |state: &str| Callback {
        code: None,
        state: Some(state.to_owned()),
        error: Some("access_denied".into()),
    };
    let response = oidc
        .callback(&headers, denied(&provider_state))
        .await
        .unwrap();
    let loopback = location(&response);
    assert!(loopback.starts_with(REDIRECT));
    let parameters = query(&loopback);
    assert_eq!(parameters["error"], "access_denied");
    assert_eq!(parameters["state"], STATE);
    assert!(!parameters.contains_key("code"));
    // A denial that does not match the transaction goes to the app, as before.
    let response = oidc.callback(&headers, denied("other")).await.unwrap();
    assert_eq!(location(&response), "/");
    assert!(oidc.credentials.lock().unwrap().is_empty());
}
