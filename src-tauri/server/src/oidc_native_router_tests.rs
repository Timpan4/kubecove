// Failure modes (ADR 0031): a cookie request that skips the Origin check, a
// request that mixes the cookie with a bearer, a bearer that cannot log out
// or resumes another identity's session, native endpoints held behind the
// sign-in guard, and a bearer session whose Kubernetes requests do not carry
// the signed-in user's ID token.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use tower::ServiceExt;

fn state_with(oidc: oidc::Oidc, host: &str, origin: &str) -> Arc<AppState> {
    Arc::new(AppState {
        token: "must-not-authenticate".into(),
        policy: NetworkPolicy {
            hosts: vec![host.into()],
            origins: vec![origin.into()],
            secure: origin.starts_with("https://"),
        },
        sessions: Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Oidc)),
        credential_store: "encryptedFile".into(),
        access: None,
        oidc: Some(Arc::new(oidc)),
        mcp: None,
    })
}

fn oidc_state() -> Arc<AppState> {
    let config = oidc::OidcConfig::new(
        "https://identity.example".into(),
        "kubecove".into(),
        None,
        Duration::from_mins(5),
    )
    .unwrap();
    state_with(
        oidc::Oidc::new_for_test(config, "https://app.example").unwrap(),
        "app.example",
        "https://app.example",
    )
}

fn sign_in(state: &AppState, subject: &str) -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state.oidc.as_ref().unwrap().insert_identity_for_test(
        "https://identity.example",
        subject,
        now + 300,
    )
}

