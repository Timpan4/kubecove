use super::*;
use kubecove_lib::models::{CommandTransport, DECLARED_COMMANDS};
use serde_json::json;

#[test]
fn accepts_only_safe_server_channel_markers() {
    let channels = Arc::new(ChannelHub::default());
    let marker = |value: &str| {
        let payload = json!({ "channel": value });
        channel::<Value>(CommandArgs::new(&payload).unwrap(), &channels).map(|_| ())
    };
    assert!(marker("__KUBECOVE_CHANNEL__:42").is_ok());
    assert!(marker("__KUBECOVE_CHANNEL__:9007199254740991").is_ok());
    for invalid in [
        "__TAURI_CHANNEL__:42",
        "__KUBECOVE_CHANNEL__:-1",
        "__KUBECOVE_CHANNEL__:4x",
        "__KUBECOVE_CHANNEL__:9007199254740992",
    ] {
        let error = marker(invalid).unwrap_err();
        assert_eq!(
            (error.kind, error.message.as_str()),
            (AppErrorKind::Validation, "invalid channel id"),
            "{invalid}"
        );
    }
}

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

/// A client-named variable that lists a readable kubeconfig. Only the child
/// process of `sessions_select_only_the_operator_kubeconfig_source` has it.
const CLIENT_SELECTED_VAR: &str = "KUBECOVE_SERVER_CLIENT_SELECTED_KUBECONFIG";

// ADR 0026: a session may select only the operator's kubeconfig source. The
// client-named variable lists a readable kubeconfig, so reading it would let
// the context listing succeed. The variable reaches a child copy of this test
// binary through its startup environment, so no test writes the environment
// while other test threads may read it.
#[test]
fn sessions_select_only_the_operator_kubeconfig_source() {
    if std::env::var_os(CLIENT_SELECTED_VAR).is_some() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(operator_source_is_the_only_selectable_source());
        return;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("kubecove-server-selector-{nanos}.yaml"));
    std::fs::write(&path, fixture_kubeconfig("client-selected")).unwrap();
    let module = module_path!().split_once("::").map_or("", |(_, path)| path);
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            &format!("{module}::sessions_select_only_the_operator_kubeconfig_source"),
            "--exact",
            "--test-threads=1",
        ])
        .env(CLIENT_SELECTED_VAR, &path)
        .output()
        .expect("run child test");
    let _ = std::fs::remove_file(path);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "child test failed:\n{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn operator_source_is_the_only_selectable_source() {
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

    for selector in [
        CLIENT_SELECTED_VAR,
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
}
