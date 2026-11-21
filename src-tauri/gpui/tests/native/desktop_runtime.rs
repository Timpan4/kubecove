use super::scratch::Scratch;
use kubecove_gpui::backend::{Backend, NativeBackend};
use kubecove_lib::commands::REQUEST_READ_TIMEOUT;
use std::{path::Path, process::Command};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn two_usage_samples_report_platform_memory() {
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
        if cfg!(target_os = "linux") {
            assert!(samples[0].0.wss_bytes.is_none());
            assert!(memory.wss_bytes.is_some_and(|value| value > 0) && window.is_some());
            assert!(memory.pss_bytes.is_some_and(|value| value > 0));
        } else {
            assert!(memory.wss_bytes.is_none() && memory.pss_bytes.is_none());
        }
    }
}

// Run the real exec credential process in a child test so no other tests see this kubeconfig.
#[test]
fn context_reads_and_cancellation_finish_while_exec_credentials_are_pending() {
    const CHILD: &str = "KUBECOVE_TEST_PENDING_PLUGIN";
    if std::env::var_os(CHILD).is_none() {
        let fixture = Scratch::new("credential-plugin-tests", "pending");
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "desktop_runtime::context_reads_and_cancellation_finish_while_exec_credentials_are_pending"])
            .env(CHILD, "1")
            .env("KUBECONFIG", fixture.join("config.json"))
            .env_remove("HTTPS_PROXY")
            .env_remove("https_proxy")
            .status().unwrap();
        assert!(status.success());
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(REQUEST_READ_TIMEOUT, exercise_pending_read())
            .await
            .expect("context listing and cancellation must finish before the credential deadline");
    });
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
    let (mut plugin, _) = gate.accept().await.unwrap();
    let contexts = backend.contexts(None).await.unwrap();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].name, "fixture");
    assert!(!pending.is_finished());
    assert_eq!(namespace_loads(&backend).await, 1);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    // The credential process is owned by a blocking client build. The backend
    // read must release its cache load while that build is still blocked.
    while namespace_loads(&backend).await != 0 {
        tokio::task::yield_now().await;
    }
    plugin.write_all(b"continue").await.unwrap();
    assert_eq!(plugin.read(&mut [0]).await.unwrap(), 0);
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
