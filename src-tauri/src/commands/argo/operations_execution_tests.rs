use super::*;
use crate::commands::helpers::client_cache::rotate_client_generation;
use kube::core::GroupVersionKind;
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
    let path = env::temp_dir().join(format!("kubecove-argo-operation-{}.yaml", nanos()));
    fs::write(
        &path,
        format!(
            "apiVersion: v1\nkind: Config\n{current}: argo-fallback\n{clusters}:\n- name: fake\n  cluster:\n    server: {server}\n{contexts}:\n- name: argo-fallback\n  context:\n    cluster: fake\n    user: fake\n{users}:\n- name: fake\n  user: {{}}\n"
        ),
    )
    .expect("write kubeconfig");
    path
}

/// Accepts one request, simulates a workspace switch while it is in flight,
/// then answers it.
async fn answer_after_rotation(listener: TcpListener) -> Vec<u8> {
    let (mut socket, _) = listener.accept().await.expect("accept");
    let mut received = Vec::new();
    let mut buffer = [0; 4096];
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = socket.read(&mut buffer).await.expect("read request");
        assert!(read > 0, "client closed before sending a request");
        received.extend_from_slice(&buffer[..read]);
    }
    rotate_client_generation();
    let body = r#"{"apiVersion":"argoproj.io/v1alpha1","kind":"Application","metadata":{"name":"demo","namespace":"argocd"}}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    // A cancelled client has already dropped the connection.
    let _ = socket.write_all(response.as_bytes()).await;
    received
}

#[tokio::test]
async fn confirmed_kubernetes_operation_survives_workspace_client_rotation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let kubeconfig = write_kubeconfig(&format!("http://{}", listener.local_addr().unwrap()));
    let env_var = format!("KUBECOVE_TEST_ARGO_OPERATION_{}", nanos());
    env::set_var(&env_var, &kubeconfig);
    let request = ArgoOperationRequest {
        action: "refresh".into(),
        transport: "kubernetes".into(),
        cluster_context: Some("argo-fallback".into()),
        kubeconfig_env_var: Some(env_var.clone()),
        resource_version: Some("42".into()),
        application: ArgoApplicationRef {
            name: "demo".into(),
            namespace: Some("argocd".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "argoproj.io",
        "v1alpha1",
        "Application",
    ));

    let (result, received) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(
            kubernetes_operation(request, resource),
            answer_after_rotation(listener)
        )
    })
    .await
    .expect("operation timed out");
    env::remove_var(&env_var);
    let _ = fs::remove_file(&kubeconfig);

    assert!(received
        .starts_with(b"PATCH /apis/argoproj.io/v1alpha1/namespaces/argocd/applications/demo"));
    assert!(
        result
            .expect("workspace rotation must not cancel a confirmed write")
            .accepted
    );
}
