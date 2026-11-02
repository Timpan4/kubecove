//! Boundary tests for the MCP endpoint (ADR 0028 verification list).
//!
//! Failure modes:
//! - A request without the token, with a wrong token, with another scheme, or with two
//!   Authorization headers reaches the tools.
//! - A web page reaches the desktop endpoint by DNS rebinding (a foreign Host) or a
//!   cross-origin request (any Origin).
//! - The tool list holds a tool beyond `TOOL_NAMES` or one not annotated read-only.
//! - `describe_resource` or `get_resource_yaml` returns Secret values, including through
//!   kubectl's last-applied annotation.
//! - A tool asks for a client other than a Read client.
//! - Paging skips or repeats rows, or reports a next page after the last one.
//! - The token file is readable by other users, changes across restarts, or survives
//!   rotation.

use crate::{
    EndpointPolicy, McpContext, McpEndpoint, TOOL_NAMES, compact::page, load_or_create_token,
    rotate_token,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use http::{Request, Response, StatusCode, header};
use http_body_util::{BodyExt as _, Full};
use kube::{Client, client::Body};
use kubecove_kube::commands::{ClientIntent, ClusterLiveStore, FixedClusterAccess};
use serde_json::{Value, json};
use std::{
    convert::Infallible,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

const TOKEN: &str = "test-token";
const SECRET_VALUE: &str = "hunter2-database-password";

fn endpoint(cluster: &Arc<FixedClusterAccess>) -> McpEndpoint {
    let context = McpContext::new(ClusterLiveStore::default(), Some(cluster.scope()));
    McpEndpoint::new(
        Arc::new(context),
        TOKEN.to_owned(),
        EndpointPolicy::loopback(),
        CancellationToken::new(),
    )
}

/// A cluster that serves one Secret, carrying its value in `data` and in kubectl's
/// last-applied annotation, and no events.
fn cluster() -> Arc<FixedClusterAccess> {
    let encoded = STANDARD.encode(SECRET_VALUE);
    let last_applied =
        json!({"apiVersion": "v1", "kind": "Secret", "stringData": {"password": SECRET_VALUE}})
            .to_string();
    let secret = json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": "db",
            "namespace": "apps",
            "uid": "secret-uid",
            "annotations": {"kubectl.kubernetes.io/last-applied-configuration": last_applied},
        },
        "type": "Opaque",
        "data": {"password": encoded},
    });
    let service = tower::service_fn(move |request: Request<Body>| {
        let (status, body) = match request.uri().path() {
            "/api/v1/namespaces/apps/secrets/db" => (StatusCode::OK, secret.clone()),
            "/api/v1/namespaces/apps/events" => (
                StatusCode::OK,
                json!({"apiVersion": "v1", "kind": "EventList", "metadata": {}, "items": []}),
            ),
            _ => (
                StatusCode::NOT_FOUND,
                json!({"apiVersion": "v1", "kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404}),
            ),
        };
        let mut response = Response::new(Body::from(body.to_string().into_bytes()));
        *response.status_mut() = status;
        async move { Ok::<_, Infallible>(response) }
    });
    FixedClusterAccess::new(Client::new(service, "default"))
}

fn request(headers: &[(&str, &str)], body: &Value) -> Request<Full<Bytes>> {
    let mut builder = Request::post("/mcp")
        .header(header::HOST, "127.0.0.1:47615")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

const AUTHORIZED: (&str, &str) = ("authorization", "Bearer test-token");

async fn send(endpoint: &McpEndpoint, request: Request<Full<Bytes>>) -> (StatusCode, Bytes) {
    let response = endpoint.handle(request).await;
    let status = response.status();
    (
        status,
        response.into_body().collect().await.unwrap().to_bytes(),
    )
}

fn list_tools() -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}})
}

/// Calls a tool and returns its decoded JSON result, failing on a tool error.
async fn call(endpoint: &McpEndpoint, name: &str, arguments: Value) -> (String, Value) {
    let body = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    });
    let (status, bytes) = send(endpoint, request(&[AUTHORIZED], &body)).await;
    assert_eq!(status, StatusCode::OK);
    let message: Value = serde_json::from_slice(&bytes).unwrap();
    let result = &message["result"];
    let text = result["content"][0]["text"].as_str().unwrap().to_owned();
    assert_ne!(result["isError"], true, "{name} failed: {text}");
    (text.clone(), serde_json::from_str(&text).unwrap())
}

