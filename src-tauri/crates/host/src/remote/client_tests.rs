use super::super::models::RemoteConnectionSummary;
use super::{api_url, channel_id_from_marker, normalize_server_url, RemoteConnection};
use crate::remote::{models::RemoteCredentials, tunnels::RemoteTunnelRegistry};
use reqwest::Client as HttpClient;
use serde_json::json;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[tokio::test]
async fn close_marks_connection_inactive_before_waiting_for_server() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("test listener address");
    let (request_received, received) = oneshot::channel();
    let (release_response, response_released) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept close request");
        let _ = request_received.send(());
        let _ = response_released.await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .expect("write close response");
    });
    super::install_tls_provider();
    let (outgoing, _receiver) = mpsc::channel(1);
    outgoing
        .try_send(WsMessage::Ping(Vec::new().into()))
        .expect("fill outgoing queue");
    let connection = Arc::new(RemoteConnection {
        credentials: RemoteCredentials {
            server_url: format!("http://{address}"),
            bearer_token: "test-token".to_string(),
            resume_token: "test-resume".to_string(),
        },
        http: HttpClient::new(),
        summary: RemoteConnectionSummary {
            server_url: format!("http://{address}"),
            server_version: "test".to_string(),
            session_grace_seconds: 30,
            connected: true,
        },
        outgoing,
        channels: Mutex::new(std::collections::HashMap::default()),
        controls: Mutex::new(std::collections::HashMap::default()),
        tunnel_senders: Mutex::new(std::collections::HashMap::default()),
        tunnels: Arc::new(RemoteTunnelRegistry::default()),
        active: AtomicBool::new(true),
        socket_task: Mutex::new(None),
    });
    let closing = {
        let connection = Arc::clone(&connection);
        tokio::spawn(async move { connection.close().await })
    };

    received.await.expect("server receives close request");
    closing
        .await
        .expect("local close returns while server is stalled");
    assert!(!connection.is_active());
    assert!(
        !server.is_finished(),
        "server is still withholding its response"
    );

    let _ = release_response.send(());
    server.await.expect("test server");
}

#[test]
fn tunnel_error_removes_download_sender_and_reaches_control_waiter() {
    super::install_tls_provider();
    let (outgoing, _receiver) = mpsc::channel(1);
    let connection = RemoteConnection {
        credentials: RemoteCredentials {
            server_url: "http://localhost".to_string(),
            bearer_token: "test-token".to_string(),
            resume_token: "test-resume".to_string(),
        },
        http: HttpClient::new(),
        summary: RemoteConnectionSummary {
            server_url: "http://localhost".to_string(),
            server_version: "test".to_string(),
            session_grace_seconds: 30,
            connected: true,
        },
        outgoing,
        channels: Mutex::new(std::collections::HashMap::default()),
        controls: Mutex::new(std::collections::HashMap::default()),
        tunnel_senders: Mutex::new(std::collections::HashMap::default()),
        tunnels: Arc::new(RemoteTunnelRegistry::default()),
        active: AtomicBool::new(true),
        socket_task: Mutex::new(None),
    };
    let connection_id = "550e8400-e29b-41d4-a716-446655440000";
    let (tunnel_sender, _tunnel_receiver) = mpsc::channel(1);
    connection
        .add_tunnel_sender(connection_id.to_string(), tunnel_sender)
        .expect("register tunnel download sender");
    let control_key = format!("tunnel_opened:{connection_id}");
    let (control_sender, mut control_receiver) = oneshot::channel();
    connection
        .controls
        .lock()
        .expect("remote control registry lock")
        .insert(control_key, control_sender);
    let error = json!({
        "type": "tunnel_error",
        "connectionId": connection_id,
        "error": {"message": "remote relay failed"}
    });

    assert!(!super::handle_text(&connection, &error.to_string()));
    assert!(!connection
        .tunnel_senders
        .lock()
        .expect("remote tunnel connection lock")
        .contains_key(connection_id));
    assert_eq!(
        control_receiver
            .try_recv()
            .expect("deliver handshake error"),
        error
    );

    // Forward nested JSON verbatim, including escapes and number lexemes. A
    // parse into Value loses this representation and allocates the entire tree.
    let received = Arc::new(Mutex::new(Vec::new()));
    let sent = Arc::clone(&received);
    let channel = tauri::ipc::Channel::new(move |body| {
        let tauri::ipc::InvokeResponseBody::Json(json) = body else {
            panic!("channel payload must remain JSON");
        };
        sent.lock().unwrap().push(json);
        Ok(())
    });
    connection.subscribe(42, channel).unwrap();
    let payload = r#"{ "nested": [1.00, {"line":"a\n\"b","unicode":"\u0061"}], "null": null }"#;
    assert!(!super::handle_text(
        &connection,
        &format!(r#"{{"type":"channel","channelId":42,"message":{payload}}}"#)
    ));
    assert!(!super::handle_text(
        &connection,
        r#"{"type":"channel_gap","channelId":42,"dropped":3}"#
    ));
    assert!(!super::handle_text(
        &connection,
        r#"{"type":"channel","channelId":42}"#
    ));
    assert!(!super::handle_text(
        &connection,
        r#"{"type":"channel","channelId":42,"message":[}"#
    ));
    assert_eq!(
        *received.lock().unwrap(),
        [
            payload.to_string(),
            r#"{"dropped":3,"type":"channel_gap"}"#.to_string(),
            "null".to_string(),
        ]
    );
}

#[test]
fn plaintext_urls_are_allowed_only_for_loopback_hosts() {
    for url in [
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://127.0.0.2:8080",
        "http://[::1]:8080",
        "https://example.test",
    ] {
        assert!(normalize_server_url(url).is_ok(), "rejected {url}");
    }
    for url in [
        "http://example.test",
        "http://localhost.example.test",
        "http://192.0.2.1",
        "http://[2001:db8::1]",
    ] {
        assert!(normalize_server_url(url).is_err(), "accepted {url}");
    }
}

#[test]
fn api_routes_include_the_version_and_preserve_a_proxy_prefix() {
    for (base, expected) in [
        (
            "https://example.test",
            "https://example.test/api/v1/session",
        ),
        (
            "https://example.test/kubecove/",
            "https://example.test/kubecove/api/v1/session",
        ),
    ] {
        assert_eq!(api_url(base, "session").unwrap().as_str(), expected);
    }
}

#[test]
fn rejects_urls_with_embedded_credentials_or_browser_fragment() {
    for url in [
        "https://user:password@example.test",
        "https://example.test/#token=secret",
        "file:///tmp/server",
    ] {
        assert!(normalize_server_url(url).is_err(), "accepted {url}");
    }
}

#[test]
fn normalizes_trailing_slashes_and_keeps_reverse_proxy_prefix() {
    assert_eq!(
        normalize_server_url("https://example.test/kubecove/").expect("valid URL"),
        "https://example.test/kubecove"
    );
}

#[test]
fn accepts_only_safe_server_channel_markers() {
    assert_eq!(channel_id_from_marker("__KUBECOVE_CHANNEL__:42"), Some(42));
    for marker in [
        "__TAURI_CHANNEL__:42",
        "__KUBECOVE_CHANNEL__:-1",
        "__KUBECOVE_CHANNEL__:4x",
    ] {
        assert_eq!(channel_id_from_marker(marker), None);
    }
}
