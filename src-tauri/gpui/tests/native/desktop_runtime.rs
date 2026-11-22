use super::scratch::Scratch;
use kubecove_gpui::backend::{Backend, NativeBackend};
use kubecove_lib::commands::REQUEST_READ_TIMEOUT;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

#[tokio::test]
async fn two_usage_samples_report_platform_memory() {
    #[cfg(target_os = "linux")]
    let (pss_available, referenced_available, can_reset) = {
        let rollup = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
        let can_reset = std::fs::OpenOptions::new()
            .write(true)
            .open("/proc/self/clear_refs")
            .and_then(|mut file| std::io::Write::write_all(&mut file, b"1"))
            .is_ok();
        let contains = |field| rollup.lines().any(|line| line.starts_with(field));
        (contains("Pss:"), contains("Referenced:"), can_reset)
    };
    let backend = NativeBackend::new().unwrap();
    let mut samples = Vec::new();
    for _ in 0..2 {
        let metrics = backend.app_usage(true).await.unwrap();
        let host = metrics
            .breakdown
            .into_iter()
            .find(|group| group.label == "KubeCove")
            .unwrap();
        samples.push((host.memory, metrics.working_set_window_ms));
    }
    let (memory, window) = &samples[1];
    if cfg!(target_os = "windows") {
        assert!(memory.wss_bytes.is_some_and(|value| value > 0));
        assert!(memory.rss_bytes.is_none() && memory.pss_bytes.is_none());
    } else {
        assert!(memory.rss_bytes.is_some_and(|value| value > 0));
        #[cfg(target_os = "linux")]
        {
            assert!(samples[0].0.wss_bytes.is_none());
            assert_eq!(
                memory.wss_bytes.is_some(),
                can_reset && referenced_available
            );
            assert_eq!(window.is_some(), can_reset);
            assert_eq!(memory.pss_bytes.is_some(), pss_available);
            assert!(memory.pss_bytes.is_none_or(|value| value > 0));
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(window.is_none());
            assert!(memory.wss_bytes.is_none() && memory.pss_bytes.is_none());
        }
    }
}

// Run the real exec credential process in a child test so no other tests see this kubeconfig.
#[tokio::test]
async fn context_reads_and_cancellation_finish_while_exec_credentials_are_pending() {
    const CHILD: &str = "KUBECOVE_TEST_PENDING_PLUGIN";
    if std::env::var_os(CHILD).is_none() {
        let fixture = Scratch::new("credential-plugin-tests", "pending");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "desktop_runtime::context_reads_and_cancellation_finish_while_exec_credentials_are_pending"])
            .env(CHILD, "1")
            .env("KUBECONFIG", fixture.join("config.json"))
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let Ok(status) = tokio::time::timeout(REQUEST_READ_TIMEOUT, child.wait()).await else {
            child
                .kill()
                .await
                .expect("kill and reap the stalled credential test");
            panic!("desktop runtime credential test exceeded the request deadline");
        };
        let status = status.expect("wait for desktop runtime credential test");
        assert!(status.success());
        return;
    }
    exercise_pending_read().await;
}

async fn exercise_pending_read() {
    let gate = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/fixtures/exec-credential-plugin.mjs");
    // Own the API port throughout the test so the pending read can never contact another service.
    let api = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = serde_json::json!({
        "clusters": [{"name":"fixture","cluster":{"server":format!("http://{}", api.local_addr().unwrap())}}],
        "contexts": [{"name":"fixture","context":{"cluster":"fixture","user":"fixture"}}],
        "users": [{"name":"fixture","user":{"exec":{
            "apiVersion":"client.authentication.k8s.io/v1", "command":"node", "interactiveMode":"Never",
            "args":[plugin,gate.local_addr().unwrap().port().to_string()]
        }}}]
    });
    let path = std::env::var_os("KUBECONFIG").unwrap();
    std::fs::write(path, serde_json::to_vec(&config).unwrap()).unwrap();
    let backend = NativeBackend::new().unwrap();
    let pending = tokio::spawn(backend.namespaces("fixture".into(), None));
    let (mut plugin, contexts) = tokio::time::timeout(REQUEST_READ_TIMEOUT, async {
        let (plugin, _) = gate.accept().await.unwrap();
        (plugin, backend.contexts(None).await.unwrap())
    })
    .await
    .expect("context listing must finish while credentials are pending");
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].name, "fixture");
    assert!(!pending.is_finished());
    // Use the local wait budget from the native MCP tests and the remote session poll cadence.
    tokio::time::timeout(Duration::from_secs(5), async {
        assert_eq!(namespace_loads(&backend).await, 1);
        pending.abort();
        assert!(pending.await.unwrap_err().is_cancelled());
        // The cache load must end while the credential process is still blocked.
        while namespace_loads(&backend).await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("namespace cache still loading after cancellation");
    tokio::time::timeout(REQUEST_READ_TIMEOUT, async {
        plugin.write_all(b"continue").await.unwrap();
        assert_eq!(plugin.read(&mut [0]).await.unwrap(), 0);
    })
    .await
    .expect("credential plugin did not exit after being released");
}

async fn namespace_loads(backend: &NativeBackend) -> usize {
    backend
        .diagnostics(true, false)
        .await
        .unwrap()
        .caches
        .into_iter()
        .find(|cache| cache.label == "namespaces")
        .expect("namespace cache diagnostics")
        .loading
}
