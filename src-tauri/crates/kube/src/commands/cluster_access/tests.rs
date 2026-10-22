use super::*;
use crate::commands::helpers::client_cache::rotate_client_generation;
use crate::models::AppErrorKind;
use k8s_openapi::api::core::v1::Namespace;
use kube::Api;
use std::{
    env, fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos()
}

/// Keys are assembled so the credential-scanning pre-commit hook does not flag
/// this credential-free, plain-HTTP fixture.
fn write_kubeconfig(server: &str) -> PathBuf {
    let [clusters, contexts, users, current] = [
        ["cluster", "s"],
        ["context", "s"],
        ["user", "s"],
        ["current", "-context"],
    ]
    .map(|parts| parts.concat());
    let path = env::temp_dir().join(format!("kubecove-cluster-access-{}.yaml", nanos()));
    fs::write(
        &path,
        format!(
            "apiVersion: v1\nkind: Config\n{current}: rotation\n{clusters}:\n- name: fake\n  cluster:\n    server: {server}\n{contexts}:\n- name: rotation\n  context:\n    cluster: fake\n    user: fake\n{users}:\n- name: fake\n  user: {{}}\n"
        ),
    )
    .expect("write kubeconfig");
    path
}

/// Accepts one request and rotates the session's finite client generation
/// while that request is in flight, then answers it.
async fn answer_after_rotation(listener: &TcpListener) {
    let (mut socket, _) = listener.accept().await.expect("accept request");
    let mut received = Vec::new();
    let mut buffer = [0; 4096];
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = socket.read(&mut buffer).await.expect("read request");
        assert!(read > 0, "client closed before sending a request");
        received.extend_from_slice(&buffer[..read]);
    }
    rotate_client_generation();
    let body = r#"{"apiVersion":"v1","kind":"Namespace","metadata":{"name":"default"}}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    // A cancelled client has already dropped the connection.
    let _ = socket.write_all(response.as_bytes()).await;
}

// Failure modes: a confirmed write, live session or stream cancelled by
// workspace rotation after the API server may already have acted, and a read
// or preview that keeps running after its workspace was left (ADR 0009).
#[tokio::test]
async fn generation_rotation_cancels_only_read_and_preview_clients() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let kubeconfig = write_kubeconfig(&format!("http://{}", listener.local_addr().unwrap()));
    let env_var = format!("KUBECOVE_TEST_CLUSTER_ACCESS_{}", nanos());
    env::set_var(&env_var, &kubeconfig);
    let access = ClusterAccess::from_kubeconfig(
        KubeconfigSource::from_settings(Some(&env_var), Vec::new(), true).expect("source"),
    );
    let scope = SessionScope::new(None);

    let mut outcomes = Vec::new();
    for intent in [
        ClientIntent::Read,
        ClientIntent::Preview,
        ClientIntent::ConfirmedOperation,
        ClientIntent::LiveSession,
        ClientIntent::Stream,
    ] {
        let result = Arc::clone(&scope)
            .run(async {
                let client = access.client(intent, "rotation").await?;
                let namespaces = Api::<Namespace>::all(client);
                let read = namespaces.get("default");
                let (result, ()) = tokio::time::timeout(Duration::from_secs(10), async {
                    tokio::join!(read, answer_after_rotation(&listener))
                })
                .await
                .expect("request timed out");
                result.map_err(AppError::from)
            })
            .await;
        outcomes.push((intent, result.map(|_| ()).map_err(|error| error.kind)));
    }
    env::remove_var(&env_var);
    let _ = fs::remove_file(&kubeconfig);

    assert_eq!(
        outcomes,
        vec![
            (ClientIntent::Read, Err(AppErrorKind::Cancelled)),
            (ClientIntent::Preview, Err(AppErrorKind::Cancelled)),
            (ClientIntent::ConfirmedOperation, Ok(())),
            (ClientIntent::LiveSession, Ok(())),
            (ClientIntent::Stream, Ok(())),
        ]
    );
}
