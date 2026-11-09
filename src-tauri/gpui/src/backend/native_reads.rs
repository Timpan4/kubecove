//! The `Backend` implementation for the desktop client: each read runs on the
//! native runtime through the same `kubecove_lib` commands the Tauri app uses.

use super::*;

impl Backend for NativeBackend {
    fn diagnostics(&self, enabled: bool, clear: bool) -> Read<adapters::BackendDiagnostics> {
        adapters::diagnostics(self, enabled, clear)
    }
    fn open_documentation(&self) -> Read<()> {
        adapters::open_documentation(self)
    }
    fn reveal_secret(&self, resource: ResourceSummary, key: String) -> Read<String> {
        adapters::reveal_secret(self, resource, key)
    }
    fn load_preferences(&self) -> Result<crate::preferences::Document, AppError> {
        self.preferences.load()
    }
    fn save_preferences(
        &self,
        document: crate::preferences::Document,
    ) -> Result<crate::preferences::Document, AppError> {
        self.preferences.save(document)
    }
    argo::native!();
    mcp::native!();
    fn add_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::add_kubeconfig_paths(paths) })
    }
    fn start_forward(
        &self,
        request: kubecove_lib::models::PortForwardRequest,
    ) -> Read<kubecove_lib::models::PortForwardSessionSummary> {
        session_controls::start_forward(self, request)
    }
    fn stop_forward(&self, id: String) -> Read<bool> {
        session_controls::stop_forward(self, id)
    }
    fn exec(&self, request: kubecove_lib::models::PodExecSessionRequest) -> Read<ExecStream> {
        session_controls::exec(self, request)
    }
    fn exec_input(&self, id: String, data: String) -> Read<bool> {
        session_controls::input(self, id, data)
    }
    fn exec_resize(
        &self,
        id: String,
        size: kubecove_lib::models::PodExecTerminalSize,
    ) -> Read<bool> {
        session_controls::resize(self, id, size)
    }
    fn stop_exec(&self, id: String) -> Read<bool> {
        session_controls::stop_exec(self, id)
    }
    fn cleanup_sessions(
        &self,
        request: kubecove_lib::models::LiveSessionCleanupRequest,
    ) -> Read<kubecove_lib::models::LiveSessionCleanupResult> {
        session_controls::cleanup(self, request)
    }
    fn resource_kinds(
        &self,
        context: String,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        live_data::resource_kinds(self, context)
    }
    fn namespace_custom_kinds(
        &self,
        scope: Scope,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        live_data::namespace_custom_kinds(self, scope)
    }
    fn kind_resources(
        &self,
        scope: Scope,
        key: WatchResourceKey,
        recycled: Vec<ResourceSummary>,
    ) -> Read<KindRows> {
        live_data::kind_resources(self, scope, key, recycled)
    }
    fn named_kind_resources(
        &self,
        scope: Scope,
        key: WatchResourceKey,
        names: Vec<(Option<String>, String)>,
        recycled: Vec<ResourceSummary>,
    ) -> Read<KindRows> {
        live_data::named_kind_resources(self, scope, key, names, recycled)
    }
    fn watch(&self, target: WatchTarget) -> Read<LogStream> {
        live_data::watch(self, target)
    }

    fn contexts(&self, kubeconfig_source_key: Option<String>) -> Read<Vec<ClusterContext>> {
        self.read(async move { commands::list_kube_contexts(kubeconfig_source_key) })
    }

    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        live_data::resources(self, scope, refresh)
    }

    fn scope_builtin_resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        live_data::builtin_resources(self, scope, refresh)
    }

    fn scope_custom_resources(&self, scope: Scope) -> Option<Read<ScopeResult>> {
        Some(live_data::custom_resources(self, scope))
    }

    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let cancellations = self.cancellations.clone();
        if let Some(kind) =
            delivery::dynamic_kind(&resource).filter(|_| resource.dynamic == Some(true))
        {
            return self.read(delivery::dynamic_details(resource, kind, cancellations));
        }
        self.read(async move {
            supported(&resource.kind)?;
            commands::get_resource_details(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn application_resources(
        &self,
        application: ArgoApplicationSummary,
        refresh: bool,
    ) -> Read<ScopeResult> {
        self.read(delivery_workspace::resources(
            application,
            refresh,
            self.store.clone(),
        ))
    }

    fn topology(&self, scope: Scope) -> Read<Arc<ResourceTopology>> {
        topology_metrics::topology(self, scope, "ownership")
    }

    fn network_topology(&self, scope: Scope) -> Read<Arc<ResourceTopology>> {
        topology_metrics::topology(self, scope, "networkFlow")
    }

    fn app_usage(&self) -> Read<kubecove_lib::models::AppUsageMetrics> {
        topology_metrics::app_usage(self)
    }

    fn metrics(&self, scope: Scope) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        topology_metrics::metrics(self, scope)
    }

    fn gitops(&self, context: String) -> Read<GitOpsInventory> {
        self.read(inventory::gitops(context))
    }

    fn cluster_resources(&self, context: String, refresh: bool) -> Read<ScopeResult> {
        self.read(inventory::cluster_resources(
            context,
            refresh,
            self.store.clone(),
        ))
    }

    fn events(
        &self,
        resource: ResourceSummary,
        uid: Option<String>,
    ) -> Read<Vec<ResourceEventSummary>> {
        let cancellations = self.cancellations.clone();
        self.read(async move {
            commands::list_resource_events(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                uid,
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn incidents(&self, context: String) -> Read<IncidentCockpitSummary> {
        let requests = crate::incidents::workspace_requests();
        let store = self.store.clone();
        self.read(async move {
            commands::list_incident_cockpit(context, requests, None, State::new(&store)).await
        })
    }

    fn live_sessions(&self) -> Read<LiveSessions> {
        let (port_forwards, exec_sessions) =
            (self.port_forwards.clone(), self.exec_sessions.clone());
        self.read(async move {
            let access = ClusterAccess::resolve(None)?;
            Ok(LiveSessions {
                port_forwards: commands::list_port_forwards(State::new(&port_forwards)).await?,
                exec_sessions: commands::list_pod_exec_sessions(State::new(&exec_sessions)).await?,
                source_key: Some(access.source_key().to_owned()),
                show_source_labels: access.source_label().is_some(),
            })
        })
    }

    fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        detail_reads::log_stream(self, target)
    }

    fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        self.read(async move {
            let namespace = resource.namespace.unwrap_or_default();
            commands::list_deployment_revisions(resource.cluster, resource.name, namespace, None)
                .await
        })
    }

    fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        self.read(commands::get_resource_action_state(request))
    }

    fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        self.read(commands::preview_resource_action(request))
    }

    fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        self.read(commands::execute_resource_action(request))
    }

    fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        self.resource_yaml_encoded(resource, mode, kubecove_lib::models::YamlEncoding::Yaml)
    }

    fn resource_yaml_encoded(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: kubecove_lib::models::YamlEncoding,
    ) -> Read<String> {
        detail_reads::resource_yaml(self, resource, mode, encoding)
    }

    fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        self.read(commands::prepare_yaml_apply(request))
    }

    fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.read(commands::apply_yaml(request))
    }

    fn lint_yaml(&self, request: YamlApplyRequest) -> Read<KubernetesYamlLintResult> {
        self.read(commands::lint_kubernetes_yaml(request))
    }

    fn namespaces(
        &self,
        context: String,
        kubeconfig_source_key: Option<String>,
    ) -> Read<Vec<NamespaceSummary>> {
        let store = self.store.clone();
        self.read(async move {
            commands::list_namespaces(context, kubeconfig_source_key, State::new(&store))
                .await
                .map(Arc::unwrap_or_clone)
        })
    }

    fn rbac(&self, context: String) -> Read<RbacInspectionSummary> {
        self.read(security::rbac(context, self.cancellations.clone()))
    }

    fn review_rbac(&self, request: RbacAccessReviewRequest) -> Read<RbacAccessReviewResult> {
        self.read(security::review(request, self.cancellations.clone()))
    }

    fn kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        self.read(security::kubeconfig_sources())
    }

    fn kubeconfig_writes_unavailable(&self) -> Option<&'static str> {
        (!self.settings_initialized).then_some(security::KUBECONFIG_WRITES)
    }

    fn kubeconfig_source_edits_unavailable(&self) -> Option<&'static str> {
        (!self.settings_initialized).then_some(security::KUBECONFIG_WRITES)
    }

    fn set_kubeconfig_env_var(&self, value: String) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::set_kubeconfig_env_var(value) })
    }

    fn set_show_kubeconfig_source_labels(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::set_show_kubeconfig_source_labels(show) })
    }

    fn pick_kubeconfig_paths(&self) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.pick_kubeconfig()
    }
    fn remove_kubeconfig_path(&self, path: String) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::remove_kubeconfig_path(path) })
    }
    fn reorder_kubeconfig_paths(&self, paths: Vec<String>) -> Read<KubeconfigSourcesSummary> {
        if !self.settings_initialized {
            return self.read(security::unsaved());
        }
        self.read(async move { commands::reorder_kubeconfig_paths(paths) })
    }
    fn check_updates(&self) -> Read<UpdateStatus> {
        self.read(settings_integrations::check_updates())
    }
    fn import_workspace(&self) -> Read<Option<String>> {
        self.read(workspace_files::import())
    }
    fn export_workspace(&self, name: String, content: String) -> Read<bool> {
        self.read(workspace_files::export(name, content))
    }

    fn helm_releases(&self, context: String) -> Read<HelmReleaseList> {
        self.read(commands::list_helm_releases(context, None))
    }

    fn helm_release(&self, release: HelmReleaseSummary) -> Read<HelmReleaseDetails> {
        self.read(delivery::helm_release(release))
    }

    fn helm_reconciliation(&self, release: HelmReleaseSummary) -> Read<HelmReleaseReconciliation> {
        self.read(delivery::helm_reconciliation(release))
    }

    fn argo_inspector(&self, application: ArgoApplicationRef) -> Read<ArgoApplicationInspector> {
        let (store, cancellations) = (self.argo_connections.clone(), self.cancellations.clone());
        self.read(delivery::argo_inspector(application, store, cancellations))
    }
}
