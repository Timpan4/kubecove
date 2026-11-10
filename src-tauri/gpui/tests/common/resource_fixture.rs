use super::*;
use kubecove_gpui::backend::{INSPECTION_KINDS, Listed, discovered_key};
use kubecove_lib::models::{WatchResourceKey, WatchResourceKind};

impl Fixture {
    pub(super) fn resource_fixture(&self, scope: Scope) -> Read<ScopeResult> {
        let Some((fixture, executor)) = self
            .off_thread_reads
            .get()
            .and_then(|(fixture, executor)| Some((fixture.upgrade()?, executor.clone())))
        else {
            return self.build_resource_read(scope);
        };
        // Only the row copy runs in the background. The timed read it returns is
        // awaited by the caller, as before.
        #[allow(clippy::async_yields_async)]
        let read = executor.spawn(async move { fixture.build_resource_read(scope) });
        Box::pin(async move { read.await.await })
    }

    /// Filters and copies the scope's rows, then returns the timed read.
    fn build_resource_read(&self, scope: Scope) -> Read<ScopeResult> {
        let phase = self.shell_read_phase.lock().unwrap().clone();
        if phase == "resources-failed" {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted resources read failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        if self.failed_workspace_context.lock().unwrap().as_deref() == Some(&scope.context) {
            return Box::pin(async {
                Err(AppError::new("Context unavailable", AppErrorKind::Cluster))
            });
        }
        if self.live_failed() {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted reload failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        // The Deployment list is refused, as `shellScopeRequests` refuses it in the browser mocks.
        let forbidden =
            |kind: &str| phase == "payments-deployments-forbidden" && kind == "Deployment";
        // Cluster-scoped kinds come from `cluster_resources`, as in the native backend.
        // Each context has the exact rows exported by the browser mocks.
        let resources: Vec<_> = self
            .context_resources
            .iter()
            .filter(|row| row.cluster == scope.context)
            .filter(|row| !CLUSTER_KINDS.contains(&row.kind.as_str()) && !forbidden(&row.kind))
            .filter(|row| {
                scope.namespace.is_none() || row.namespace.as_deref() == scope.namespace.as_deref()
            })
            .map(|row| ResourceSummary {
                cluster: scope.context.clone(),
                ..row.clone()
            })
            .collect();
        // Built-in lists always run; custom kinds are listed when present, as in
        // the native backend's presence probe.
        let mut listed: Vec<_> = INSPECTION_KINDS
            .iter()
            .filter(|kind| !forbidden(kind))
            .map(|kind| WatchResourceKey {
                resource_kind: WatchResourceKind {
                    kind: (*kind).to_owned(),
                    group: None,
                    version: None,
                    api_version: None,
                    plural: None,
                    namespaced: Some(true),
                },
                namespace: scope.namespace.clone(),
            })
            .collect();
        listed.extend(
            self.kinds
                .iter()
                .filter(|kind| resources.iter().any(|row| row.kind == kind.kind))
                .map(|kind| discovered_key(kind, scope.namespace.clone())),
        );
        let listed = listed
            .into_iter()
            .map(|key| Listed {
                context: scope.context.clone(),
                key,
            })
            .collect();
        let warnings = if phase == "payments-deployments-forbidden" {
            vec!["Deployment: deployments.apps is forbidden".to_owned()]
        } else {
            vec![]
        };
        let resources = self.with_removed_resource(resources);
        let namespaces = self.namespaces.clone();
        // The version is read with the rows it describes.
        let versions = self.list_versions(scope.namespace.as_deref());
        let read = self.timing.read(
            "list_resource_scope",
            &scope.context,
            scope.namespace.as_deref().unwrap_or_default(),
            (namespaces, resources, versions),
        );
        Box::pin(async move {
            let (namespaces, resources, versions) = read.await?;
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings,
                versions,
                listed,
            })
        })
    }
    /// Applies the scripted cluster changes after a resource was pinned:
    /// a deleted Deployment, or an Ingress served at another API version.
    pub(super) fn with_removed_resource(
        &self,
        resources: Vec<ResourceSummary>,
    ) -> Vec<ResourceSummary> {
        let phase = self.shell_read_phase.lock().unwrap().clone();
        self.live_rows(resources)
            .into_iter()
            .filter(|row| {
                !(phase == "payments-api-removed"
                    && row.kind == "Deployment"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == "payments-api")
            })
            .map(|row| {
                if phase == "payments-ingress-version-changed"
                    && row.kind == "Ingress"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == "payments-api"
                {
                    ResourceSummary {
                        api_version: Some("networking.k8s.io/v1beta1".into()),
                        ..row
                    }
                } else {
                    row
                }
            })
            .collect::<Vec<_>>()
    }

