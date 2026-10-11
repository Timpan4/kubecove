use crate::{backend::BackendState, channels::ChannelHub};
use kubecove_lib::{
    models::{AppError, AppErrorKind},
    runtime::Channel,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::sync::Arc;

macro_rules! command_table {
    ($state:ident, $channels:ident, $args:ident; $($name:ident => $call:expr),+ $(,)?) => {
        pub(super) const COMMANDS: &[&str] = &[$(stringify!($name)),+];

        pub(super) async fn dispatch(
            $state: &BackendState,
            $channels: &Arc<ChannelHub>,
            command: &str,
            $args: &Value,
        ) -> Result<Option<Value>, AppError> {
            match command {
                $(stringify!($name) => serialize($call).map(Some),)+
                _ => Ok(None),
            }
        }
    };
}

mod argo;
mod inspection;
mod live;

pub const DESKTOP_ONLY: &[&str] = &[
    "pick_kubeconfig_paths",
    "save_workspace_export_json",
    "pick_workspace_import_json",
    "remote_connect",
    "remote_disconnect",
    "remote_connection_status",
    "remote_invoke",
    "remote_subscribe_channel",
    "remote_unsubscribe_channel",
];

pub async fn dispatch(
    state: &BackendState,
    channels: &Arc<ChannelHub>,
    command: &str,
    args: &Value,
) -> Result<Value, AppError> {
    state
        .scoped(Box::pin(dispatch_unscoped(state, channels, command, args)))
        .await
}

async fn dispatch_unscoped(
    state: &BackendState,
    channels: &Arc<ChannelHub>,
    command: &str,
    args: &Value,
) -> Result<Value, AppError> {
    if !args.is_object() {
        return Err(AppError::new(
            "command arguments must be an object",
            AppErrorKind::Validation,
        ));
    }
    let result = if inspection::COMMANDS.contains(&command) {
        inspection::dispatch(state, channels, command, args).await?
    } else if argo::COMMANDS.contains(&command) {
        argo::dispatch(state, channels, command, args).await?
    } else if live::COMMANDS.contains(&command) {
        live::dispatch(state, channels, command, args).await?
    } else {
        None
    };
    if let Some(value) = result {
        return Ok(value);
    }
    Err(AppError::new(
        if DESKTOP_ONLY.contains(&command) {
            "this command requires the desktop client"
        } else {
            "unknown command"
        },
        AppErrorKind::UnsupportedOperation,
    ))
}

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, AppError> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null)).map_err(|_| {
        AppError::new(
            format!("missing or invalid argument: {key}"),
            AppErrorKind::Validation,
        )
    })
}

fn channel<T: Serialize + Send + 'static>(
    args: &Value,
    key: &str,
    channels: &Arc<ChannelHub>,
) -> Result<Channel<T>, AppError> {
    let id = args
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| value.strip_prefix("__KUBECOVE_CHANNEL__:"))
        .and_then(|value| value.parse::<u64>().ok())
        // JavaScript channel ids must remain exactly representable.
        .filter(|id| *id < (1_u64 << 53))
        .ok_or_else(|| AppError::new("invalid channel id", AppErrorKind::Validation))?;
    Ok(channels.channel(id))
}

fn serialize<T: Serialize>(value: T) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|_| {
        AppError::new(
            "failed to serialize command result",
            AppErrorKind::Serialization,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn every_desktop_command_has_an_explicit_transport_decision() {
        let source = include_str!("../../../src/lib.rs");
        let names = source
            .split("tauri::generate_handler![")
            .nth(1)
            .unwrap()
            .split("])")
            .next()
            .unwrap()
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect::<BTreeSet<_>>();
        let all = inspection::COMMANDS
            .iter()
            .chain(argo::COMMANDS)
            .chain(live::COMMANDS)
            .chain(DESKTOP_ONLY)
            .copied()
            .collect::<Vec<_>>();
        let classified = all.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(
            all.len(),
            classified.len(),
            "duplicate command classification"
        );
        assert_eq!(names, classified);
    }

    #[tokio::test]
    async fn native_dialogs_unknown_commands_and_malformed_payloads_are_rejected() {
        let state = BackendState::default();
        let channels = Arc::new(ChannelHub::default());
        for command in DESKTOP_ONLY.iter().copied().chain(["run_shell"]) {
            let error = dispatch(&state, &channels, command, &json!({}))
                .await
                .unwrap_err();
            assert_eq!(error.kind, AppErrorKind::UnsupportedOperation);
        }
        let error = dispatch(
            &state,
            &channels,
            "add_kubeconfig_paths",
            &json!({"paths": 42}),
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
        let error = dispatch(&state, &channels, "get_backend_diagnostics", &Value::Null)
            .await
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
    }

    #[tokio::test]
    async fn camel_case_required_arguments_match_the_desktop_contract() {
        let state = BackendState::default();
        let channels = Arc::new(ChannelHub::default());
        let error = dispatch(
            &state,
            &channels,
            "stop_port_forward",
            &json!({"session_id": "x"}),
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert_eq!(
            dispatch(
                &state,
                &channels,
                "stop_port_forward",
                &json!({"sessionId": "x"})
            )
            .await
            .unwrap(),
            json!(false)
        );
    }
}