#[tokio::test]
async fn requests_without_the_bearer_token_never_reach_the_tools() {
    let endpoint = endpoint(&cluster());
    let refused: [&[(&str, &str)]; 5] = [
        &[],
        &[("authorization", "Bearer wrong-token")],
        &[("authorization", "Basic dGVzdC10b2tlbg==")],
        &[("authorization", "bearer test-token")],
        &[AUTHORIZED, AUTHORIZED],
    ];
    for headers in refused {
        let (status, _) = send(&endpoint, request(headers, &list_tools())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{headers:?}");
    }
    let (status, _) = send(&endpoint, request(&[AUTHORIZED], &list_tools())).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn desktop_endpoint_refuses_foreign_hosts_and_every_browser_origin() {
    let endpoint = endpoint(&cluster());
    for headers in [
        [AUTHORIZED, ("host", "attacker.example:47615")],
        [AUTHORIZED, ("origin", "http://127.0.0.1:47615")],
        [AUTHORIZED, ("origin", "https://attacker.example")],
    ] {
        let mut request = request(&headers[..1], &list_tools());
        let (name, value) = headers[1];
        request.headers_mut().insert(name, value.parse().unwrap());
        let (status, _) = send(&endpoint, request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{name}: {value}");
    }
}

#[tokio::test]
async fn tool_list_is_exactly_the_read_only_tools() {
    let (status, bytes) = send(&endpoint(&cluster()), request(&[AUTHORIZED], &list_tools())).await;
    assert_eq!(status, StatusCode::OK);
    let message: Value = serde_json::from_slice(&bytes).unwrap();
    let tools = message["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, TOOL_NAMES);
    for tool in tools {
        assert_eq!(
            tool["annotations"]["readOnlyHint"], true,
            "{}",
            tool["name"]
        );
        assert_eq!(
            tool["annotations"]["destructiveHint"], false,
            "{}",
            tool["name"]
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
    }
}

#[tokio::test]
async fn secret_values_never_leave_through_describe_or_yaml() {
    let cluster = cluster();
    let endpoint = endpoint(&cluster);
    let arguments =
        json!({"context": "fixed", "kind": "Secret", "namespace": "apps", "name": "db"});
    let (described, body) = call(&endpoint, "describe_resource", arguments.clone()).await;
    assert_eq!(body["summary"]["name"], "db");
    assert!(
        body["data"]["password"].is_string(),
        "the key stays visible"
    );
    let (yaml, _) = call(&endpoint, "get_resource_yaml", arguments).await;
    for text in [&described, &yaml] {
        assert!(!text.contains(SECRET_VALUE));
        assert!(!text.contains(&STANDARD.encode(SECRET_VALUE)));
    }
    let intents = cluster.intents();
    assert_ne!(intents, Vec::new());
    assert!(
        intents.iter().all(|intent| *intent == ClientIntent::Read),
        "{intents:?}"
    );
}

#[test]
fn pages_cover_every_row_once_and_end_without_a_next_offset() {
    let rows: Vec<usize> = (0..7).collect();
    let mut seen = Vec::new();
    let mut offset = Some(0);
    while let Some(start) = offset {
        let page = page(rows.clone(), Some(3), Some(start)).unwrap();
        assert_eq!(page.total, 7);
        assert_eq!(page.offset, start);
        seen.extend(page.items);
        offset = page.next_offset;
    }
    assert_eq!(seen, rows);
    let past_end = page(rows.clone(), Some(3), Some(50)).unwrap();
    assert_eq!(past_end.items, Vec::<usize>::new());
    assert_eq!(past_end.next_offset, None);
    assert!(page(rows, Some(0), None).is_err());
}

#[test]
fn token_file_is_private_stable_and_replaced_on_rotation() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kubecove-mcp-token-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    let first = load_or_create_token(&dir).unwrap();
    assert_eq!(
        first.len(),
        43,
        "32 random bytes, base64url without padding"
    );
    assert_eq!(load_or_create_token(&dir).unwrap(), first);
    let rotated = rotate_token(&dir).unwrap();
    assert_ne!(rotated, first);
    assert_eq!(load_or_create_token(&dir).unwrap(), rotated);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join(crate::TOKEN_FILE_NAME);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // A file loosened by hand is restricted again on load.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(load_or_create_token(&dir).unwrap(), rotated);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
