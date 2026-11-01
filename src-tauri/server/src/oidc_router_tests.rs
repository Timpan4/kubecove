use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use std::time::{Duration, SystemTime};
use tower::ServiceExt;

fn oidc_state() -> Arc<AppState> {
    let config = oidc::OidcConfig::new(
        "https://identity.example".into(),
        "kubecove".into(),
        None,
        Duration::from_mins(5),
    )
    .unwrap();
    Arc::new(AppState {
        token: "must-not-authenticate".into(),
        policy: NetworkPolicy {
            hosts: vec!["app.example".into()],
            origins: vec!["https://app.example".into()],
            secure: true,
        },
        sessions: Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Oidc)),
        credential_store: "encryptedFile".into(),
        access: None,
        oidc: Some(Arc::new(
            oidc::Oidc::new_for_test(config, "https://app.example").unwrap(),
        )),
    })
}
fn credential(state: &AppState, subject: &str, expired: bool) -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cookie = state.oidc.as_ref().unwrap().insert_identity_for_test(
        "https://identity.example",
        subject,
        if expired { now } else { now + 300 },
    );
    format!("{}={cookie}", oidc::AUTH_COOKIE)
}

fn post(path: &str, cookie: &str, resume: Option<&str>, origin: &str) -> Request<Body> {
    let mut request = Request::post(path)
        .header(header::HOST, "app.example")
        .header(header::ORIGIN, origin)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(resume) = resume {
        request = request.header("x-kubecove-session", resume);
    }
    request.body(Body::from("{}")).unwrap()
}