    pub(super) fn application_resources_fixture(
        &self,
        app: ArgoApplicationSummary,
    ) -> Read<ScopeResult> {
        let resources = self.resources.clone();
        let namespaces = self.namespaces.clone();
        // delivery_workspace::resources lists each kind once per target namespace
        // and reports each list's version under that list's own watch key.
        let mut targets = app.resource_namespaces;
        if targets.is_empty() {
            targets.extend(app.destination_namespace);
        }
        let versions = if targets.is_empty() {
            self.list_versions(None)
        } else {
            targets
                .iter()
                .flat_map(|namespace| self.list_versions(Some(namespace)))
                .collect()
        };
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings: vec![],
                versions,
                listed: vec![],
            })
        })
    }

    pub(super) fn cluster_resources_fixture(&self) -> Read<ScopeResult> {
        let resources = self
            .resources
            .iter()
            .filter(|row| CLUSTER_KINDS.contains(&row.kind.as_str()))
            .cloned()
            .collect();
        let versions = self.list_versions(None);
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces: vec![],
                resources,
                warnings: vec![],
                versions,
                listed: vec![],
            })
        })
    }

    pub(super) fn namespace_custom_kinds_fixture(
        &self,
        scope: Scope,
    ) -> Read<Vec<kubecove_lib::models::DiscoveredResourceKind>> {
        let kinds = self
            .kinds
            .iter()
            .filter(|kind| {
                kind.namespaced
                    && self.resources.iter().any(|row| {
                        row.kind == kind.kind
                            && row.api_version.as_ref() == Some(&kind.api_version)
                            // A cluster-wide scope has no namespace and reads every namespace.
                            && scope
                                .namespace
                                .as_ref()
                                .is_none_or(|namespace| row.namespace.as_ref() == Some(namespace))
                    })
            })
            .cloned()
            .collect();
        Box::pin(async move { Ok(kinds) })
    }

    pub(super) fn details_fixture(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let _profile = kubecove_gpui::frame_profile::scope("fixture", "details");
        let found = self
            .details
            .iter()
            .find(|detail| {
                detail.summary.kind == resource.kind
                    && detail.summary.cluster == resource.cluster
                    && detail.summary.name == resource.name
                    && detail.summary.namespace == resource.namespace
            })
            .cloned()
            .or_else(|| self.gitops.application_details(&resource));
        // Named after the command the native and web clients call for this resource.
        let command = if resource.dynamic == Some(true) {
            "get_dynamic_resource_details"
        } else {
            "get_resource_details"
        };
        let read = self.timing.read(command, &resource.cluster, "", found);
        self.coverage_read(
            "get_resource_details",
            Box::pin(async move {
                read.await?
                    .ok_or_else(|| AppError::new("No fixture details", AppErrorKind::NotFound))
            }),
        )
    }

    pub(super) fn events_fixture(
        &self,
        resource: ResourceSummary,
    ) -> Read<Vec<ResourceEventSummary>> {
        if let Some(events) = self.scripted_events(&resource) {
            return Box::pin(async move { Ok(events) });
        }
        let found = self
            .events
            .iter()
            .find(|row| {
                row.kind == resource.kind
                    && row.name == resource.name
                    && row.namespace == resource.namespace
            })
            .map_or_else(Vec::new, |row| row.events.clone());
        Box::pin(async move { Ok(found) })
    }

    pub(super) fn reveal_secret_fixture(
        &self,
        resource: ResourceSummary,
        key: String,
    ) -> Read<String> {
        let request = serde_json::json!({"clusterContext": resource.cluster, "name": resource.name, "namespace": resource.namespace, "key": key});
        let found = self
            .secret_reveals
            .iter()
            .find(|entry| entry["request"] == request)
            .and_then(|entry| entry["encoded"].as_str())
            .map(str::to_owned);
        Box::pin(async move {
            found.ok_or_else(|| {
                AppError::new("No matching Secret reveal fixture", AppErrorKind::NotFound)
            })
        })
    }
}
