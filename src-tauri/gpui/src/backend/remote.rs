//! Remote routing never falls back to local credentials after a socket failure.
//! Explicit disconnect is the only transition back to the local backend.
use super::*;
use kubecove_host::remote::{self, RemoteConnectionStore};
use kubecove_lib::models::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
#[path = "remote_inventory.rs"]
mod reads;
#[path = "remote_streams.rs"]
mod streams;
use reads::inventory;
#[path = "remote_contract.rs"]
mod contract;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct DesktopBackend {
    local: NativeBackend,
    remote: Arc<RemoteConnectionStore>,
    selected: Arc<AtomicBool>,
    channel: AtomicU64,
    preferences: Arc<crate::preferences::Store>,
}

impl DesktopBackend {
    pub fn new(local: NativeBackend) -> Self {
        Self {
            local,
            remote: Arc::default(),
            selected: Arc::default(),
            channel: AtomicU64::new(1),
            preferences: Default::default(),
        }
    }
    fn selected(&self) -> bool {
        self.selected.load(Ordering::Acquire)
    }
    fn call<T: DeserializeOwned + Send + 'static>(
        &self,
        command: &'static str,
        args: Value,
    ) -> Read<T> {
        let remote = self.remote.clone();
        self.local.read(invoke(remote, command, args))
    }
    fn scope(
        &self,
        scope: Scope,
        kinds: &'static [&'static str],
        refresh: bool,
    ) -> Read<ScopeResult> {
        let remote = self.remote.clone();
        self.local
            .read(async move { inventory(remote, scope, kinds, refresh).await })
    }
}

async fn invoke<T: DeserializeOwned>(
    remote: Arc<RemoteConnectionStore>,
    command: &str,
    args: Value,
) -> Result<T, AppError> {
    contract::validate(command, &args)?;
    let result = remote::remote_invoke(command.to_owned(), args, State::new(&remote)).await?;
    serde_json::from_value(result).map_err(|_| {
        AppError::new(
            "Invalid remote command response",
            AppErrorKind::Serialization,
        )
    })
}

fn resource_args(resource: &ResourceSummary) -> Value {
    json!({"clusterContext":resource.cluster,"kind":resource.kind,"name":resource.name,"namespace":resource.namespace})
}
fn scope_args(scope: Scope) -> Value {
    json!({"clusterContext":scope.context,"namespaces":scope.namespace.into_iter().collect::<Vec<_>>()})
}

macro_rules! route {
    ($name:ident($($arg:ident: $ty:ty),*) -> $result:ty, $command:literal, $args:expr) => {
        fn $name(&self, $($arg: $ty),*) -> Read<$result> {
            if self.selected() { self.call($command, $args) } else { self.local.$name($($arg),*) }
        }
    };
}

