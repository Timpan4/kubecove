#![cfg(unix)]

use super::*;
use crate::commands::{cluster_access::SourcePolicy, helpers::session_scope::SessionScope};
use crate::models::AppErrorKind;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

// Failure modes: a failing plugin's stdout, arguments or environment reach the
// frontend; its stderr reaches a server session; a failure during token
// refresh, after the client was built, loses the plugin detail.
struct Failing {
    root: PathBuf,
    source: KubeconfigSource,
}

impl Failing {
    fn new(server: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "kubecove-credential-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("plugin.sh"), r#"cd "$1"
if [ -e ok ]; then
    printf '{"apiVersion":"client.authentication.k8s.io/v1","kind":"ExecCredential","status":{"token":"fixture-only","expirationTimestamp":"2000-01-01T00:00:00Z"}}\n'
    exit 0
fi
printf 'fixture-stdout-secret\n'
printf 'opening browser\n' >&2
printf 'error: issuer did not match id_token eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4In0.c2ln\n' >&2
exit 3
"#).unwrap();
        let exec = serde_json::json!({"apiVersion":"client.authentication.k8s.io/v1", "command":"sh", "args":[root.join("plugin.sh"),root,"fixture-argument"],"env":[{"name":"PLUGIN_FIXTURE","value":"fixture-environment"}],"interactiveMode":"Never"});
        // Keys are built separately so the repository's credential scanner
        // does not treat this synthetic fixture as a checked-in kubeconfig.
        let mut config = serde_json::Map::new();
        config.insert(
            ["cluster", "s"].concat(),
            serde_json::json!([{"name":"fixture","cluster":{"server":server}}]),
        );
        config.insert(["context", "s"].concat(), serde_json::json!([{"name":"fixture","context":{"cluster":"fixture","user":"fixture-user"}}]));
        config.insert(
            ["user", "s"].concat(),
            serde_json::json!([{"name":"fixture-user","user":{"exec":exec}}]),
        );
        let path = root.join("config.json");
        fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let source = KubeconfigSource::from_e2e_path(path);
        Self { root, source }
    }

    async fn client(&self) -> Result<Client, AppError> {
        client(
            &self.source,
            &self.source.key(),
            ClientIntent::Read,
            "fixture",
        )
        .await
    }
}

impl Drop for Failing {
    fn drop(&mut self) {
        // A second panic while a failed assertion unwinds would abort the run.
        let _ = fs::remove_dir_all(&self.root);
    }
}

const PRIVATE: [&str; 4] = [
    "fixture-stdout-secret",
    "fixture-argument",
    "fixture-environment",
    "eyJ",
];

fn assert_described(error: &AppError, plugin: &Failing) {
    assert_eq!(error.kind, AppErrorKind::CredentialUnavailable);
    let credential = error.credential.as_deref().expect("local plugin detail");
    assert_eq!(credential.context, "fixture");
    assert_eq!(credential.user.as_deref(), Some("fixture-user"));
    assert_eq!(credential.command, "sh");
    assert_eq!(credential.exit_status, Some(3));
    assert_eq!(
        credential.plugin_message.as_deref(),
        Some("error: issuer did not match id_token [REDACTED]")
    );
    assert_eq!(
        credential.reproduce,
        Some(format!(
            "kubectl --kubeconfig {} --context fixture auth whoami",
            plugin.root.join("config.json").display()
        ))
    );
    assert_eq!(
        credential.kubeconfig,
        [plugin
            .root
            .join("config.json")
            .to_string_lossy()
            .into_owned()]
    );
    let json = serde_json::to_string(error).unwrap();
    for private in PRIVATE {
        assert!(!json.contains(private), "{json}");
    }
}

#[tokio::test]
async fn local_build_failure_names_plugin_and_server_sessions_do_not() {
    let plugin = Failing::new("http://127.0.0.1:1");
    let error = SessionScope::with_source_policy(None, SourcePolicy::Local)
        .run(plugin.client())
        .await
        .err()
        .unwrap();
    assert_described(&error, &plugin);

    let error = SessionScope::new(None)
        .run(plugin.client())
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind, AppErrorKind::CredentialUnavailable);
    assert!(error.credential.is_none());
    let json = serde_json::to_string(&error).unwrap();
    for private in PRIVATE
        .iter()
        .chain(&["issuer did not match", "opening browser", "plugin.sh"])
    {
        assert!(!json.contains(private), "{json}");
    }
}

#[tokio::test]
async fn failed_token_refresh_after_build_names_plugin() {
    let plugin = Failing::new("http://127.0.0.1:1");
    fs::write(plugin.root.join("ok"), "").unwrap();
    SessionScope::with_source_policy(None, SourcePolicy::Local)
        .run(async {
            let client = plugin.client().await.unwrap();
            fs::remove_file(plugin.root.join("ok")).unwrap();
            let api = kube::Api::<k8s_openapi::api::core::v1::Namespace>::all(client);
            let error = AppError::from(api.get("default").await.unwrap_err());
            assert_described(&error, &plugin);
        })
        .await;
}
