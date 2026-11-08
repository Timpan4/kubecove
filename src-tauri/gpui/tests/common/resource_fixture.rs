use super::*;
use kubecove_gpui::backend::{INSPECTION_KINDS, Listed, discovered_key};
use kubecove_lib::models::{WatchResourceKey, WatchResourceKind};

impl Fixture {
    pub(super) fn resource_fixture(&self, scope: Scope) -> Read<ScopeResult> {
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
}
