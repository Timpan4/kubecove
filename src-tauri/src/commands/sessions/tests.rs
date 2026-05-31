use super::*;
use std::{env, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, timeout},
};

fn valid_request() -> PortForwardRequest {
    PortForwardRequest {
        cluster_context: "kind-dev".to_string(),
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

fn live_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set for the live smoke test"))
}

fn live_port(name: &str) -> i64 {
    live_env(name)
        .parse()
        .unwrap_or_else(|_| panic!("{name} must be a valid port"))
}

fn live_expect_http_response() -> bool {
    matches!(
        env::var("KUBECOVE_LIVE_PF_EXPECT_HTTP_RESPONSE").as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

async fn wait_until_port_closes(port: u16) -> bool {
    for _ in 0..20 {
        match timeout(
            Duration::from_millis(250),
            TcpStream::connect((LOCAL_ADDRESS, port)),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => return true,
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

#[test]
fn validates_required_pod_target_and_ports() {
    assert!(validate_request(&valid_request()).is_ok());

    assert_eq!(
        validate_request(&PortForwardRequest {
            cluster_context: "".to_string(),
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
fn registry_lists_marks_and_stops_sessions() {
    let registry = PortForwardRegistry::default();
    registry.insert_summary_for_test(test_summary("pf-1", 18080));

    assert!(registry.has_local_port(18080));
    assert_eq!(registry.list().len(), 1);

    registry.mark_error("pf-1", "forbidden".to_string());
    let session = registry.list().pop().expect("session");
    assert_eq!(session.status, "error");
    assert_eq!(session.last_error.as_deref(), Some("forbidden"));

    assert!(registry.stop("pf-1"));
    assert!(!registry.stop("pf-1"));
    assert!(registry.list().is_empty());
}

#[test]
fn registry_detects_local_port_collision() {
    let registry = PortForwardRegistry::default();
    registry.insert_summary_for_test(test_summary("pf-1", 18080));

    assert!(registry.has_local_port(18080));
    assert!(!registry.has_local_port(18081));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires a reachable Kubernetes cluster and KUBECOVE_LIVE_PF_* env vars"]
async fn live_pod_port_forward_starts_serves_lists_and_stops() {
    let registry = PortForwardRegistry::default();
    let target_kind =
        env::var("KUBECOVE_LIVE_PF_TARGET_KIND").unwrap_or_else(|_| "Pod".to_string());
    let target_name = env::var("KUBECOVE_LIVE_PF_TARGET_NAME")
        .or_else(|_| env::var("KUBECOVE_LIVE_PF_POD"))
        .expect("KUBECOVE_LIVE_PF_TARGET_NAME or KUBECOVE_LIVE_PF_POD must be set");
    let request = PortForwardRequest {
        cluster_context: live_env("KUBECOVE_LIVE_PF_CONTEXT"),
        namespace: live_env("KUBECOVE_LIVE_PF_NAMESPACE"),
        target_kind: Some(target_kind),
        target_name: Some(target_name),
        pod_name: None,
        remote_port: live_port("KUBECOVE_LIVE_PF_REMOTE_PORT"),
        local_port: env::var("KUBECOVE_LIVE_PF_LOCAL_PORT").ok().map(|value| {
            value
                .parse()
                .expect("KUBECOVE_LIVE_PF_LOCAL_PORT must be valid")
        }),
    };

    let summary = start_pod_port_forward_in_registry(request, &registry)
        .await
        .expect("port-forward should start");
    assert_eq!(summary.local_address, LOCAL_ADDRESS);
    assert_eq!(summary.status, "listening");
    assert!(registry
        .list()
        .iter()
        .any(|session| session.id == summary.id));

    let mut stream = TcpStream::connect((LOCAL_ADDRESS, summary.local_port))
        .await
        .expect("local port should accept connections");
    if live_expect_http_response() {
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await
            .expect("request should write through port-forward");

        let mut response = vec![0_u8; 128];
        let bytes_read = timeout(Duration::from_secs(5), stream.read(&mut response))
            .await
            .expect("response should arrive through port-forward")
            .expect("response should read through port-forward");
        assert!(bytes_read > 0);
    }

    assert!(registry.stop(&summary.id));
    assert!(registry
        .list()
        .iter()
        .all(|session| session.id != summary.id));
    assert!(
        wait_until_port_closes(summary.local_port).await,
        "local port {} remained open after stop",
        summary.local_port
    );
}
