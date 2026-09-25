use super::*;
use std::{
    collections::HashSet,
    future::Future,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{sleep, timeout},
};

fn valid_request() -> PortForwardRequest {
    PortForwardRequest {
        cluster_context: "kind-dev".to_string(),
        kubeconfig_env_var: None,
        namespace: "default".to_string(),
        target_kind: Some("Pod".to_string()),
        target_name: Some("api-0".to_string()),
        pod_name: None,
        remote_port: 8080,
        local_port: Some(18080),
    }
}

fn test_summary(id: &str, local_port: u16) -> PortForwardSessionSummary {
    PortForwardSessionSummary {
        id: id.to_string(),
        cluster_context: "kind-dev".to_string(),
        kubeconfig_env_var: None,
        kubeconfig_source_key: None,
        kubeconfig_source_label: None,
        namespace: "default".to_string(),
        target_kind: "Pod".to_string(),
        target_name: "api-0".to_string(),
        pod_name: "api-0".to_string(),
        remote_port: 8080,
        resolved_pod_name: "api-0".to_string(),
        resolved_pod_port: 8080,
        local_port,
        local_address: LOCAL_ADDRESS.to_string(),
        local_url: format!("http://{LOCAL_ADDRESS}:{local_port}"),
        status: "listening".to_string(),
        started_at: "2026-05-31T00:00:00Z".to_string(),
        last_error: None,
    }
}

async fn spawn_session_with_handler<H, F>(
    registry: &PortForwardRegistry,
    handler: H,
) -> (String, u16)
where
    H: Fn(TcpStream) -> F + Send + 'static,
    F: Future<Output = Result<(), AppError>> + Send + 'static,
{
    let listener = TcpListener::bind((LOCAL_ADDRESS, 0))
        .await
        .expect("localhost listener should bind");
    let local_port = listener
        .local_addr()
        .expect("localhost listener should have a bound address")
        .port();
    let id = registry.session_id();
    let task_id = id.clone();
    let task_registry = registry.clone();
    let handle = tauri::async_runtime::spawn(async move {
        run_port_forward_session(task_id, listener, task_registry, handler).await;
    });
    registry
        .insert(test_summary(&id, local_port), handle)
        .expect("test session should insert");
    (id, local_port)
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
        match timeout(
            Duration::from_millis(50),
            TcpStream::connect((LOCAL_ADDRESS, local_port)),
        )
        .await
        {
            Ok(Ok(_stream)) => {}
            Ok(Err(_)) | Err(_) => return true,
        }
        sleep(Duration::from_millis(20)).await;
    }
    false
}

fn session_snapshot(
    registry: &PortForwardRegistry,
    session_id: &str,
) -> Option<PortForwardSessionSummary> {
    registry
        .list()
        .into_iter()
        .find(|session| session.id == session_id)
}

#[test]
fn validates_required_pod_target_and_ports() {
    assert!(validate_request(&valid_request()).is_ok());

    assert_eq!(
        validate_request(&PortForwardRequest {
            cluster_context: String::new(),
            ..valid_request()
        })
        .expect_err("missing target")
        .message,
        "port-forward target is required",
    );
    assert_eq!(
        validate_request(&PortForwardRequest {
            target_kind: Some("Deployment".to_string()),
            ..valid_request()
        })
        .expect_err("unsupported target")
        .message,
        "port-forward target kind must be Pod or Service",
    );
    assert_eq!(
        validate_request(&PortForwardRequest {
            remote_port: 0,
            ..valid_request()
        })
        .expect_err("bad remote port")
        .message,
        "remote_port must be between 1 and 65535",
    );
    assert_eq!(
        validate_request(&PortForwardRequest {
            local_port: Some(80),
            ..valid_request()
        })
        .expect_err("privileged local port")
        .message,
        "local_port must be 1024 or higher",
    );
}

#[test]
fn registry_detects_local_port_collision() {
    let registry = PortForwardRegistry::default();
    registry.insert_summary_for_test(test_summary("pf-1", 18080));

    assert!(registry.has_local_port(18080));
    assert!(!registry.has_local_port(18081));
}

