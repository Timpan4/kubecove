//! Desktop tunnels through the remote invoke path, against a fake server that
//! answers tunnel controls on the socket queue and serves one HTTP invoke.

use super::{handle_text, RemoteConnection};
use crate::remote::{
    invoke_remote,
    models::{RemoteConnectionSummary, RemoteCredentials},
    tunnels::RemoteTunnelRegistry,
};
use kubecove_models::{LiveSessionCleanupResult, PortForwardSessionSummary};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::{
    net::Ipv4Addr,
    sync::{atomic::AtomicBool, Arc, Mutex, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
    time::sleep,
};
use tokio_tungstenite::tungstenite::Message as WsMessage;

const SERVER_SOURCE: &str = "kubeconfigSource=server";
const SERVER_SOURCE_LABEL: &str = "Server kubeconfig";

struct FakeServer {
    connection: Arc<RemoteConnection>,
    unregistered: Arc<Mutex<Vec<String>>>,
    invoked: JoinHandle<(String, Value)>,
}

async fn fake_server(invoke_response: Value) -> FakeServer {
    super::install_tls_provider();
    let http = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind fake server");
    let server_url = format!("http://{}", http.local_addr().expect("fake server address"));
    let invoked = tokio::spawn(serve_one_invoke(http, invoke_response));
    let (outgoing, sent) = mpsc::channel(32);
    let connection = Arc::new(RemoteConnection {
        credentials: RemoteCredentials {
            server_url: server_url.clone(),
            bearer_token: "test-token".to_string(),
            resume_token: "test-resume".to_string(),
        },
        http: HttpClient::new(),
        summary: RemoteConnectionSummary {
            server_url,
            server_version: "test".to_string(),
            session_grace_seconds: 30,
            connected: true,
        },
        outgoing,
        channels: Mutex::default(),
        controls: Mutex::default(),
        tunnel_senders: Mutex::default(),
        tunnels: Arc::new(RemoteTunnelRegistry::default()),
        active: AtomicBool::new(true),
        socket_task: Mutex::new(None),
    });
    let unregistered = Arc::default();
    tokio::spawn(answer_controls(
        Arc::downgrade(&connection),
        sent,
        Arc::clone(&unregistered),
    ));
    FakeServer {
        connection,
        unregistered,
        invoked,
    }
}

/// Registers every forward as the server would after validating it against
/// the server's kubeconfig source.
async fn answer_controls(
    connection: Weak<RemoteConnection>,
    mut sent: mpsc::Receiver<WsMessage>,
    unregistered: Arc<Mutex<Vec<String>>>,
) {
    while let Some(message) = sent.recv().await {
        let WsMessage::Text(text) = message else {
            continue;
        };
        let control: Value = serde_json::from_str(&text).expect("control frame is JSON");
        let forward_id = control["forwardId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let reply = match control["type"].as_str() {
            Some("tunnel_register") => json!({
                "type": "tunnel_registered", "forwardId": forward_id,
                "resolvedPodName": "api-0", "resolvedPodPort": 8080,
                "kubeconfigSourceKey": SERVER_SOURCE, "kubeconfigSourceLabel": SERVER_SOURCE_LABEL,
            }),
            Some("tunnel_unregister") => {
                unregistered.lock().unwrap().push(forward_id.clone());
                json!({"type": "tunnel_unregistered", "forwardId": forward_id})
            }
            _ => continue,
        };
        let Some(connection) = connection.upgrade() else {
            return;
        };
        handle_text(&connection, &reply.to_string());
    }
}

/// Answers one HTTP invoke and returns its path and JSON body.
async fn serve_one_invoke(listener: TcpListener, response: Value) -> (String, Value) {
    let (mut stream, _) = listener.accept().await.expect("accept invoke");
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    let (head_end, body_len) = loop {
        let read = stream.read(&mut buffer).await.expect("read invoke");
        assert!(read > 0, "client closed before sending its invoke");
        request.extend_from_slice(&buffer[..read]);
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = std::str::from_utf8(&request[..end]).expect("ASCII headers");
            let length = head
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map_or(0, |(_, value)| {
                    value.trim().parse().expect("content length")
                });
            break (end + 4, length);
        }
    };
    while request.len() < head_end + body_len {
        let read = stream.read(&mut buffer).await.expect("read invoke body");
        assert!(read > 0, "client closed before sending its invoke body");
        request.extend_from_slice(&buffer[..read]);
    }
    let path = std::str::from_utf8(&request[..head_end])
        .expect("ASCII request line")
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_string();
    let body = serde_json::from_slice(&request[head_end..head_end + body_len]).expect("JSON body");
    let response = response.to_string();
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .as_bytes(),
        )
        .await
        .expect("write invoke response");
    (path, body)
}

