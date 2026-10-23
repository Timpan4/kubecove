//! The command contract shared by Tauri IPC and server HTTP (ADR 0025).

use crate::{AppError, AppErrorKind};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Declares every backend command once and passes the list to `$adapter`.
///
/// Each entry is `transport mode fn name(wireKey: Type, ...) with HostParam, ...;`:
///
/// - `transport` is `both` (Tauri IPC and server HTTP), `remote_intercepted` (both, and the
///   desktop remote client runs its own handler on the laptop instead of forwarding the call
///   unchanged) or `desktop_only`.
/// - `mode` is `async` or `sync`. Synchronous commands keep Tauri's main-thread execution.
/// - `name` is the command name and the function in the shared `commands` API.
/// - Wire arguments use the exact camelCase key the frontend sends, in call order.
/// - Host parameters follow the wire arguments in call order: `State<T>` for managed state,
///   `Channel` for the stream channel sent under the `channel` key, and `App` for the
///   desktop app handle.
///
/// Types are resolved where an adapter expands the contract.
#[macro_export]
macro_rules! command_contract {
    ($adapter:ident) => {
        $adapter! {
            both sync fn list_kube_contexts(kubeconfigEnvVar: Option<String>);
            both async fn list_namespaces(clusterContext: String, kubeconfigEnvVar: Option<String>)
                with State<ClusterLiveStore>;
            both async fn list_resource_kinds(clusterContext: String, kubeconfigEnvVar: Option<String>)
                with State<ClusterLiveStore>;
            both async fn list_present_custom_resource_kinds(
                clusterContext: String,
                namespaces: Vec<String>,
                kubeconfigEnvVar: Option<String>,
            ) with State<ClusterLiveStore>;
            both async fn list_resources(
                clusterContext: String,
                kind: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
            ) with State<ClusterLiveStore>;
            both async fn list_deployment_revisions(
                clusterContext: String,
                name: String,
                namespace: String,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn list_dynamic_resources(
                clusterContext: String,
                resourceKind: DiscoveredResourceKind,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
            ) with State<ClusterLiveStore>;
            both async fn list_resource_scope(
                clusterContext: String,
                requests: Vec<ResourceListRequest>,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<ClusterLiveStore>, State<BackendCancellationRegistry>;
            both async fn list_resource_topology(
                clusterContext: String,
                namespaces: Vec<String>,
                mode: Option<String>,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<ClusterLiveStore>, State<BackendCancellationRegistry>;
            both async fn list_resource_metrics(
                clusterContext: String,
                namespaces: Vec<String>,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn list_resource_events(
                clusterContext: String,
                kind: String,
                name: String,
                namespace: Option<String>,
                uid: Option<String>,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn get_resource_yaml(
                clusterContext: String,
                kind: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn reveal_secret_data_value(
                clusterContext: String,
                name: String,
                namespace: String,
                key: String,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn get_resource_details(
                clusterContext: String,
                kind: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn get_dynamic_resource_details(
                clusterContext: String,
                resourceKind: DiscoveredResourceKind,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn prepare_yaml_apply(request: YamlApplyRequest);
            both async fn apply_yaml(request: YamlApplyRequest);
            both async fn lint_kubernetes_yaml(request: YamlApplyRequest);
            both async fn get_resource_action_state(request: ResourceActionStateRequest);
            both async fn preview_resource_action(request: ResourceActionRequest);
            both async fn execute_resource_action(request: ResourceActionRequest);
            both async fn detect_argocd(clusterContext: String, kubeconfigEnvVar: Option<String>);
            both async fn list_argocd_applications(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn get_argocd_application_details(
                clusterContext: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
            );
            both async fn list_argocd_appsets(clusterContext: String, kubeconfigEnvVar: Option<String>);
            both async fn list_argocd_appprojects(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn get_argocd_appset_details(
                clusterContext: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
            );
            both async fn get_argocd_appproject_details(
                clusterContext: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
            );
            both async fn discover_argo_servers(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn connect_argo_server(
                id: String,
                serverUrl: String,
                endpoint: Option<ArgoServerEndpoint>,
                token: Option<String>,
                username: Option<String>,
                password: Option<String>,
                insecureTls: bool,
                customCaPem: Option<Vec<u8>>,
                rememberCredential: bool,
                clusterContext: Option<String>,
                kubeconfigEnvVar: Option<String>,
                workspaceId: Option<String>,
                confirmedTarget: Option<ArgoConfirmedTarget>,
            ) with State<ArgoConnectionStore>;
            both sync fn get_argo_connection_status(id: String) with State<ArgoConnectionStore>;
            both async fn disconnect_argo_server(id: String) with State<ArgoConnectionStore>;
            both sync fn forget_argo_credential(profile: ArgoConnectionProfile);
            both async fn get_argo_application_inspector(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
                connectionId: Option<String>,
                transport: String,
                application: ArgoApplicationRef,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<ArgoConnectionStore>, State<BackendCancellationRegistry>;
            both async fn get_argo_resource_comparison(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
                connectionId: Option<String>,
                transport: String,
                application: ArgoApplicationRef,
                resource: ArgoManagedResource,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<ArgoConnectionStore>, State<BackendCancellationRegistry>;
            both async fn preflight_argo_operation(request: ArgoOperationRequest)
                with State<ArgoConnectionStore>;
            both async fn run_argo_operation(confirmation: ArgoOperationConfirmation)
                with State<ArgoConnectionStore>;
            both async fn detect_flux(clusterContext: String, kubeconfigEnvVar: Option<String>);
            both async fn list_flux_resources(
                clusterContext: String,
                resourceKind: FluxResourceKind,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn get_flux_resource_details(
                clusterContext: String,
                resourceKind: FluxResourceKind,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
            );
            both async fn list_helm_releases(clusterContext: String, kubeconfigEnvVar: Option<String>);
            both async fn get_helm_release_details(
                clusterContext: String,
                namespace: String,
                storageKind: String,
                storageName: String,
                kubeconfigEnvVar: Option<String>,
                yamlViewMode: Option<YamlViewMode>,
                yamlEncoding: Option<YamlEncoding>,
            );
            both async fn get_helm_release_reconciliation(
                clusterContext: String,
                namespace: String,
                storageKind: String,
                storageName: String,
                kubeconfigEnvVar: Option<String>,
            );
            both async fn list_incident_cockpit(
                clusterContext: String,
                requests: Vec<ResourceListRequest>,
                kubeconfigEnvVar: Option<String>,
            ) with State<ClusterLiveStore>;
            both async fn list_rbac_inspection(
                clusterContext: String,
                kubeconfigEnvVar: Option<String>,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn review_rbac_access(
                request: RbacAccessReviewRequest,
                requestId: Option<String>,
                cancelScope: Option<String>,
            ) with State<BackendCancellationRegistry>;
            both async fn start_resource_watch(
                clusterContext: String,
                keys: Vec<WatchResourceKey>,
                kubeconfigEnvVar: Option<String>,
            ) with Channel, State<StreamRegistry>, State<ClusterLiveStore>;
            both async fn refresh_resource_cache(
                clusterContext: String,
                keys: Vec<WatchResourceKey>,
                namespaces: Vec<String>,
                kubeconfigEnvVar: Option<String>,
            ) with State<ClusterLiveStore>;
            both async fn start_resource_event_watch(
                clusterContext: String,
                kind: String,
                name: String,
                namespace: Option<String>,
                kubeconfigEnvVar: Option<String>,
            ) with Channel, State<StreamRegistry>;
            both async fn start_pod_log_stream(request: PodLogStreamRequest)
                with Channel, State<StreamRegistry>;
            both async fn start_aggregated_log_stream(request: AggregatedLogStreamRequest)
                with Channel, State<StreamRegistry>;
            both async fn stop_stream(streamId: String) with State<StreamRegistry>;
            remote_intercepted async fn start_pod_port_forward(request: PortForwardRequest)
                with State<PortForwardRegistry>;
            remote_intercepted async fn stop_port_forward(sessionId: String)
                with State<PortForwardRegistry>;
            remote_intercepted async fn list_port_forwards() with State<PortForwardRegistry>;
            both async fn start_pod_exec_session(request: PodExecSessionRequest)
                with Channel, State<PodExecRegistry>;
            both async fn write_pod_exec_stdin(sessionId: String, data: String)
                with State<PodExecRegistry>;
            both async fn resize_pod_exec_terminal(sessionId: String, size: PodExecTerminalSize)
                with State<PodExecRegistry>;
            both async fn stop_pod_exec_session(sessionId: String) with State<PodExecRegistry>;
            both async fn list_pod_exec_sessions() with State<PodExecRegistry>;
            remote_intercepted async fn stop_live_sessions_outside_scope(
                request: LiveSessionCleanupRequest,
            ) with State<PortForwardRegistry>, State<PodExecRegistry>;
            both async fn get_app_usage_metrics() with State<AppUsageMonitor>;
            both sync fn get_kubeconfig_sources();
            both sync fn set_kubeconfig_env_var(envVar: String);
            both sync fn set_show_kubeconfig_source_labels(show: bool);
            both sync fn add_kubeconfig_paths(paths: Vec<String>);
            both sync fn remove_kubeconfig_path(path: String);
            both sync fn reorder_kubeconfig_paths(paths: Vec<String>);
            both sync fn set_backend_diagnostics_enabled(enabled: bool);
            both sync fn get_backend_diagnostics();
            both sync fn get_backend_cache_diagnostics() with State<ClusterLiveStore>;
            both sync fn clear_backend_diagnostics() with State<ClusterLiveStore>;
            both sync fn cancel_backend_requests(cancelScope: String)
                with State<BackendCancellationRegistry>;
            both async fn cancel_workspace_requests()
                with State<BackendCancellationRegistry>, State<ClusterLiveStore>, State<ArgoConnectionStore>;
            desktop_only sync fn pick_kubeconfig_paths() with App;
            desktop_only sync fn save_workspace_export_json(fileName: String, content: String)
                with App;
            desktop_only sync fn pick_workspace_import_json() with App;
            desktop_only async fn remote_connect(serverUrl: String, token: String)
                with State<RemoteConnectionStore>;
            desktop_only async fn remote_disconnect() with State<RemoteConnectionStore>;
            desktop_only async fn remote_connection_status() with State<RemoteConnectionStore>;
            desktop_only async fn remote_invoke(command: String, args: Value)
                with State<RemoteConnectionStore>;
            desktop_only async fn remote_subscribe_channel(channelId: u64)
                with Channel, State<RemoteConnectionStore>;
            desktop_only async fn remote_unsubscribe_channel(channelId: u64)
                with State<RemoteConnectionStore>;
        }
    };
}

/// Where a declared command is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTransport {
    /// Tauri IPC and server HTTP.
    Both,
    /// Tauri IPC and server HTTP; the desktop remote client handles it locally.
    RemoteIntercepted,
    /// Tauri IPC only.
    DesktopOnly,
}

/// One entry of [`DECLARED_COMMANDS`].
#[derive(Debug)]
pub struct DeclaredCommand {
    pub name: &'static str,
    pub transport: CommandTransport,
    /// Wire argument keys in call order.
    pub arguments: &'static [&'static str],
}

macro_rules! command_transport {
    (both) => {
        CommandTransport::Both
    };
    (remote_intercepted) => {
        CommandTransport::RemoteIntercepted
    };
    (desktop_only) => {
        CommandTransport::DesktopOnly
    };
}

macro_rules! declared_commands {
    ($(
        $transport:ident $mode:ident fn $name:ident($($key:ident: $ty:ty),* $(,)?)
            $(with $($host:ident $(<$state:ty>)?),+)?;
    )*) => {
        /// Every declared command, in declaration order.
        pub const DECLARED_COMMANDS: &[DeclaredCommand] = &[$(
            DeclaredCommand {
                name: stringify!($name),
                transport: command_transport!($transport),
                arguments: &[$(stringify!($key)),*],
            },
        )*];
    };
}

command_contract!(declared_commands);

/// Named arguments of one command invocation. Every adapter decodes through this type, so a
/// declared key means the same thing in each transport.
#[derive(Debug, Clone, Copy)]
pub struct CommandArgs<'a> {
    values: &'a Map<String, Value>,
}

impl<'a> CommandArgs<'a> {
    pub fn new(payload: &'a Value) -> Result<Self, AppError> {
        payload
            .as_object()
            .map(|values| Self { values })
            .ok_or_else(|| {
                AppError::new(
                    "command arguments must be an object",
                    AppErrorKind::Validation,
                )
            })
    }

    /// Decodes one argument. A missing key reads as null, so an optional argument becomes
    /// `None` and a required one fails.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, AppError> {
        T::deserialize(self.values.get(key).unwrap_or(&Value::Null)).map_err(|error| {
            AppError::new(
                format!("missing or invalid argument: {key}"),
                AppErrorKind::Validation,
            )
            .with_source(error)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn declared_keys_use_the_camel_case_form_the_frontend_sends() {
        let mut names = BTreeSet::new();
        for command in DECLARED_COMMANDS {
            assert!(
                names.insert(command.name),
                "{} is declared twice",
                command.name
            );
            let mut keys = BTreeSet::new();
            for key in command.arguments {
                assert!(
                    keys.insert(key),
                    "{}: {key} is declared twice",
                    command.name
                );
                let mut chars = key.chars();
                assert!(
                    chars.next().is_some_and(|first| first.is_ascii_lowercase())
                        && chars.all(|char| char.is_ascii_alphanumeric()),
                    "{}: {key} is not lowerCamel",
                    command.name
                );
            }
        }
    }

    #[test]
    fn arguments_decode_missing_optional_as_none_and_reject_the_rest() {
        let payload = json!({"clusterContext": "kind-dev", "replicas": "three"});
        let args = CommandArgs::new(&payload).unwrap();
        assert_eq!(args.get::<String>("clusterContext").unwrap(), "kind-dev");
        assert_eq!(
            args.get::<Option<String>>("kubeconfigEnvVar").unwrap(),
            None
        );
        for (key, error) in [
            ("namespace", args.get::<String>("namespace").unwrap_err()),
            ("replicas", args.get::<Option<i32>>("replicas").unwrap_err()),
        ] {
            assert_eq!(error.kind, AppErrorKind::Validation);
            assert_eq!(error.message, format!("missing or invalid argument: {key}"));
        }
        for payload in [json!(null), json!([]), json!("clusterContext")] {
            assert_eq!(
                CommandArgs::new(&payload).unwrap_err().kind,
                AppErrorKind::Validation
            );
        }
    }
}
