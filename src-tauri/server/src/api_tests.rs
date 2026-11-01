use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test(start_paused = true)]
async fn authentication_wait_observes_expiry_without_a_revocation_token() {
    let identity = Extension(AccessIdentity {
        subject: "access-user".into(),
        expires_at: tokio::time::Instant::now() + Duration::from_secs(1),
    });
    let ended = authentication_ended(Some(&identity), None);
    tokio::pin!(ended);
    assert!(futures_util::poll!(&mut ended).is_pending());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(futures_util::poll!(&mut ended).is_ready());
}

pub(super) fn state() -> Arc<AppState> {
    Arc::new(AppState {
        token: "test-token".into(),
        policy: NetworkPolicy {
            hosts: vec!["localhost:1234".into()],
            origins: vec!["http://localhost:1234".into()],
            secure: false,
        },
        sessions: Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Token)),
        credential_store: "encryptedFile".into(),
        access: None,
        oidc: None,
    })
}

// Failure modes: the startup token, cookie or resume token reaches a log line;
// a lifecycle event is missing or repeated; a request path becomes a command
// value; a refused request is not recorded; a line is not one JSON object.
#[tokio::test]
async fn token_session_lifecycle_is_logged_without_credentials() {
    let logs = crate::observe::capture::Captured::default();
    let _logging = logs.install();
    let app = router(state());
    let wrong = post("/api/v1/session")
        .body(Body::from(json!({"token": "wrong-token"}).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(wrong).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let response = app
        .clone()
        .oneshot(
            post("/api/v1/session")
                .body(Body::from(json!({"token": "test-token"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let resume = value["resumeToken"].as_str().unwrap().to_owned();
    let resumed = post("/api/v1/session")
        .header(header::COOKIE, &cookie)
        .body(Body::from(json!({"resumeToken": resume}).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(resumed).await.unwrap().status(),
        StatusCode::OK
    );
    for command in ["get_backend_diagnostics", "attacker_chosen_command_name"] {
        let request = post(&format!("/api/v1/invoke/{command}"))
            .header(header::COOKIE, &cookie)
            .header("x-kubecove-session", &resume)
            .body(Body::from("{}"))
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
    }
    let unauthenticated = post("/api/v1/session/close").body(Body::empty()).unwrap();
    assert_eq!(
        app.clone().oneshot(unauthenticated).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let mut rebound = post("/api/v1/session").body(Body::from("{}")).unwrap();
    rebound
        .headers_mut()
        .insert(header::HOST, "evil.example".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(rebound).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let close = post("/api/v1/session/close")
        .header(header::COOKIE, &cookie)
        .header("x-kubecove-session", &resume)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close).await.unwrap().status(),
        StatusCode::OK
    );
    // A valid cookie with a closed session's resume token is no sign-in.
    let stale = post("/api/v1/session")
        .header(header::COOKIE, &cookie)
        .body(Body::from(json!({"resumeToken": resume}).to_string()))
        .unwrap();
    assert!(!app.oneshot(stale).await.unwrap().status().is_success());

    let text = logs.text();
    for (name, secret) in [
        ("startup token", "test-token"),
        ("rejected token", "wrong-token"),
        ("resume token", resume.as_str()),
        ("request path", "attacker_chosen_command_name"),
    ] {
        assert!(!text.contains(secret), "the {name} reached the logs");
    }
    let sign_ins = logs.named("sign_in");
    assert_eq!(
        sign_ins
            .iter()
            .map(|event| (event["outcome"].clone(), event["reason"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("failure"), json!("invalid_token")),
            (json!("success"), Value::Null),
            (json!("success"), Value::Null),
        ]
    );
    let opened = logs.named("session_opened");
    let resumed = logs.named("session_resumed");
    let closed = logs.named("session_closed");
    assert_eq!((opened.len(), resumed.len(), closed.len()), (1, 1, 1));
    let session = &opened[0]["session_id"];
    assert_eq!(opened[0]["auth_mode"], "token");
    assert!(opened[0].get("subject").is_none());
    assert_eq!(&resumed[0]["session_id"], session);
    assert_eq!(&closed[0]["session_id"], session);
    assert_eq!(closed[0]["reason"], "client");
    let commands = logs.named("command_completed");
    assert_eq!(
        commands
            .iter()
            .map(|event| (event["command"].clone(), event["outcome"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("get_backend_diagnostics"), json!("ok")),
            (json!("unknown"), json!("error")),
        ]
    );
    assert!(commands.iter().all(|event| &event["session_id"] == session));
    assert_eq!(commands[1]["error_kind"], "unsupportedOperation");
    let refused = logs.named("authentication_refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["reason"], "missing");
    let refused = logs.named("request_refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["reason"], "host");
    assert_eq!(refused[0]["host"], "evil.example");
}

// Failure mode: at the default level, client-supplied Host or Origin text
// reaches the logs of a refused request.
#[tokio::test]
async fn default_level_omits_refused_header_values() {
    use tracing::instrument::WithSubscriber;
    let logs = crate::observe::capture::Captured::default();
    let app = router(state());
    let mut bad_host = post("/api/v1/session").body(Body::from("{}")).unwrap();
    bad_host
        .headers_mut()
        .insert(header::HOST, "evil-host.example".parse().unwrap());
    let mut bad_origin = post("/api/v1/session").body(Body::from("{}")).unwrap();
    bad_origin.headers_mut().insert(
        header::ORIGIN,
        "http://evil-origin.example".parse().unwrap(),
    );
    for request in [bad_host, bad_origin] {
        let response = app
            .clone()
            .oneshot(request)
            .with_subscriber(logs.at_default_level())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let refused = logs.named("request_refused");
    assert_eq!(
        refused
            .iter()
            .map(|event| event["reason"].clone())
            .collect::<Vec<_>>(),
        [json!("host"), json!("origin")]
    );
    assert!(
        refused
            .iter()
            .all(|event| event.get("host").is_none() && event.get("origin").is_none())
    );
    assert!(!logs.text().contains("evil"));
}

// Failure modes: a metric label carries a subject, session identifier, resume
// token, cluster context or a request's command text.
#[tokio::test]
async fn metric_labels_never_carry_identities_or_cluster_names() {
    let state = access_state();
    let app = router(Arc::clone(&state));
    let subject = "label-probe-subject@example.com";
    let response = app
        .clone()
        .oneshot(access_post("/api/v1/session", subject, None, &json!({})))
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let resume = value["resumeToken"].as_str().unwrap().to_owned();
    let session_id = state
        .sessions
        .owner(&resume)
        .await
        .unwrap()
        .session_id
        .to_string();
    for (command, args) in [
        (
            "list_namespaces",
            json!({"clusterContext": "label-probe-context", "kubeconfigEnvVar": null}),
        ),
        ("label_probe_command", json!({})),
    ] {
        app.clone()
            .oneshot(access_post(
                &format!("/api/v1/invoke/{command}"),
                subject,
                Some(&resume),
                &args,
            ))
            .await
            .unwrap();
    }
    let text = crate::metrics::global().render(&state.sessions).await;
    assert!(
        text.contains("kubecove_server_command_duration_seconds_count{command=\"list_namespaces\"")
    );
    assert!(text.contains("kubecove_server_command_duration_seconds_count{command=\"unknown\""));
    for (name, identity) in [
        ("subject", subject),
        ("resume token", resume.as_str()),
        ("session id", session_id.as_str()),
        ("cluster context", "label-probe-context"),
        ("request path", "label_probe_command"),
    ] {
        assert!(!text.contains(identity), "the {name} reached a metric");
    }
}

// API-only builds must retain authentication and Host checks while removing UI routes.
#[cfg(not(feature = "web-ui"))]
#[tokio::test]
async fn api_only_has_no_assets_and_preserves_request_guards() {
    let app = router(state());
    for path in ["/", "/index.html", "/assets/missing.js"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::HOST, "localhost:1234")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth")
                .header(header::HOST, "localhost:1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(post("/api/v1/session").body(Body::from("{}")).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

fn access_state() -> Arc<AppState> {
    let mut state = state();
    Arc::get_mut(&mut state).unwrap().access = Some(Arc::new(crate::access::tests::access()));
    state
}

fn access_post(path: &str, subject: &str, resume: Option<&str>, body: &Value) -> Request<Body> {
    let mut request = post(path).header(
        crate::access::ASSERTION_HEADER,
        crate::access::tests::token(subject),
    );
    if let Some(resume) = resume {
        request = request.header("x-kubecove-session", resume);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn http2_authority_stands_in_for_the_host_header() {
    for (uri, status) in [
        ("https://localhost:1234/api/v1/auth", StatusCode::OK),
        (
            "https://evil.example:1234/api/v1/auth",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = router(state())
            .oneshot(
                Request::builder()
                    .version(axum::http::Version::HTTP_2)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{uri}");
    }
}

#[tokio::test]
async fn access_login_needs_no_token_and_never_creates_a_token_cookie() {
    let app = router(access_state());
    let response = app
        .clone()
        .oneshot(access_post("/api/v1/session", "first", None, &json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let resume = value["resumeToken"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(access_post(
            "/api/v1/invoke/get_backend_diagnostics",
            "first",
            Some(resume),
            &json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for path in [
        "/api/v1/session",
        "/api/v1/invoke/get_backend_diagnostics",
        "/api/v1/channels/42/close",
        "/api/v1/session/close",
    ] {
        let body = if path == "/api/v1/session" {
            json!({"resumeToken": resume})
        } else {
            json!({})
        };
        let response = app
            .clone()
            .oneshot(access_post(path, "other", Some(resume), &body))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CONFLICT,
            "cross-subject {path}"
        );
    }
    let response = app
        .clone()
        .oneshot(access_post(
            "/api/v1/session",
            "first",
            None,
            &json!({"resumeToken": resume}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(access_post(
            "/api/v1/session/close",
            "first",
            Some(resume),
            &json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn access_mode_has_no_token_or_cookie_fallback_and_keeps_origin_checks() {
    let app = router(access_state());
    for credential in [
        (header::AUTHORIZATION, "Bearer test-token"),
        (header::COOKIE, "kubecove_session=test-token"),
    ] {
        let response = app
            .clone()
            .oneshot(
                post("/api/v1/session")
                    .header(credential.0, credential.1)
                    .header("cf-access-authenticated-user-email", "email@timpan.me")
                    .body(Body::from("{\"token\":\"test-token\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    for origin in [None, Some("https://evil.example")] {
        let mut request = access_post("/api/v1/session", "first", None, &json!({}));
        request.headers_mut().remove(header::ORIGIN);
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert(header::ORIGIN, origin.parse().unwrap());
        }
        request
            .headers_mut()
            .insert(header::AUTHORIZATION, "Bearer test-token".parse().unwrap());
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    let mut duplicate = access_post("/api/v1/session", "first", None, &json!({}));
    duplicate.headers_mut().append(
        crate::access::ASSERTION_HEADER,
        crate::access::tests::token("first").parse().unwrap(),
    );
    assert_eq!(
        app.clone().oneshot(duplicate).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "localhost:1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth")
                .header(header::HOST, "localhost:1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mode: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(mode, json!({"mode":"cloudflareAccess"}));
}

fn post(path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, "localhost:1234")
        .header(header::ORIGIN, "http://localhost:1234")
        .header(header::CONTENT_TYPE, "application/json")
}

#[tokio::test]
async fn http_boundary_rejects_wrong_token_origin_host_and_oversized_bodies() {
    let state = state();
    let app = router(Arc::clone(&state));
    for token in [Value::Null, json!("wrong")] {
        let request = post("/api/v1/session")
            .body(Body::from(json!({"token": token}).to_string()))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }
    for (header, value) in [
        (header::HOST, "evil.example"),
        (header::ORIGIN, "http://evil.example"),
    ] {
        let mut request = post("/api/v1/session")
            .body(Body::from("{\"token\":\"test-token\"}"))
            .unwrap();
        request.headers_mut().insert(header, value.parse().unwrap());
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    let request = post("/api/v1/session")
        .body(Body::from(" ".repeat(2 * 1024 * 1024 + 1)))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(error["kind"], "validation");
}

#[tokio::test]
async fn bearer_requests_keep_the_typed_error_and_command_result_contract() {
    let state = state();
    let info = state.sessions.authenticate_as(None, None).await.unwrap();
    let app = router(Arc::clone(&state));
    for (command, args, expected) in [
        ("get_backend_diagnostics", json!({}), StatusCode::OK),
        ("pick_kubeconfig_paths", json!({}), StatusCode::NOT_FOUND),
        (
            "stop_port_forward",
            json!({"sessionId": 42}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/v1/invoke/{command}"))
            .header(header::HOST, "localhost:1234")
            .header(header::AUTHORIZATION, "Bearer test-token")
            .header("x-kubecove-session", &info.resume_token)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(args.to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        if expected == StatusCode::OK {
            assert!(value.is_array());
        } else {
            assert!(value["message"].is_string());
            assert!(value["kind"].is_string());
        }
    }
}

#[test]
fn delivered_html_and_header_allow_only_the_exact_served_websocket_origin() {
    let policy = state().policy.content_security_policy("localhost:1234");
    assert!(policy.contains("connect-src 'self' ws://localhost:1234;"));
    assert!(!policy.contains("connect-src 'self' ws: "));
    let html = index_with_policy(include_str!("../../../index.html"), &policy).unwrap();
    assert!(html.contains(&format!("content=\"{policy}\"")));
    assert!(!html.contains("ws://localhost:1430"));
    assert_eq!(html.matches("Content-Security-Policy").count(), 1);
}

#[tokio::test]
async fn access_websocket_closes_at_jwt_expiry() {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let mut state = access_state();
    Arc::get_mut(&mut state).unwrap().policy.hosts = vec![address.to_string()];
    Arc::get_mut(&mut state).unwrap().policy.origins = vec![origin.clone()];
    let session = state
        .sessions
        .authenticate_as(None, Some("first"))
        .await
        .unwrap();
    let app = router(Arc::clone(&state));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!(
        "{origin}/api/v1/channels?resumeToken={}",
        session.resume_token
    );
    let client = reqwest::Client::new();
    let upgrade = |subject: &str| {
        client
            .get(&url)
            .header(header::ORIGIN, &origin)
            .header(
                crate::access::ASSERTION_HEADER,
                crate::access::tests::token(subject),
            )
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
    };
    assert_eq!(
        upgrade("other").send().await.unwrap().status(),
        StatusCode::CONFLICT
    );
    let response = upgrade("first").send().await.unwrap();
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    let mut socket = response.upgrade().await.unwrap();
    tokio::time::pause();
    // Advance by the fixture JWT's encoded lifetime, without waiting in real time.
    tokio::time::advance(Duration::from_secs(3600)).await;
    let mut frame = [0_u8; 2];
    let bytes = socket.read(&mut frame).await.unwrap();
    assert!(bytes == 0 || frame[0] == 0x88, "expired socket must close");
    state.sessions.shutdown().await;
    server.abort();
    let _ = server.await;
}

// Test-only Access proxy for repeatable browser E2E. Production builds cannot expose it.
#[tokio::test]
#[ignore = "manual browser fixture, stopped through /__test__/finish"]
async fn access_browser_fixture() {
    let data_dir = std::env::var_os("KUBECOVE_ACCESS_FIXTURE_DATA")
        .map(std::path::PathBuf::from)
        .expect("browser fixture requires an isolated data directory");
    kubecove_lib::commands::init_kubeconfig_settings_path(data_dir.clone());
    let credential_store = kubecove_lib::commands::initialize_server_credential_store(data_dir)
        .unwrap()
        .kind;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let mut state = access_state();
    Arc::get_mut(&mut state).unwrap().credential_store = credential_store;
    Arc::get_mut(&mut state).unwrap().policy.hosts = vec![address.to_string()];
    Arc::get_mut(&mut state).unwrap().policy.origins = vec![origin.clone()];
    let finished = Arc::new(tokio::sync::Notify::new());
    let signal = Arc::clone(&finished);
    let assertion = crate::access::tests::token("browser-user");
    let app = router(Arc::clone(&state))
        .route(
            "/__test__/finish",
            axum::routing::post(move || {
                signal.notify_one();
                async { StatusCode::OK }
            }),
        )
        .layer(middleware::from_fn(
            move |mut request: axum::extract::Request, next: Next| {
                request
                    .headers_mut()
                    .insert(crate::access::ASSERTION_HEADER, assertion.parse().unwrap());
                async move { next.run(request).await }
            },
        ));
    println!("Access browser fixture: {origin}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! { () = finished.notified() => {}, _ = tokio::signal::ctrl_c() => {} }
        })
        .await
        .unwrap();
    state.sessions.shutdown().await;
}
