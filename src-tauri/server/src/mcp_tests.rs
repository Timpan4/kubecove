//! `/mcp` on the server (ADR 0028).
//!
//! Failure modes: `/mcp` answers without `--mcp`; the MCP token opens `/api/*` or the
//! startup token opens `/mcp`; a foreign browser Origin reaches the tools; a refused
//! request goes unrecorded or a token reaches a log line.

use super::*;
use crate::observe::capture::Captured;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

fn data_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kubecove-server-mcp-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The token-mode test state with `--mcp`, and the MCP token.
fn mcp_state() -> (Arc<AppState>, String, std::path::PathBuf) {
    let dir = data_dir();
    let mut state = Arc::into_inner(api_tests::state()).unwrap();
    state.mcp = Some(
        mcp::McpServer::new(
            &dir,
            state.policy.origins.clone(),
            tokio_util::sync::CancellationToken::new(),
        )
        .unwrap(),
    );
    let token = kubecove_mcp::load_or_create_token(&dir).unwrap();
    (Arc::new(state), token, dir)
}

fn mcp_request(bearer: Option<&str>, origin: Option<&str>) -> Request<Body> {
    let mut request = Request::post("/mcp")
        .header(header::HOST, "localhost:1234")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    request
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}).to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn mcp_route_exists_only_with_the_flag() {
    // The startup token and the served Origin pass the guard, so only routing decides.
    let response = router(api_tests::state())
        .oneshot(mcp_request(
            Some("test-token"),
            Some("http://localhost:1234"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get(header::WWW_AUTHENTICATE), None);
}

#[tokio::test]
async fn mcp_and_startup_tokens_open_only_their_own_routes() {
    let logs = Captured::default();
    let _logging = logs.install();
    let (state, mcp_token, dir) = mcp_state();
    let app = router(state);

    let response = app
        .clone()
        .oneshot(mcp_request(Some(&mcp_token), None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        body["result"]["tools"].as_array().unwrap().len(),
        kubecove_mcp::TOOL_NAMES.len()
    );
    let response = app
        .clone()
        .oneshot(mcp_request(Some(&mcp_token), Some("http://localhost:1234")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "the served origin");

    for bearer in [None, Some("test-token"), Some("wrong-token")] {
        let response = app
            .clone()
            .oneshot(mcp_request(bearer, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{bearer:?}");
        assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");
    }
    let response = app
        .clone()
        .oneshot(mcp_request(
            Some(&mcp_token),
            Some("https://attacker.example"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let login = Request::post("/api/v1/session")
        .header(header::HOST, "localhost:1234")
        .header(header::ORIGIN, "http://localhost:1234")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"token": mcp_token}).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(login).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let invoke = Request::post("/api/v1/invoke/get_backend_diagnostics")
        .header(header::HOST, "localhost:1234")
        .header(header::ORIGIN, "http://localhost:1234")
        .header(header::AUTHORIZATION, format!("Bearer {mcp_token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.oneshot(invoke).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let refused = logs.named("request_refused");
    assert_eq!(
        refused
            .iter()
            .filter(|event| event["reason"] == "mcp_token")
            .count(),
        3
    );
    let text = logs.text();
    assert!(!text.contains(&mcp_token));
    assert!(!text.contains("wrong-token"));
    std::fs::remove_dir_all(dir).unwrap();
}