/// A desktop request: a bearer, no cookie and no Origin.
fn desktop(path: &str, credential: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header(header::HOST, "app.example")
        .header(header::AUTHORIZATION, format!("Bearer {credential}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn json(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn bearer_needs_no_origin_and_cannot_mix_with_the_cookie() {
    let state = oidc_state();
    let credential = sign_in(&state, "subject");
    let app = router(state);
    let response = app
        .clone()
        .oneshot(desktop("/api/v1/session", &credential, "{}"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::SET_COOKIE));

    let cookie = format!("{}={credential}", oidc::AUTH_COOKIE);
    let mut mixed = desktop("/api/v1/session", &credential, "{}");
    mixed
        .headers_mut()
        .insert(header::COOKIE, cookie.parse().unwrap());
    let response = app.clone().oneshot(mixed).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "cookie needs Origin"
    );

    let mut mixed = desktop("/api/v1/session", &credential, "{}");
    mixed
        .headers_mut()
        .insert(header::COOKIE, cookie.parse().unwrap());
    mixed
        .headers_mut()
        .insert(header::ORIGIN, "https://app.example".parse().unwrap());
    let response = app.clone().oneshot(mixed).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "two sign-ins");

    let mut foreign = desktop("/api/v1/session", &credential, "{}");
    foreign
        .headers_mut()
        .insert(header::ORIGIN, "https://attacker.example".parse().unwrap());
    let response = app.oneshot(foreign).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn auth_reports_native_sign_in_and_bearer_authentication() {
    let state = oidc_state();
    let credential = sign_in(&state, "subject");
    let app = router(state);
    for (authorization, authenticated) in [
        (format!("Bearer {credential}"), true),
        ("Bearer unknown".to_owned(), false),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v1/auth")
                    .header(header::HOST, "app.example")
                    .header(header::AUTHORIZATION, authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            json(response).await,
            serde_json::json!({"mode":"oidc", "authenticated":authenticated, "nativeSignIn":true})
        );
    }
}

#[tokio::test]
async fn bearer_logout_revokes_and_other_identities_cannot_resume() {
    let state = oidc_state();
    let first = sign_in(&state, "first");
    let second = sign_in(&state, "second");
    let app = router(Arc::clone(&state));
    let response = app
        .clone()
        .oneshot(desktop("/api/v1/session", &first, "{}"))
        .await
        .unwrap();
    let resume = json(response).await["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let owner = state.sessions.owner(&resume).await.unwrap();

    let body = format!("{{\"resumeToken\":\"{resume}\"}}");
    let response = app
        .clone()
        .oneshot(desktop("/api/v1/session", &second, &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(!owner.ended.is_cancelled());

    let mut logout = desktop("/api/v1/oidc/logout", &first, "{}");
    logout
        .headers_mut()
        .insert("x-kubecove-session", resume.parse().unwrap());
    let response = app.clone().oneshot(logout).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(owner.ended.is_cancelled());
    let response = app
        .oneshot(desktop("/api/v1/session", &first, "{}"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn native_endpoints_are_open_but_keep_the_origin_check() {
    let state = oidc_state();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::get(
                "/api/v1/oidc/native/start?redirect_uri=http://localhost:1/kubecove/callback",
            )
            .header(header::HOST, "app.example")
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    let exchange = |origin: Option<&str>, cookie: Option<&str>| {
        let mut request = Request::post("/api/v1/oidc/native/token")
            .header(header::HOST, "app.example")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request
            .body(Body::from(
                r#"{"code":"unknown","codeVerifier":"v","redirectUri":"r"}"#,
            ))
            .unwrap()
    };
    for (origin, cookie, status) in [
        (None, None, StatusCode::BAD_REQUEST),
        (Some("https://app.example"), None, StatusCode::BAD_REQUEST),
        (
            Some("https://attacker.example"),
            None,
            StatusCode::FORBIDDEN,
        ),
        (None, Some("__Host-kubecove_oidc=x"), StatusCode::FORBIDDEN),
    ] {
        let response = app.clone().oneshot(exchange(origin, cookie)).await.unwrap();
        assert_eq!(response.status(), status, "{origin:?} {cookie:?}");
    }
}

fn query(location: &str) -> HashMap<String, String> {
    openidconnect::url::Url::parse(location)
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

/// The whole desktop flow through the router, with forwarded credentials.
#[tokio::test]
async fn desktop_sign_in_opens_a_session_that_forwards_the_users_token() {
    const ORIGIN: &str = "http://localhost:43170";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    const STATE: &str = "desktop-state-desktop-state-desktop-state-x";
    const REDIRECT: &str = "http://[::1]:50125/kubecove/callback";
    let (oidc, _provider) = oidc::mock::fixture_with(ORIGIN, Some("kubernetes"))
        .await
        .unwrap();
    let state = state_with(oidc, "localhost:43170", ORIGIN);
    let app = router(Arc::clone(&state));
    let get = |uri: String, cookie: Option<String>| {
        let mut request = Request::get(uri).header(header::HOST, "localhost:43170");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.body(Body::empty()).unwrap()
    };
    let mut start =
        openidconnect::url::Url::parse(&format!("{ORIGIN}/api/v1/oidc/native/start")).unwrap();
    start
        .query_pairs_mut()
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("code_challenge", CHALLENGE)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", STATE);
    let path = format!("{}?{}", start.path(), start.query().unwrap());
    let response = app.clone().oneshot(get(path, None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let authorization = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(location(&response))
        .send()
        .await
        .unwrap();
    let callback =
        openidconnect::url::Url::parse(authorization.headers()[header::LOCATION].to_str().unwrap())
            .unwrap();
    let path = format!("{}?{}", callback.path(), callback.query().unwrap());
    let response = app.clone().oneshot(get(path, Some(cookie))).await.unwrap();
    let loopback = location(&response);
    assert!(loopback.starts_with(REDIRECT));
    let parameters = query(&loopback);
    assert_eq!(parameters["state"], STATE);

    let body = serde_json::json!({
        "code": parameters["code"], "codeVerifier": VERIFIER, "redirectUri": REDIRECT,
    });
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/oidc/native/token")
                .header(header::HOST, "localhost:43170")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let credential = json(response).await["credential"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut session = desktop("/api/v1/session", &credential, "{}");
    session
        .headers_mut()
        .insert(header::HOST, "localhost:43170".parse().unwrap());
    let response = app.oneshot(session).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let resume = json(response).await["resumeToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let owner = state.sessions.owner(&resume).await.unwrap();
    assert!(
        owner.backend.scope.bearer().is_some(),
        "the bearer session carries the user's ID token"
    );
    state.sessions.shutdown().await;
}