#[test]
fn registry_stops_sessions_outside_context_or_source_scope() {
    let registry = PortForwardRegistry::default();
    let mut kept = test_summary("pf-kept", 18080);
    kept.kubeconfig_source_key = Some("kubeconfigSource=current".to_string());
    let mut wrong_context = test_summary("pf-context", 18081);
    wrong_context.cluster_context = "other-context".to_string();
    wrong_context.kubeconfig_source_key = Some("kubeconfigSource=current".to_string());
    let mut wrong_source = test_summary("pf-source", 18082);
    wrong_source.kubeconfig_source_key = Some("kubeconfigSource=old".to_string());
    registry.insert_summary_for_test(kept);
    registry.insert_summary_for_test(wrong_context);
    registry.insert_summary_for_test(wrong_source);

    let allowed = HashSet::from(["kind-dev".to_string()]);
    let stopped = registry.stop_outside_scope(&allowed, "kubeconfigSource=current");

    assert_eq!(
        stopped,
        vec!["pf-context".to_string(), "pf-source".to_string()]
    );
    let remaining = registry.list();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "pf-kept");
}

#[tokio::test(flavor = "current_thread")]
async fn session_loop_invokes_handler_per_connection() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let accepted_count = Arc::new(AtomicU32::new(0));
        let handler_count = accepted_count.clone();
        let (session_id, local_port) = spawn_session_with_handler(&registry, move |_stream| {
            let handler_count = handler_count.clone();
            async move {
                handler_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        })
        .await;

        let _first = TcpStream::connect((LOCAL_ADDRESS, local_port))
            .await
            .expect("first local connection should be accepted");
        let _second = TcpStream::connect((LOCAL_ADDRESS, local_port))
            .await
            .expect("second local connection should be accepted");

        let saw_both_connections = wait_until(|| accepted_count.load(Ordering::Relaxed) == 2).await;
        registry.stop(&session_id);

        assert!(
            saw_both_connections,
            "session loop should invoke handler for each accepted connection"
        );
    })
    .await
    .expect("session lifecycle test should finish within timeout");
}

#[tokio::test(flavor = "current_thread")]
async fn session_loop_returns_to_listening_after_connections_close() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let (session_id, local_port) =
            spawn_session_with_handler(&registry, |_stream| async { Ok(()) }).await;
        registry.mark_status(&session_id, "connected", None);

        let _stream = TcpStream::connect((LOCAL_ADDRESS, local_port))
            .await
            .expect("local connection should be accepted");

        let returned_to_listening = wait_until(|| {
            session_snapshot(&registry, &session_id).is_some_and(|session| {
                session.status == "listening" && session.last_error.is_none()
            })
        })
        .await;
        registry.stop(&session_id);

        assert!(
            returned_to_listening,
            "session loop should mark idle sessions as listening"
        );
    })
    .await
    .expect("session lifecycle test should finish within timeout");
}

#[tokio::test(flavor = "current_thread")]
async fn handler_error_marks_session_error_but_keeps_accepting() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let accepted_count = Arc::new(AtomicU32::new(0));
        let handler_count = accepted_count.clone();
        let (session_id, local_port) = spawn_session_with_handler(&registry, move |_stream| {
            let handler_count = handler_count.clone();
            async move {
                let call_index = handler_count.fetch_add(1, Ordering::Relaxed);
                if call_index == 0 {
                    Err(AppError::new("boom", crate::models::AppErrorKind::Session))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        let _first = TcpStream::connect((LOCAL_ADDRESS, local_port))
            .await
            .expect("first local connection should be accepted");

        let marked_error = wait_until(|| {
            session_snapshot(&registry, &session_id).is_some_and(|session| {
                session.status == "error" && session.last_error.as_deref() == Some("boom")
            })
        })
        .await;

        let accepted_after_error = if marked_error {
            let _second = TcpStream::connect((LOCAL_ADDRESS, local_port))
                .await
                .expect("second local connection should be accepted after handler error");
            wait_until(|| accepted_count.load(Ordering::Relaxed) == 2).await
        } else {
            false
        };
        registry.stop(&session_id);

        assert!(
            marked_error,
            "handler error should mark session status and last_error"
        );
        assert!(
            accepted_after_error,
            "session loop should keep accepting after handler error"
        );
    })
    .await
    .expect("session lifecycle test should finish within timeout");
}

#[tokio::test(flavor = "current_thread")]
async fn stop_aborts_session_task() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let (session_id, local_port) =
            spawn_session_with_handler(&registry, |_stream| async { Ok(()) }).await;

        assert!(registry.stop(&session_id));
        assert!(
            wait_until_connect_fails(local_port).await,
            "local port {local_port} should close after session stop"
        );
    })
    .await
    .expect("session lifecycle test should finish within timeout");
}
