use super::*;
use crate::commands::{LiveSessionScope, SessionScope};
use crate::models::LiveSessionCleanupRequest;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};
use target::{PortForwardTarget, PortForwardTargetKind, ValidatedPortForwardRequest};
use tokio::{
    net::TcpStream,
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

const CURRENT_SOURCE: &str = "kubeconfigSource=current";

fn test_target(cluster_context: &str, kubeconfig_source_key: Option<&str>) -> PortForwardTarget {
    PortForwardTarget {
        request: ValidatedPortForwardRequest {
            cluster_context: cluster_context.to_string(),
            kubeconfig_env_var: None,
            kubeconfig_source_key: kubeconfig_source_key.map(str::to_string),
            kubeconfig_source_label: None,
            namespace: "default".to_string(),
            target_kind: PortForwardTargetKind::Pod,
            target_name: "api-0".to_string(),
            remote_port: 8080,
            local_port: None,
        },
        pod_name: "api-0".to_string(),
        pod_port: 8080,
    }
}

/// Starts a forward through the lifecycle, relaying each connection through `handler`.
async fn start_forward<H, F>(
    registry: &PortForwardRegistry,
    target: &PortForwardTarget,
    handler: H,
) -> PortForwardSessionSummary
where
    H: Fn(TcpStream) -> F + Send + 'static,
    F: Future<Output = Result<(), AppError>> + Send + 'static,
{
    let listener = registry
        .bind(None)
        .await
        .expect("localhost listener should bind");
    registry
        .start(registry.next_id(), target, listener, handler)
        .expect("forward should start")
}

async fn spawn_session_with_handler<H, F>(
    registry: &PortForwardRegistry,
    handler: H,
) -> (String, u16)
where
    H: Fn(TcpStream) -> F + Send + 'static,
    F: Future<Output = Result<(), AppError>> + Send + 'static,
{
    let forward = start_forward(
        registry,
        &test_target("kind-dev", Some(CURRENT_SOURCE)),
        handler,
    )
    .await;
    (forward.id, forward.local_port)
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

#[tokio::test(flavor = "current_thread")]
async fn bind_rejects_a_local_port_that_is_already_forwarded() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let (session_id, local_port) =
            spawn_session_with_handler(&registry, |_stream| async { Ok(()) }).await;

        let error = registry
            .bind(Some(local_port))
            .await
            .err()
            .expect("a forwarded local port must be rejected");
        registry.stop(&session_id);

        assert_eq!(
            error.message,
            format!("local port {local_port} is already forwarded")
        );
        assert_eq!(error.kind, AppErrorKind::LiveSessionTargetUnavailable);
    })
    .await
    .expect("local port test should finish within timeout");
}

#[tokio::test(flavor = "current_thread")]
async fn scope_change_stops_forwards_outside_the_workspace_and_keeps_the_rest() {
    timeout(Duration::from_secs(5), async {
        let registry = PortForwardRegistry::default();
        let idle = |_stream| async { Ok(()) };
        let kept = start_forward(
            &registry,
            &test_target("kind-dev", Some(CURRENT_SOURCE)),
            idle,
        )
        .await;
        let mut outside = Vec::new();
        for target in [
            test_target("other-context", Some(CURRENT_SOURCE)),
            test_target("kind-dev", Some("kubeconfigSource=old")),
            test_target("kind-dev", None),
        ] {
            outside.push(start_forward(&registry, &target, idle).await);
        }

        let stopped = registry.stop_outside_scope(
            &LiveSessionScope::new(LiveSessionCleanupRequest {
                allowed_cluster_contexts: vec!["kind-dev".to_string()],
                kubeconfig_source_key: CURRENT_SOURCE.to_string(),
            })
            .expect("valid scope"),
        );

        assert_eq!(
            stopped,
            outside
                .iter()
                .map(|forward| forward.id.clone())
                .collect::<Vec<_>>()
        );
        let listed = registry.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, kept.id);
        for forward in &outside {
            assert!(
                wait_until_connect_fails(forward.local_port).await,
                "out-of-scope local port {} should close",
                forward.local_port
            );
        }
        TcpStream::connect((LOCAL_ADDRESS, kept.local_port))
            .await
            .expect("in-scope forward keeps listening");
        registry.stop(&kept.id);
    })
    .await
    .expect("scope change test should finish within timeout");
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

// A forwarding server refuses to build a Kubernetes client outside a session
// scope, and JoinSet tasks do not inherit the task-local scope by themselves.
#[tokio::test(flavor = "current_thread")]
async fn relay_runs_each_connection_in_the_callers_session_scope() {
    timeout(Duration::from_secs(5), async {
        let scope = SessionScope::new(None);
        let (listener, local_port) = relay::bind_loopback(None)
            .await
            .expect("localhost listener should bind");
        let (seen, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = Arc::clone(&scope).run_sync(|| {
            relay::spawn_relay(
                listener,
                move |_stream| {
                    let seen = seen.clone();
                    async move {
                        let _ = seen.send(SessionScope::current());
                        Ok(())
                    }
                },
                |_| {},
            )
        });

        let _stream = TcpStream::connect((LOCAL_ADDRESS, local_port))
            .await
            .expect("local connection should be accepted");
        let current = seen_rx.recv().await.expect("connection should run");
        handle.abort();

        assert!(current.is_some_and(|current| Arc::ptr_eq(&current, &scope)));
    })
    .await
    .expect("relay scope test should finish within timeout");
}
