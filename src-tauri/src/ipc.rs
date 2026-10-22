//! Tauri IPC adapter for the command contract (ADR 0025). Every declared command becomes one
//! `#[tauri::command]` that decodes its declared keys with `CommandArgs`, so Tauri never
//! derives argument keys from Rust parameter names.

use crate::{
    commands::{
        self, AppUsageMonitor, ArgoConnectionStore, BackendCancellationRegistry, ClusterLiveStore,
        PodExecRegistry, PortForwardRegistry, RemoteConnectionStore, StreamRegistry,
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
};
use serde::Serialize;
use serde_json::Value;
use tauri::{
    ipc::{Channel, Invoke, InvokeBody, JavaScriptChannelId, Request, Response},
    Manager, Runtime, State, Webview,
};

fn request_args<'a>(request: &'a Request<'_>) -> Result<CommandArgs<'a>, AppError> {
    match request.body() {
        InvokeBody::Json(payload) => CommandArgs::new(payload),
        InvokeBody::Raw(_) => Err(AppError::new(
            "command arguments must be an object",
            AppErrorKind::Validation,
        )),
    }
}

fn managed<T: Send + Sync + 'static, R: Runtime>(
    webview: &Webview<R>,
) -> Result<State<'_, T>, AppError> {
    webview
        .try_state::<T>()
        .ok_or_else(|| AppError::new("command state is not managed", AppErrorKind::Internal))
}

fn channel<T, R: Runtime>(
    args: CommandArgs<'_>,
    webview: &Webview<R>,
) -> Result<Channel<T>, AppError> {
    Ok(args
        .get::<JavaScriptChannelId>("channel")?
        .channel_on(webview.clone()))
}

fn respond<T: Serialize>(value: &T) -> Result<Response, AppError> {
    serde_json::to_string(value)
        .map(Response::new)
        .map_err(|error| {
            AppError::new(
                "failed to serialize command result",
                AppErrorKind::Serialization,
            )
            .with_source(error)
        })
}

// Commands without wire arguments or host parameters still validate their payload, so their
// `args` or `webview` binding can be unused.
macro_rules! ipc_command {
    (async $name:ident($request:ident, $webview:ident) $body:block) => {
        #[tauri::command]
        #[allow(unused_variables)]
        async fn $name<R: Runtime>(
            $request: Request<'_>,
            $webview: Webview<R>,
        ) -> Result<Response, AppError> $body
    };
    (sync $name:ident($request:ident, $webview:ident) $body:block) => {
        #[tauri::command]
        #[allow(unused_variables)]
        fn $name<R: Runtime>(
            $request: Request<'_>,
            $webview: Webview<R>,
        ) -> Result<Response, AppError> $body
    };
}

macro_rules! host_param {
    ($args:ident, $webview:ident; State<$state:ty>) => {
        managed::<$state, _>(&$webview)?
    };
    ($args:ident, $webview:ident; Channel) => {
        channel($args, &$webview)?
    };
    ($args:ident, $webview:ident; App) => {
        $webview.app_handle().clone()
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

macro_rules! tauri_ipc {
    ($(
        $transport:ident $mode:ident fn $name:ident($($key:ident: $ty:ty),* $(,)?)
            $(with $($host:ident $(<$state:ty>)?),+)?;
    )*) => {
        $(
            ipc_command!($mode $name(request, webview) {
                let args = request_args(&request)?;
                respond(&call!($mode commands::$name(
                    $(args.get::<$ty>(stringify!($key))?,)*
                    $($(host_param!(args, webview; $host $(<$state>)?),)+)?
                ))?)
            });
        )*

        /// Handles every declared command over Tauri IPC.
        pub(crate) fn invoke_handler<R: Runtime>(
        ) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$($name),*]
        }
    };
}

kubecove_models::command_contract!(tauri_ipc);
