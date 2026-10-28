use super::*;
use crate::commands::argo::scope::{acquire_connection_lease, scoped_connection};
use crate::models::ArgoServerEndpoint;
use uuid::Uuid;

fn profile() -> ArgoConnectionProfile {
    ArgoConnectionProfile {
        id: "server".into(),
        url: "https://argo.example/argo-cd".into(),
        endpoint: ArgoServerEndpoint::ExternalHttps {
            url: "https://argo.example/argo-cd".into(),
        },
        cluster_context: Some("cluster".into()),
        workspace_id: Some("workspace".into()),
        kubeconfig_source_key: Some("kubeconfigSource=test".into()),
        transport: "connected".into(),
        remember_credential: true,
    }
}

fn connection(generation: &str) -> Arc<ConnectedArgo> {
    connection_with_tunnel(generation, None)
}

fn connection_with_tunnel(
    generation: &str,
    tunnel: Option<ArgoServiceTunnel>,
) -> Arc<ConnectedArgo> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    Arc::new(ConnectedArgo {
        profile: profile(),
        token: "token".into(),
        username: None,
        client: HttpClient::new(),
        tunnel,
        generation: generation.into(),
        instance_id: Uuid::new_v4().to_string(),
        gate: Arc::new(tokio::sync::Mutex::new(())),
    })
}

#[tokio::test]
async fn lease_rejects_replaced_and_disconnected_connections() {
    let store = ArgoConnectionStore::default();
    let old = connection("old");
    store
        .replace_connection("server".into(), old)
        .await
        .unwrap();
    let selected = scoped_connection(
        &store,
        "server",
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
    )
    .unwrap();
    let replacement = connection("replacement");
    store
        .replace_connection("server".into(), replacement.clone())
        .await
        .unwrap();

    let error = acquire_connection_lease(
        &store,
        "server",
        selected,
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
        "old",
        "old-instance",
    )
    .await
    .err()
    .expect("replacement must reject the old connection");
    assert_eq!(error.message, "Argo CD connection was replaced");
    assert!(Arc::ptr_eq(
        store.connections.lock().unwrap().get("server").unwrap(),
        &replacement
    ));

    let selected = scoped_connection(
        &store,
        "server",
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
    )
    .unwrap();
    store.remove_connection("server").await.unwrap();
    let error = acquire_connection_lease(
        &store,
        "server",
        selected,
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
        "replacement",
        "replacement-instance",
    )
    .await
    .err()
    .expect("disconnect must reject the selected connection");
    assert_eq!(error.message, "Argo CD connection was replaced");
}

#[tokio::test]
async fn disconnect_and_replacement_wait_for_active_lease() {
    let store = ArgoConnectionStore::default();
    let old = connection("old");
    store
        .replace_connection("server".into(), old.clone())
        .await
        .unwrap();

    let lease = acquire_connection_lease(
        &store,
        "server",
        scoped_connection(
            &store,
            "server",
            "cluster",
            Some("workspace"),
            "kubeconfigSource=test",
        )
        .unwrap(),
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
        "old",
        &old.instance_id,
    )
    .await
    .unwrap();
    let mut removal = Box::pin(store.remove_connection("server"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), removal.as_mut())
            .await
            .is_err()
    );
    assert!(Arc::ptr_eq(
        store.connections.lock().unwrap().get("server").unwrap(),
        &old
    ));
    drop(lease);
    removal.await.unwrap();
    assert!(store.connections.lock().unwrap().is_empty());

    let old = connection("old");
    store
        .replace_connection("server".into(), old.clone())
        .await
        .unwrap();
    let lease = acquire_connection_lease(
        &store,
        "server",
        scoped_connection(
            &store,
            "server",
            "cluster",
            Some("workspace"),
            "kubeconfigSource=test",
        )
        .unwrap(),
        "cluster",
        Some("workspace"),
        "kubeconfigSource=test",
        "old",
        &old.instance_id,
    )
    .await
    .unwrap();
    let replacement = connection("replacement");
    let mut replace = Box::pin(store.replace_connection("server".into(), replacement.clone()));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), replace.as_mut())
            .await
            .is_err()
    );
    assert!(Arc::ptr_eq(
        store.connections.lock().unwrap().get("server").unwrap(),
        &old
    ));
    drop(lease);
    replace.await.unwrap();
    assert!(Arc::ptr_eq(
        store.connections.lock().unwrap().get("server").unwrap(),
        &replacement
    ));
}

#[tokio::test]
async fn disconnect_removes_replacement_after_waiting_on_old_gate() {
    let store = ArgoConnectionStore::default();
    let old = connection("old");
    store
        .replace_connection("server".into(), old.clone())
        .await
        .unwrap();
    let old_gate = old.gate.clone().lock_owned().await;
    let replacement = connection("replacement");
    let mut replace = Box::pin(store.replace_connection("server".into(), replacement));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), replace.as_mut())
            .await
            .is_err()
    );
    let mut removal = Box::pin(store.remove_connection("server"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), removal.as_mut())
            .await
            .is_err()
    );

    drop(old_gate);
    replace.await.unwrap();
    removal.await.unwrap();
    assert!(store.connections.lock().unwrap().is_empty());
}

#[tokio::test]
async fn close_all_removes_connections_and_closes_tunnels() {
    let store = ArgoConnectionStore::default();
    let (first_relay, first_relay_stopped) = tokio::sync::oneshot::channel();
    let (second_relay, second_relay_stopped) = tokio::sync::oneshot::channel();
    store
        .replace_connection(
            "first".into(),
            connection_with_tunnel("first", Some(ArgoServiceTunnel::test_tunnel(first_relay))),
        )
        .await
        .unwrap();
    store
        .replace_connection(
            "second".into(),
            connection_with_tunnel("second", Some(ArgoServiceTunnel::test_tunnel(second_relay))),
        )
        .await
        .unwrap();

    store.close_all().await.unwrap();

    assert!(store.connections.lock().unwrap().is_empty());
    assert!(first_relay_stopped.await.is_err());
    assert!(second_relay_stopped.await.is_err());
}

#[tokio::test]
async fn source_scope_rejects_same_context_from_another_kubeconfig() {
    let store = ArgoConnectionStore::default();
    store
        .replace_connection("server".into(), connection("generation"))
        .await
        .unwrap();

    let error = scoped_connection(
        &store,
        "server",
        "cluster",
        Some("workspace"),
        "kubeconfigSource=other",
    )
    .err()
    .expect("another kubeconfig source must not reuse the connection");

    assert_eq!(
        error.message,
        "Argo CD connection is outside current workspace scope"
    );
}

#[tokio::test]
async fn workspace_cleanup_rejects_connection_started_before_cleanup() {
    let store = ArgoConnectionStore::default();
    let started_epoch = store.connection_epoch();
    let (relay, relay_stopped) = tokio::sync::oneshot::channel();
    let late = connection_with_tunnel("late", Some(ArgoServiceTunnel::test_tunnel(relay)));

    store.close_all().await.unwrap();
    let error = store
        .replace_connection_at("late".into(), late, started_epoch)
        .await
        .expect_err("cleanup must reject a connection that started earlier");

    assert_eq!(
        error.message,
        "Argo CD connection was cancelled by workspace cleanup"
    );
    assert!(store.connections.lock().unwrap().is_empty());
    assert!(relay_stopped.await.is_err());
}
