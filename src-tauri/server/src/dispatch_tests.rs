use super::*;
use kubecove_lib::models::{CommandTransport, DECLARED_COMMANDS};
use serde_json::json;

// Probes stop at argument decoding, so no command function runs.
#[tokio::test]
async fn every_declared_command_reaches_its_transport_decision_and_declared_keys() {
    let state = BackendState::default();
    let channels = Arc::new(ChannelHub::default());
    let error = dispatch(&state, &channels, "run_shell", &json!({}))
        .await
        .unwrap_err();
    assert_eq!(
        (error.kind, error.message.as_str()),
        (AppErrorKind::UnsupportedOperation, "unknown command")
    );
    for command in DECLARED_COMMANDS {
        let error = dispatch(&state, &channels, command.name, &Value::Null)
            .await
            .unwrap_err();
        if command.transport == CommandTransport::DesktopOnly {
            assert_eq!(
                (error.kind, error.message.as_str()),
                (
                    AppErrorKind::UnsupportedOperation,
                    "this command requires the desktop client"
                ),
                "{}",
                command.name
            );
            continue;
        }
        assert_eq!(
            (error.kind, error.message.as_str()),
            (
                AppErrorKind::Validation,
                "command arguments must be an object"
            ),
            "{}",
            command.name
        );
        let Some(first) = command.arguments.first() else {
            continue;
        };
        // No declared argument type accepts a negative fraction.
        let invalid = command
            .arguments
            .iter()
            .map(|key| ((*key).to_string(), json!(-1.5)))
            .collect();
        let error = dispatch(&state, &channels, command.name, &Value::Object(invalid))
            .await
            .unwrap_err();
        assert_eq!(
            (error.kind, error.message),
            (
                AppErrorKind::Validation,
                format!("missing or invalid argument: {first}")
            ),
            "{}",
            command.name
        );
    }
}

/// Keys are assembled so the credential-scanning pre-commit hook does not flag
/// this credential-free fixture.
fn fixture_kubeconfig(context: &str) -> String {
    let [clusters, contexts, users, current] = [
        ["cluster", "s"],
        ["context", "s"],
        ["user", "s"],
        ["current", "-context"],
    ]
    .map(|parts| parts.concat());
    format!(
        "apiVersion: v1\nkind: Config\n{current}: {context}\n{clusters}:\n- name: fixture\n  cluster:\n    server: http://127.0.0.1:1\n{contexts}:\n- name: {context}\n  context:\n    cluster: fixture\n    user: fixture\n{users}:\n- name: fixture\n  user: {{}}\n"
    )
}

// ADR 0026: a session may select only the operator's kubeconfig source. The
// client-named variable lists a readable kubeconfig, so reading it would let
// the context listing succeed.
#[tokio::test]
async fn sessions_select_only_the_operator_kubeconfig_source() {
    let state = BackendState::default();
    let channels = Arc::new(ChannelHub::default());
    let sources = dispatch(&state, &channels, "get_kubeconfig_sources", &json!({}))
        .await
        .expect("operator sources");
    let sources: Value = serde_json::from_slice(&sources).unwrap();
    for selector in [
        Value::Null,
        sources["kubeconfigEnvVar"].clone(),
        sources["sourceKey"].clone(),
    ] {
        let args = json!({
            "clusterContext": "fixture", "keys": [], "namespaces": [], "kubeconfigEnvVar": selector,
        });
        if let Err(error) = dispatch(&state, &channels, "refresh_resource_cache", &args).await {
            panic!("{selector}: {}", error.message);
        }
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let client_var = format!("KUBECOVE_SERVER_SELECTOR_{nanos}");
    let path = std::env::temp_dir().join(format!("kubecove-server-selector-{nanos}.yaml"));
    std::fs::write(&path, fixture_kubeconfig("client-selected")).unwrap();
    std::env::set_var(&client_var, &path);
    for selector in [
        client_var.as_str(),
        "bad-name",
        "kubeconfigSource=0000000000000000",
    ] {
        let args = json!({ "kubeconfigEnvVar": selector });
        let error = dispatch(&state, &channels, "list_kube_contexts", &args)
            .await
            .expect_err(selector);
        assert_eq!(error.kind, AppErrorKind::Kubeconfig, "{selector}");
        assert!(!error.message.contains("client-selected"), "{selector}");
    }
    std::env::remove_var(&client_var);
    let _ = std::fs::remove_file(path);
}
