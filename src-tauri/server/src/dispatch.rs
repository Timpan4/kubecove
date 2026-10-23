//! Server HTTP adapter for the command contract (ADR 0020, ADR 0025). The declaration in
//! `kubecove_models::command_contract!` is the explicit command table: it routes `both` and
//! `remote_intercepted` commands and rejects `desktop_only` commands before reading arguments.

use crate::{backend::BackendState, channels::ChannelHub};
use kubecove_lib::{
    commands::{
        self, AppUsageMonitor, ArgoConnectionStore, BackendCancellationRegistry, ClusterLiveStore,
        PodExecRegistry, PortForwardRegistry, StreamRegistry,
    },
    models::{
        AggregatedLogStreamRequest, AppError, AppErrorKind, ArgoApplicationRef,
        ArgoConfirmedTarget, ArgoConnectionProfile, ArgoManagedResource, ArgoOperationConfirmation,
        ArgoOperationRequest, ArgoServerEndpoint, CommandArgs, DiscoveredResourceKind,
        FluxResourceKind, LiveSessionCleanupRequest, PodExecSessionRequest, PodExecTerminalSize,
        PodLogStreamRequest, PortForwardRequest, RbacAccessReviewRequest, ResourceActionRequest,
        ResourceActionStateRequest, ResourceListRequest, WatchResourceKey, YamlApplyRequest,
        YamlEncoding, YamlViewMode,
    },
    runtime::{Channel, State},
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

pub async fn dispatch(
    state: &BackendState,
    channels: &Arc<ChannelHub>,
    command: &str,
    args: &Value,
) -> Result<Vec<u8>, AppError> {
    state
        .scoped(Box::pin(dispatch_unscoped(state, channels, command, args)))
        .await
}

fn channel<T: Serialize + Send + 'static>(
    args: CommandArgs<'_>,
    channels: &Arc<ChannelHub>,
) -> Result<Channel<T>, AppError> {
    let id = args
        .get::<String>("channel")?
        .strip_prefix("__KUBECOVE_CHANNEL__:")
        .and_then(|value| value.parse::<u64>().ok())
        // JavaScript channel ids must remain exactly representable.
        .filter(|id| *id < (1_u64 << 53))
        .ok_or_else(|| AppError::new("invalid channel id", AppErrorKind::Validation))?;
    Ok(channels.channel(id))
}

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(value).map_err(|error| {
        AppError::new(
            "failed to serialize command result",
            AppErrorKind::Serialization,
        )
        .with_source(error)
    })
}

fn requires_desktop() -> AppError {
    AppError::new(
        "this command requires the desktop client",
        AppErrorKind::UnsupportedOperation,
    )
}

macro_rules! host_param {
    ($state:ident, $channels:ident, $args:ident; State<$managed:ty>) => {
        State::new(AsRef::<$managed>::as_ref($state))
    };
    ($state:ident, $channels:ident, $args:ident; Channel) => {
        channel($args, $channels)?
    };
}

macro_rules! call {
    (async $call:expr) => {
        $call.await
    };
    (sync $call:expr) => {
        $call
    };
}

// Desktop-only commands are rejected before their arguments are read or their command
// function is named, so the server never links their desktop implementations.
macro_rules! http_command {
    (desktop_only $($ignored:tt)*) => {
        Err(requires_desktop())
    };
    (
        $transport:ident $mode:ident $name:ident($payload:ident, $state:ident, $channels:ident)
        ($($key:ident: $ty:ty),*) $(with $($host:ident $(<$managed:ty>)?),+)?
    ) => {{
        // Commands without wire arguments still reject a non-object payload.
        #[allow(unused_variables)]
        let args = CommandArgs::new($payload)?;
        serialize(&call!($mode commands::$name(
            $(args.get::<$ty>(stringify!($key))?,)*
            $($(host_param!($state, $channels, args; $host $(<$managed>)?),)+)?
        ))?)
    }};
}

macro_rules! http_dispatch {
    ($(
        $transport:ident $mode:ident fn $name:ident($($key:ident: $ty:ty),* $(,)?)
            $(with $($host:ident $(<$managed:ty>)?),+)?;
    )*) => {
        async fn dispatch_unscoped(
            state: &BackendState,
            channels: &Arc<ChannelHub>,
            command: &str,
            payload: &Value,
        ) -> Result<Vec<u8>, AppError> {
            match command {
                $(stringify!($name) => http_command!(
                    $transport $mode $name(payload, state, channels)
                    ($($key: $ty),*) $(with $($host $(<$managed>)?),+)?
                ),)*
                _ => Err(AppError::new(
                    "unknown command",
                    AppErrorKind::UnsupportedOperation,
                )),
            }
        }
    };
}

kubecove_lib::models::command_contract!(http_dispatch);

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