impl Backend for DesktopBackend {
    super::mcp::local!();
    fn diagnostics(&self, enabled: bool, clear: bool) -> Read<super::adapters::BackendDiagnostics> {
        if !self.selected() {
            return self.local.diagnostics(enabled, clear);
        }
        let remote = self.remote.clone();
        self.local.read(async move {
            let _: bool = invoke(
                remote.clone(),
                "set_backend_diagnostics_enabled",
                json!({"enabled": enabled}),
            )
            .await?;
            if clear {
                let _: () = invoke(remote.clone(), "clear_backend_diagnostics", json!({})).await?;
            }
            Ok(super::adapters::BackendDiagnostics {
                events: invoke(remote.clone(), "get_backend_diagnostics", json!({})).await?,
                caches: invoke(remote, "get_backend_cache_diagnostics", json!({})).await?,
            })
        })
    }
    fn open_documentation(&self) -> Read<()> {
        self.local.open_documentation()
    }
    fn reveal_secret(&self, resource: ResourceSummary, key: String) -> Read<String> {
        if !self.selected() {
            return self.local.reveal_secret(resource, key);
        }
        let mut args = resource_args(&resource);
        args["key"] = json!(key);
        args.as_object_mut().expect("resource args").remove("kind");
        self.call("reveal_secret_data_value", args)
    }
    fn load_preferences(&self) -> Result<crate::preferences::Document, AppError> {
        if self.selected() {
            self.preferences.load()
        } else {
            self.local.load_preferences()
        }
    }
    fn save_preferences(
        &self,
        document: crate::preferences::Document,
    ) -> Result<crate::preferences::Document, AppError> {
        if self.selected() {
            self.preferences.save(document)
        } else {
            self.local.save_preferences(document)
        }
    }
    route!(
        preflight_argo(request: ArgoOperationRequest) -> ArgoOperationPreflight,
        "preflight_argo_operation",
        json!({"request":request})
    );
    route!(
        run_argo(confirmation: ArgoOperationConfirmation) -> ArgoOperationResult,
        "run_argo_operation",
        json!({"confirmation":confirmation})
    );
    route!(
        disconnect_argo(id: String) -> (),
        "disconnect_argo_server",
        json!({"id":id})
    );
    route!(
        inspect_argo(request: ArgoInspection) -> ArgoApplicationInspector,
        "get_argo_application_inspector",
        json!({"clusterContext":request.application.context,"application":request.application,"transport":if request.connection_id.is_some() { "connected" } else { "kubernetes" },"connectionId":request.connection_id,"kubeconfigEnvVar":request.source})
    );
    route!(
        connect_argo(request: ArgoConnect) -> ArgoConnectionStatus,
        "connect_argo_server",
        json!({"id":request.profile.id,"serverUrl":request.profile.url,"endpoint":request.profile.endpoint,"token":request.token,
            "username":request.username,"password":request.password,"insecureTls":request.insecure_tls,"customCaPem":request.custom_ca_pem,"confirmedTarget":request.confirmed_target,"kubeconfigEnvVar":request.profile.kubeconfig_source_key,"rememberCredential":request.profile.remember_credential,"clusterContext":request.profile.cluster_context,
            "workspaceId":request.profile.workspace_id})
    );
    route!(
        discover_argo(request: ArgoTarget) -> Vec<ArgoServerCapability>,
        "discover_argo_servers",
        json!({"clusterContext":request.context,"kubeconfigEnvVar":request.source})
    );
    route!(
        preview_argo(request: ArgoTarget) -> ArgoTunnelTargetPreview,
        "preview_argo_tunnel_target",
        json!({"clusterContext":request.context,"kubeconfigEnvVar":request.source,"endpoint":request.endpoint})
    );
    fn add_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.add_kubeconfig_paths(paths)
    }
    fn remote_status(&self) -> Read<Option<RemoteConnectionSummary>> {
        let remote = self.remote.clone();
        self.local
            .read(async move { remote::remote_connection_status(State::new(&remote)).await })
    }
    fn connect_remote(&self, url: String, token: String) -> Read<RemoteConnectionSummary> {
        let remote = self.remote.clone();
        let selected = self.selected.clone();
        let preferences = self.preferences.clone();
        self.local.read(async move {
            let summary = remote::remote_connect(url, token.clone(), State::new(&remote))
                .await
                .map_err(|error| {
                    AppError::new(error.message.replace(&token, "[redacted]"), error.kind)
                })?;
            preferences.clear_memory()?;
            selected.store(true, Ordering::Release);
            Ok(summary)
        })
    }
    fn disconnect_remote(&self) -> Read<()> {
        let remote = self.remote.clone();
        let selected = self.selected.clone();
        let preferences = self.preferences.clone();
        self.local.read(async move {
            remote::remote_disconnect(State::new(&remote)).await?;
            preferences.clear_memory()?;
            selected.store(false, Ordering::Release);
            Ok(())
        })
    }
    route!(contexts() -> Vec<ClusterContext>, "list_kube_contexts", json!({}));
    route!(
        namespaces(context: String) -> Vec<NamespaceSummary>,
        "list_namespaces",
        json!({"clusterContext":context})
    );
    route!(
        resource_kinds(context: String) -> Vec<DiscoveredResourceKind>,
        "list_resource_kinds",
        json!({"clusterContext":context})
    );
    route!(
        namespace_custom_kinds(scope: Scope) -> Vec<DiscoveredResourceKind>,
        "list_present_custom_resource_kinds",
        json!({"clusterContext":scope.context,"namespaces":scope.namespace.into_iter().collect::<Vec<_>>()})
    );
    route!(
        action_state(request: ResourceActionStateRequest) -> ResourceActionState,
        "get_resource_action_state",
        json!({"request":request})
    );
    route!(
        preview_action(request: ResourceActionRequest) -> ResourceActionPreview,
        "preview_resource_action",
        json!({"request":request})
    );
    route!(
        execute_action(request: ResourceActionRequest) -> ClusterOperationResult,
        "execute_resource_action",
        json!({"request":request})
    );
    route!(
        prepare_yaml_apply(request: YamlApplyRequest) -> YamlApplyPreview,
        "prepare_yaml_apply",
        json!({"request":request})
    );
    route!(
        apply_yaml(request: YamlApplyRequest) -> YamlApplyResult,
        "apply_yaml",
        json!({"request":request})
    );
    route!(
        deployment_revisions(resource: ResourceSummary) -> Vec<DeploymentRevision>,
        "list_deployment_revisions",
        json!({"clusterContext":resource.cluster,"name":resource.name,"namespace":resource.namespace})
    );
    route!(
        events(resource: ResourceSummary, uid: Option<String>) -> Vec<ResourceEventSummary>,
        "list_resource_events",
        {
            let mut args = resource_args(&resource);
            args["uid"] = json!(uid);
            args
        }
    );
    route!(
        metrics(scope: Scope) -> ResourceMetricsSummary,
        "list_resource_metrics",
        scope_args(scope)
    );
    route!(app_usage() -> AppUsageMetrics, "get_app_usage_metrics", json!({}));
    route!(
        rbac(context: String) -> RbacInspectionSummary,
        "list_rbac_inspection",
        json!({"clusterContext":context})
    );
    route!(
        review_rbac(request: RbacAccessReviewRequest) -> RbacAccessReviewResult,
        "review_rbac_access",
        json!({"request":request})
    );
    route!(
        helm_releases(context: String) -> HelmReleaseList,
        "list_helm_releases",
        json!({"clusterContext":context})
    );
    route!(
        helm_release(release: HelmReleaseSummary) -> HelmReleaseDetails,
        "get_helm_release_details",
        json!({"clusterContext":release.cluster,"namespace":release.namespace,"storageKind":release.storage_kind,"storageName":release.storage_name})
    );
    route!(
        helm_reconciliation(release: HelmReleaseSummary) -> HelmReleaseReconciliation,
        "get_helm_release_reconciliation",
        json!({"clusterContext":release.cluster,"namespace":release.namespace,"storageKind":release.storage_kind,"storageName":release.storage_name})
    );
    route!(
        argo_inspector(application: ArgoApplicationRef) -> ArgoApplicationInspector,
        "get_argo_application_inspector",
        json!({"clusterContext":application.context.clone().unwrap_or_default(),"application":application,"transport":"kubernetes"})
    );
    route!(
        incidents(context: String) -> IncidentCockpitSummary,
        "list_incident_cockpit",
        json!({"clusterContext":context,"requests":crate::incidents::workspace_requests()})
    );
    route!(kubeconfig_sources() -> KubeconfigSourcesSummary, "get_kubeconfig_sources", json!({}));

    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        if self.selected() {
            self.scope(scope, INSPECTION_KINDS, refresh)
        } else {
            self.local.resources(scope, refresh)
        }
    }
    fn cluster_resources(&self, context: String, refresh: bool) -> Read<ScopeResult> {
        if self.selected() {
            self.scope(
                Scope {
                    context,
                    namespace: None,
                },
                CLUSTER_KINDS,
                refresh,
            )
        } else {
            self.local.cluster_resources(context, refresh)
        }
    }
    fn application_resources(
        &self,
        app: ArgoApplicationSummary,
        refresh: bool,
    ) -> Read<ScopeResult> {
        if !self.selected() {
            return self.local.application_resources(app, refresh);
        }
        let remote = self.remote.clone();
        self.local.read(async move {
            let mut namespaces = app.resource_namespaces;
            if namespaces.is_empty() {
                namespaces.extend(app.destination_namespace);
            }
            namespaces.sort();
            namespaces.dedup();
            let scopes: Vec<_> = if namespaces.is_empty() {
                vec![None]
            } else {
                namespaces.into_iter().map(Some).collect()
            };
            let mut result = ScopeResult {
                namespaces: vec![],
                resources: vec![],
                warnings: vec![],
            };
            for namespace in scopes {
                let next = inventory(
                    remote.clone(),
                    Scope {
                        context: app.cluster.clone(),
                        namespace,
                    },
                    APPLICATION_KINDS,
                    refresh,
                )
                .await?;
                result.namespaces = next.namespaces;
                for row in next.resources {
                    if !result.resources.iter().any(|old| {
                        old.kind == row.kind
                            && old.name == row.name
                            && old.namespace == row.namespace
                    }) {
                        result.resources.push(row);
                    }
                }
                result.warnings.extend(next.warnings);
            }
            Ok(result)
        })
    }
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        if !self.selected() {
            return self.local.details(resource);
        }
        let mut args = resource_args(&resource);
        let command = if resource.dynamic == Some(true) {
            args.as_object_mut().expect("resource args").remove("kind");
            args["resourceKind"] = json!(super::delivery::dynamic_kind(&resource));
            "get_dynamic_resource_details"
        } else {
            "get_resource_details"
        };
        self.call(command, args)
    }
    fn kind_resources(&self, scope: Scope, key: WatchResourceKey) -> Read<Vec<ResourceSummary>> {
        if !self.selected() {
            return self.local.kind_resources(scope, key);
        }
        let kind = key.resource_kind;
        if kind.plural.is_some() {
            self.call("list_dynamic_resources", json!({"clusterContext":scope.context,"resourceKind":kind,"namespace":key.namespace}))
        } else {
            self.call(
                "list_resources",
                json!({"clusterContext":scope.context,"kind":kind.kind,"namespace":key.namespace}),
            )
        }
    }
    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        if !self.selected() {
            return self.local.topology(scope);
        }
        let mut args = scope_args(scope);
        args["mode"] = json!("ownership");
        self.call("list_resource_topology", args)
    }
    fn network_topology(&self, scope: Scope) -> Read<ResourceTopology> {
        if !self.selected() {
            return self.local.network_topology(scope);
        }
        let mut args = scope_args(scope);
        args["mode"] = json!("networkFlow");
        self.call("list_resource_topology", args)
    }
    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        self.resource_yaml_encoded(resource, mode, YamlEncoding::Yaml)
    }
    fn resource_yaml_encoded(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: YamlEncoding,
    ) -> Read<String> {
        if !self.selected() {
            return self.local.resource_yaml_encoded(resource, mode, encoding);
        }
        let mut args = resource_args(&resource);
        args["yamlViewMode"] = json!(mode);
        args["yamlEncoding"] = json!(encoding);
        if resource.dynamic == Some(true) {
            args.as_object_mut().expect("resource args").remove("kind");
            args["resourceKind"] = json!(super::delivery::dynamic_kind(&resource));
            let read: Read<ResourceDetailsFull> = self.call("get_dynamic_resource_details", args);
            Box::pin(async move { Ok(read.await?.yaml) })
        } else {
            self.call("get_resource_yaml", args)
        }
    }
    fn watch(&self, target: WatchTarget) -> Read<LogStream> {
        if !self.selected() {
            return self.local.watch(target);
        }
        match target {
            WatchTarget::Resource { scope, key } => self.stream(
                "start_resource_watch",
                json!({"clusterContext":scope.context,"keys":[key]}),
            ),
            WatchTarget::Events(resource) => {
                self.stream("start_resource_event_watch", resource_args(&resource))
            }
        }
    }
    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        if !self.selected() {
            return self.local.log_stream(target);
        }
        match target {
            LogTarget::Pod(request) => {
                self.stream("start_pod_log_stream", json!({"request":request}))
            }
            LogTarget::Aggregate(request) => {
                self.stream("start_aggregated_log_stream", json!({"request":request}))
            }
        }
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        if !self.selected() {
            return self.local.live_sessions();
        }
        let remote = self.remote.clone();
        self.local.read(async move {
            let sources: KubeconfigSourcesSummary =
                invoke(remote.clone(), "get_kubeconfig_sources", json!({})).await?;
            Ok(LiveSessions {
                port_forwards: invoke(remote.clone(), "list_port_forwards", json!({})).await?,
                exec_sessions: invoke(remote, "list_pod_exec_sessions", json!({})).await?,
                source_key: Some(sources.source_key),
                show_source_labels: sources.show_source_labels,
            })
        })
    }
    fn gitops(&self, context: String) -> Read<GitOpsInventory> {
        if !self.selected() {
            return self.local.gitops(context);
        }
        let remote = self.remote.clone();
        self.local.read(async move {
            let args = json!({"clusterContext":context});
            let mut inventory = GitOpsInventory::default();
            let detected: bool = invoke(remote.clone(), "detect_argocd", args.clone()).await?;
            if detected {
                inventory.argo =
                    Some(invoke(remote.clone(), "list_argocd_applications", args.clone()).await?);
                inventory.app_sets =
                    invoke(remote.clone(), "list_argocd_appsets", args.clone()).await?;
                inventory.projects =
                    invoke(remote.clone(), "list_argocd_appprojects", args.clone()).await?;
            }
            let flux: FluxDetectionSummary = invoke(remote.clone(), "detect_flux", args).await?;
            if flux.detected {
                let mut rows = Vec::new();
                for kind in flux.kinds {
                    rows.extend(
                        invoke::<Vec<FluxResourceSummary>>(
                            remote.clone(),
                            "list_flux_resources",
                            json!({"clusterContext":context,"resourceKind":kind}),
                        )
                        .await?,
                    );
                }
                inventory.flux = Some(rows);
            }
            Ok(inventory)
        })
    }
    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        if self.selected() {
            Some("Kubeconfig sources are set by the server operator")
        } else {
            self.local.kubeconfig_writes_unavailable()
        }
    }
    fn set_kubeconfig_env_var(&self, value: String) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.set_kubeconfig_env_var(value)
    }
    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.set_show_kubeconfig_source_labels(show)
    }
    fn pick_kubeconfig_paths(&self) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.pick_kubeconfig_paths()
    }
    fn remove_kubeconfig_path(&self, path: String) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.remove_kubeconfig_path(path)
    }
    fn reorder_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if self.selected() {
            return settings_integrations::unavailable(
                "Kubeconfig sources are set by the server operator",
            );
        }
        self.local.reorder_kubeconfig_paths(paths)
    }
    fn updates_enabled(&self) -> bool {
        self.local.updates_enabled()
    }
    fn check_updates(&self) -> Read<UpdateStatus> {
        self.local.check_updates()
    }
    fn import_workspace(&self) -> Read<Option<String>> {
        self.local.import_workspace()
    }
    fn export_workspace(&self, name: String, content: String) -> Read<bool> {
        self.local.export_workspace(name, content)
    }
}
