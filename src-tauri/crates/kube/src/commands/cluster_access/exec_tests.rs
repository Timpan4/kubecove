#![cfg(unix)]

use super::*;
use crate::commands::helpers::{
    client_cache::rotate_client_generation, session_scope::SessionScope,
};
use crate::models::AppErrorKind;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

// Failure modes: blocking a single-thread runtime during login; duplicate cold
// builds; cancellation releasing the build gate before the subprocess exits;
// stale cache publication; sharing build gates or clients across sessions.
struct Plugin {
    root: PathBuf,
    source: KubeconfigSource,
}

impl Plugin {
    fn new() -> Self {
        Self::with_server("http://127.0.0.1:1")
    }

    fn with_server(server: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "kubecove-exec-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("expiry"), "2099-01-01T00:00:00Z\n").unwrap();
        assert!(Command::new("mkfifo")
            .arg(root.join("release"))
            .status()
            .unwrap()
            .success());
        fs::write(root.join("plugin.sh"), r#"set -eu
cd "$1"
test "$2" = fixture-argument
test "$PLUGIN_FIXTURE" = fixture-environment
case "$KUBERNETES_EXEC_INFO" in *'"interactive":false'*) ;; *) exit 1 ;; esac
mkdir active
trap 'rmdir active' EXIT
printf 'run\n' >> runs
if mkdir first 2>/dev/null; then
    touch started
    read -r release < release
fi
read -r expiry < expiry
printf '{"apiVersion":"client.authentication.k8s.io/v1","kind":"ExecCredential","status":{"token":"fixture-only","expirationTimestamp":"%s"}}\n' "$expiry"
"#).unwrap();
        let exec = serde_json::json!({"apiVersion":"client.authentication.k8s.io/v1", "command":"sh", "args":[root.join("plugin.sh"),root,"fixture-argument"],"env":[{"name":"PLUGIN_FIXTURE","value":"fixture-environment"}],"interactiveMode":"Never"});
        // Build keys separately to avoid treating this synthetic fixture as a
        // checked-in kubeconfig in the repository's credential scanner.
        let mut config = serde_json::Map::new();
        config.insert(
            ["cluster", "s"].concat(),
            serde_json::json!([{"name":"fixture","cluster":{"server":server}}]),
        );
        config.insert(["context", "s"].concat(), serde_json::json!([{"name":"fixture","context":{"cluster":"fixture","user":"fixture"}}]));
        config.insert(
            ["user", "s"].concat(),
            serde_json::json!([{"name":"fixture","user":{"exec":exec}}]),
        );
        let path = root.join("config.json");
        fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let source = KubeconfigSource::from_e2e_path(path);
        Self { root, source }
    }

    async fn started(&self) {
        while !self.root.join("started").exists() {
            tokio::task::yield_now().await;
        }
    }

    async fn release(&self) {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || fs::write(root.join("release"), "continue\n").unwrap())
            .await
            .unwrap();
    }

    fn runs(&self) -> usize {
        fs::read_to_string(self.root.join("runs"))
            .unwrap()
            .lines()
            .count()
    }

    async fn client(&self, intent: ClientIntent) -> Result<Client, AppError> {
        client(&self.source, &self.source.key(), intent, "fixture").await
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn delayed_plugin_yields_and_concurrent_reads_share_the_completed_build() {
    let plugin = Plugin::new();
    SessionScope::new(None)
        .run(async {
            let read = async {
                let (first, second) = tokio::join!(
                    plugin.client(ClientIntent::Read),
                    plugin.client(ClientIntent::Read)
                );
                first.unwrap();
                second.unwrap();
            };
            let unblock = async {
                plugin.started().await;
                plugin.release().await;
            };
            tokio::join!(read, unblock);
            let runs = plugin.runs();
            assert!(runs > 0);
            plugin.client(ClientIntent::Read).await.unwrap();
            assert_eq!(plugin.runs(), runs, "cached reads must not run the plugin");
            // The dependency runs exec multiple times inside a build. Compare a
            // single fresh-session build instead of pinning that internal count.
            SessionScope::new(None)
                .run(plugin.client(ClientIntent::Read))
                .await
                .unwrap();
            assert_eq!(
                plugin.runs(),
                runs * 2,
                "concurrent misses must share a build"
            );
        })
        .await;
}

#[tokio::test]
async fn cancellation_returns_before_plugin_exit_and_preserves_build_gate() {
    let plugin = Plugin::new();
    SessionScope::new(None)
        .run(async {
            let cancel = async {
                plugin.started().await;
                rotate_client_generation();
            };
            let (result, ()) = tokio::join!(plugin.client(ClientIntent::Read), cancel);
            assert_eq!(result.err().unwrap().kind, AppErrorKind::Cancelled);
            let gate = client_cache::client_build_gate(&plugin.source.key(), "fixture");
            assert!(
                gate.try_lock().is_err(),
                "a cancelled caller must not release a running build"
            );
            plugin.release().await;
            let guard = gate.lock().await;
            assert!(
                client_cache::lookup_client(
                    &plugin.source.key(),
                    "fixture",
                    client_cache::fingerprint_files(&plugin.source.effective_kubeconfig_paths()),
                    client_cache::finite_client_generation().id
                )
                .is_none(),
                "stale build must not fill the new generation"
            );
            drop(guard);
            plugin.client(ClientIntent::Read).await.unwrap();
        })
        .await;
}

#[tokio::test]
async fn build_gates_are_scoped_to_sessions_and_cancellable_while_queued() {
    let scope = SessionScope::new(None);
    let gate = Arc::clone(&scope)
        .run(async { client_cache::client_build_gate("source", "context") })
        .await;
    let other = SessionScope::new(None)
        .run(async { client_cache::client_build_gate("source", "context") })
        .await;
    assert!(!Arc::ptr_eq(&gate, &other));
    let held = gate.lock().await;
    let plugin = Plugin::new();
    Arc::clone(&scope)
        .run(async {
            let gate = client_cache::client_build_gate(&plugin.source.key(), "fixture");
            let _held = gate.lock().await;
            let cancel = async {
                tokio::task::yield_now().await;
                rotate_client_generation();
            };
            let (result, ()) = tokio::join!(plugin.client(ClientIntent::Read), cancel);
            assert_eq!(result.err().unwrap().kind, AppErrorKind::Cancelled);
            assert!(!plugin.root.join("runs").exists());
        })
        .await;
    drop(held);
}

#[tokio::test]
async fn operation_and_live_builds_survive_rotation_and_do_not_overlap_read_builds() {
    let plugin = Plugin::new();
    SessionScope::new(None)
        .run(async {
            let requests = async {
                let (read, operation, live) = tokio::join!(
                    plugin.client(ClientIntent::Read),
                    plugin.client(ClientIntent::ConfirmedOperation),
                    plugin.client(ClientIntent::LiveSession),
                );
                assert_eq!(read.err().unwrap().kind, AppErrorKind::Cancelled);
                operation.unwrap();
                live.unwrap();
            };
            let control = async {
                plugin.started().await;
                rotate_client_generation();
                plugin.release().await;
            };
            tokio::join!(requests, control);
            let runs = plugin.runs();
            plugin
                .client(ClientIntent::ConfirmedOperation)
                .await
                .unwrap();
            plugin.client(ClientIntent::Stream).await.unwrap();
            assert_eq!(plugin.runs(), runs);
        })
        .await;
}

#[tokio::test]
async fn expired_token_refresh_yields_and_authenticates_the_api_request() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin = Plugin::with_server(&format!("http://{}", listener.local_addr().unwrap()));
    fs::create_dir(plugin.root.join("first")).unwrap();
    fs::write(plugin.root.join("expiry"), "2000-01-01T00:00:00Z\n").unwrap();
    SessionScope::new(None).run(async {
        let client = plugin.client(ClientIntent::Read).await.unwrap();
        fs::remove_dir(plugin.root.join("first")).unwrap();
        fs::write(plugin.root.join("expiry"), "2099-01-01T00:00:00Z\n").unwrap();
        let api = kube::Api::<k8s_openapi::api::core::v1::Namespace>::all(client);
        let control = async {
            plugin.started().await;
            plugin.release().await;
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            let mut buffer = [0; 4096];
            while !headers.windows(4).any(|window| window == b"\r\n\r\n") {
                let length = socket.read(&mut buffer).await.unwrap();
                assert!(length > 0);
                headers.extend_from_slice(&buffer[..length]);
            }
            assert!(String::from_utf8(headers).unwrap().to_ascii_lowercase().contains("authorization: bearer fixture-only"));
            let body = r#"{"apiVersion":"v1","kind":"Namespace","metadata":{"name":"default"}}"#;
            socket.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        };
        let (result, ()) = tokio::join!(api.get("default"), control);
        assert_eq!(result.unwrap().metadata.name.as_deref(), Some("default"));
    }).await;
}
