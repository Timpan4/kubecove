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