async fn start_tunnel(
    connection: &Arc<RemoteConnection>,
    cluster_context: &str,
) -> PortForwardSessionSummary {
    let request = json!({"request": {
        "clusterContext": cluster_context, "namespace": "default",
        "targetKind": "Pod", "targetName": "api-0", "remotePort": 8080,
    }});
    serde_json::from_value(
        invoke_remote(connection, "start_pod_port_forward", request)
            .await
            .expect("start tunnelled forward"),
    )
    .expect("port-forward summary")
}

async fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    for _ in 0..50 {
        if condition() {
            return true;
        }
        sleep(Duration::from_millis(20)).await;
    }
    false
}

async fn wait_until_connect_fails(local_port: u16) -> bool {
    for _ in 0..50 {
        if TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
            .await
            .is_err()
        {
            return true;
        }
        sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn tunnelled_forwards_carry_the_server_source_key() {
    let server = fake_server(Value::Null).await;

    let forward = start_tunnel(&server.connection, "ctx-a").await;

    assert_eq!(
        forward.kubeconfig_source_key.as_deref(),
        Some(SERVER_SOURCE)
    );
    assert_eq!(
        forward.kubeconfig_source_label.as_deref(),
        Some(SERVER_SOURCE_LABEL)
    );
    server.connection.tunnels.close_all();
}

#[tokio::test]
async fn scope_cleanup_stops_out_of_scope_tunnels_and_keeps_in_scope_ones() {
    let server = fake_server(json!({
        "stoppedPortForwardIds": [], "stoppedPodExecIds": ["pod-exec-1"],
        "stoppedPortForwards": 0, "stoppedPodExecSessions": 1,
    }))
    .await;
    let kept = start_tunnel(&server.connection, "ctx-a").await;
    let stopped = start_tunnel(&server.connection, "ctx-b").await;

    let cleanup = json!({"request": {
        "allowedClusterContexts": ["ctx-a"], "kubeconfigSourceKey": SERVER_SOURCE,
    }});
    let result: LiveSessionCleanupResult = serde_json::from_value(
        invoke_remote(
            &server.connection,
            "stop_live_sessions_outside_scope",
            cleanup,
        )
        .await
        .expect("scope cleanup"),
    )
    .expect("cleanup result");

    assert_eq!(
        result.stopped_port_forward_ids,
        std::slice::from_ref(&stopped.id)
    );
    assert_eq!(result.stopped_port_forwards, 1);
    // The server still applies the cleanup to its own sessions.
    assert_eq!(result.stopped_pod_exec_ids, ["pod-exec-1"]);
    assert_eq!(result.stopped_pod_exec_sessions, 1);
    let (path, body) = server.invoked.await.expect("fake server");
    assert_eq!(path, "/api/v1/invoke/stop_live_sessions_outside_scope");
    assert_eq!(body["request"]["kubeconfigSourceKey"], SERVER_SOURCE);

    let listed = server
        .connection
        .tunnels
        .list()
        .into_iter()
        .map(|forward| forward.id)
        .collect::<Vec<_>>();
    assert_eq!(listed, std::slice::from_ref(&kept.id));
    assert!(
        wait_until(|| *server.unregistered.lock().unwrap() == std::slice::from_ref(&stopped.id))
            .await,
        "the server must drop the stopped forward's registration"
    );
    assert!(
        wait_until_connect_fails(stopped.local_port).await,
        "the out-of-scope laptop listener must close"
    );
    TcpStream::connect((Ipv4Addr::LOCALHOST, kept.local_port))
        .await
        .expect("the in-scope laptop listener keeps accepting");
    server.connection.tunnels.close_all();
}
