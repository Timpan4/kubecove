use super::tests::pending_callback;
use super::*;
use crate::observe::{capture::Captured, CloseReason};
use axum::{
    extract::State,
    http::{header, HeaderMap},
    Extension,
};
use std::time::Duration;

// Failure modes: a sign-in, callback, refresh or session event that carries the
// transaction cookie, code, state, authentication cookie, resume token, ID
// token, refresh token or provider error text.
#[tokio::test]
async fn sign_in_events_carry_no_credentials() {
    let logs = Captured::default();
    let _logging = logs.install();
    let (oidc, provider) = mock::fixture_with("http://localhost:43135", Some("kubernetes"))
        .await
        .unwrap();
    // The login token expires inside the refresh margin, so activation renews it.
    provider.set_claim_mode(mock::ClaimMode::ShortLived);
    let (callback_headers, callback) = pending_callback(&oidc).await;
    let transaction = callback_headers[header::COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    let code = callback.code.clone().unwrap();
    let state_parameter = callback.state.clone().unwrap();
    let response = oidc.callback(&callback_headers, callback).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let auth_cookie = response
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
    oidc.callback(
        &HeaderMap::new(),
        Callback {
            code: None,
            state: None,
            error: Some("provider-secret-detail".into()),
        },
    )
    .await
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, auth_cookie.parse().unwrap());
    let auth = oidc.verify(&headers).unwrap();
    let oidc = Arc::new(oidc);
    let state = Arc::new(crate::AppState {
        token: String::new(),
        policy: crate::security::NetworkPolicy {
            hosts: vec![],
            origins: vec![],
            secure: true,
        },
        sessions: Arc::new(crate::sessions::Sessions::new(
            Duration::from_mins(5),
            AuthMode::Oidc,
        )),
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
    let resume = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let (id_token, refresh_token) = {
        let credentials = oidc.credentials.lock().unwrap();
        let tokens = credentials[&auth.credential.0].tokens.clone().unwrap();
        (tokens.id_token, tokens.refresh_token.unwrap())
    };
    state
        .sessions
        .close(&resume, CloseReason::Logout)
        .await
        .unwrap();

    let text = logs.text();
    for secret in [
        transaction.as_str(),
        transaction.split_once('=').unwrap().1,
        &code,
        &state_parameter,
        auth_cookie.split_once('=').unwrap().1,
        &auth.credential.0,
        &resume,
        &id_token,
        &refresh_token,
        "provider-secret-detail",
    ] {
        assert!(!text.contains(secret), "a credential reached the logs");
    }
    // Every JWT, including the ID token issued before the refresh, starts so.
    assert!(!text.contains("eyJ"));

    let sign_ins = logs.named("sign_in");
    assert_eq!(sign_ins.len(), 2);
    assert_eq!(sign_ins[0]["outcome"], "success");
    assert!(sign_ins[0]["subject"]
        .as_str()
        .unwrap()
        .ends_with("mock-user"));
    assert_eq!(sign_ins[1]["outcome"], "failure");
    assert_eq!(sign_ins[1]["reason"], "provider_denied");
    let opened = logs.named("session_opened");
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0]["auth_mode"], "oidc");
    let closed = logs.named("session_closed");
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0]["reason"], "logout");
    assert_eq!(closed[0]["session_id"], opened[0]["session_id"]);
    assert!(logs
        .named("oidc_refresh")
        .iter()
        .any(|event| event["outcome"] == "renewed"));
}