async fn session(app: &Router, cookie: &str) -> String {
    let response = app
        .clone()
        .oneshot(post("/api/v1/session", cookie, None, "https://app.example"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    body["resumeToken"].as_str().unwrap().into()
}

#[tokio::test]
async fn oidc_denies_startup_credentials_and_expired_cookies() {
    let state = oidc_state();
    let expired = credential(&state, "expired", true);
    let app = router(state);
    for cookie in ["", "kubecove_session=must-not-authenticate", &expired] {
        let response = app
            .clone()
            .oneshot(post("/api/v1/session", cookie, None, "https://app.example"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app
        .oneshot(
            Request::post("/api/v1/session")
                .header(header::HOST, "app.example")
                .header(header::ORIGIN, "https://app.example")
                .header(header::AUTHORIZATION, "Bearer must-not-authenticate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"token\":\"must-not-authenticate\"}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn oidc_metadata_reports_cookie_authentication_without_claims() {
    let state = oidc_state();
    let cookie = credential(&state, "subject", false);
    let app = router(state);
    for (cookie, authenticated) in [("", false), (cookie.as_str(), true)] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v1/auth")
                    .header(header::HOST, "app.example")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(
            body,
            serde_json::json!({"mode":"oidc", "authenticated":authenticated})
        );
    }
}

#[tokio::test]
async fn oidc_owner_is_identity_bound_and_logout_revokes_only_matching_owner() {
    let state = oidc_state();
    let first = credential(&state, "first", false);
    let second = credential(&state, "second", false);
    let same_subject = credential(&state, "first", false);
    let app = router(Arc::clone(&state));
    let resume = session(&app, &first).await;
    let owner = state.sessions.owner(&resume).await.unwrap();
    let auth = state
        .oidc
        .as_ref()
        .unwrap()
        .verify(
            &post("/", &first, None, "https://app.example")
                .into_parts()
                .0
                .headers,
        )
        .unwrap();
    for path in [
        "/api/v1/invoke/get_backend_diagnostics",
        "/api/v1/channels/1/close",
        "/api/v1/session/close",
    ] {
        let response = app
            .clone()
            .oneshot(post(path, &second, Some(&resume), "https://app.example"))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CONFLICT,
            "cross-identity {path}"
        );
    }
    // Another browser for the same subject lacks this owner's resume credential.
    let response = app
        .clone()
        .oneshot(post(
            "/api/v1/oidc/logout",
            &same_subject,
            None,
            "https://app.example",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!owner.ended.is_cancelled());
    assert!(!auth.revoked.is_cancelled());
    let response = app
        .clone()
        .oneshot(post(
            "/api/v1/oidc/logout",
            &first,
            Some(&resume),
            "https://app.example",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|cookie| cookie.to_str().unwrap().contains("Max-Age=0")));
    assert!(owner.ended.is_cancelled());
    assert!(auth.revoked.is_cancelled());
    let response = app
        .oneshot(post("/api/v1/session", &first, None, "https://app.example"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn oidc_keeps_host_and_origin_checks_on_login_and_logout() {
    let state = oidc_state();
    let cookie = credential(&state, "subject", false);
    let app = router(state);
    for path in [
        "/api/v1/oidc/login",
        "/api/v1/oidc/logout",
        "/api/v1/session",
    ] {
        let response = app
            .clone()
            .oneshot(post(path, &cookie, None, "https://attacker.example"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let mut request = post(path, &cookie, None, "https://app.example");
        request.headers_mut().remove(header::ORIGIN);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = app
        .oneshot(
            Request::get("/api/v1/auth")
                .header(header::HOST, "attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn oidc_websocket_closes_on_expiration_and_credential_revocation() {
    use tokio::io::AsyncReadExt;
    for logout in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let origin = format!("http://{address}");
        let mut state = oidc_state();
        Arc::get_mut(&mut state).unwrap().policy.hosts = vec![address.to_string()];
        Arc::get_mut(&mut state).unwrap().policy.origins = vec![origin.clone()];
        let cookie = credential(&state, "subject", false);
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, cookie.parse().unwrap());
        let auth = state.oidc.as_ref().unwrap().verify(&headers).unwrap();
        let session = state
            .sessions
            .authenticate_as(None, Some(&auth.identity.subject))
            .await
            .unwrap();
        let server_app = router(Arc::clone(&state));
        let server = tokio::spawn(async move { axum::serve(listener, server_app).await.unwrap() });
        let url = format!(
            "{origin}/api/v1/channels?resumeToken={}",
            session.resume_token
        );
        let response = reqwest::Client::new()
            .get(&url)
            .header(header::ORIGIN, &origin)
            .header(header::COOKIE, &cookie)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        let mut socket = response.upgrade().await.unwrap();
        if logout {
            state.oidc.as_ref().unwrap().logout(&headers).unwrap();
        } else {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(300)).await;
        }
        let mut frame = [0_u8; 2];
        let bytes = socket.read(&mut frame).await.unwrap();
        assert!(
            bytes == 0 || frame[0] == 0x88,
            "authentication loss must close socket"
        );
        state.sessions.shutdown().await;
        server.abort();
        let _ = server.await;
        if !logout {
            tokio::time::resume();
        }
    }
}

// Only the test binary can serve this local provider and browser fixture.
#[tokio::test]
#[ignore = "manual browser verification, stopped through /__test__/finish"]
async fn oidc_browser_fixture() {
    let data_dir = std::env::var_os("KUBECOVE_OIDC_FIXTURE_DATA")
        .map(std::path::PathBuf::from)
        .expect("browser fixture requires an isolated data directory");
    kubecove_lib::commands::init_kubeconfig_settings_path(data_dir.clone());
    let credential_store = kubecove_lib::commands::initialize_server_credential_store(data_dir)
        .unwrap()
        .kind;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://localhost:{}", address.port());
    let (oidc, _provider) = oidc::mock::fixture(&origin).await.unwrap();
    let state = Arc::new(AppState {
        token: String::new(),
        access: None,
        oidc: Some(Arc::new(oidc)),
        policy: NetworkPolicy {
            hosts: vec![format!("localhost:{}", address.port())],
            origins: vec![origin.clone()],
            secure: false,
        },
        sessions: Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Oidc)),
        credential_store,
    });
    let finished = Arc::new(tokio::sync::Notify::new());
    let signal = Arc::clone(&finished);
    let app = router(Arc::clone(&state)).route(
        "/__test__/finish",
        axum::routing::post(move || {
            signal.notify_one();
            async { StatusCode::OK }
        }),
    );
    println!("OIDC browser fixture: {origin}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! { () = finished.notified() => {}, _ = tokio::signal::ctrl_c() => {} }
        })
        .await
        .unwrap();
    state.sessions.shutdown().await;
}
