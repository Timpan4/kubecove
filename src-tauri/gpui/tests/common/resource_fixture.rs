use super::*;

impl Fixture {
    pub(super) fn resource_fixture(&self, scope: Scope) -> Read<ScopeResult> {
        if *self.shell_read_phase.lock().unwrap() == "resources-failed" {
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
        // Cluster-scoped kinds come from `cluster_resources`, as in the native backend.
        // Each context has the exact rows exported by the browser mocks.
        let resources = self
            .context_resources
            .iter()
            .filter(|row| row.cluster == scope.context)
            .filter(|row| !CLUSTER_KINDS.contains(&row.kind.as_str()))
            .filter(|row| {
                scope.namespace.is_none() || row.namespace.as_deref() == scope.namespace.as_deref()
            })
            .map(|row| ResourceSummary {
                cluster: scope.context.clone(),
                ..row.clone()
            })
            .collect();
        let resources = self.with_removed_resource(resources);
        let namespaces = self.namespaces.clone();
        let read = self.timing.read(
            "list_resource_scope",
            &scope.context,
            scope.namespace.as_deref().unwrap_or_default(),
            (namespaces, resources),
        );
        Box::pin(async move {
            let (namespaces, resources) = read.await?;
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings: vec![],
            })
        })
    }
    pub(super) fn with_removed_resource(
        &self,
        resources: Vec<ResourceSummary>,
    ) -> Vec<ResourceSummary> {
        // The scripted Deployment was deleted from the cluster after it was pinned.
        let removed = *self.shell_read_phase.lock().unwrap() == "payments-api-removed";
        self.live_rows(resources)
            .into_iter()
            .filter(|row| {
                !(removed
                    && row.kind == "Deployment"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == "payments-api")
            })
            .collect::<Vec<_>>()
    }
}
