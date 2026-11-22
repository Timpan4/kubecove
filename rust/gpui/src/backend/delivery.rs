//! Native reads for the Helm page, the Argo CD Application tab and the details
//! of custom resources such as Applications. Helm reads release storage Secrets
//! and ConfigMaps. The Application tab uses the Kubernetes transport only, so
//! no Argo CD connection or credential is used (ADR 0013).

use kubecove_lib::{
    commands::{self, ArgoConnectionStore, BackendCancellationRegistry},
    models::{
        AppError, ArgoApplicationInspector, ArgoApplicationRef, DiscoveredResourceKind,
        HelmReleaseDetails, HelmReleaseReconciliation, HelmReleaseSummary, ResourceDetailsFull,
        ResourceSummary, YamlEncoding, YamlViewMode,
    },
    runtime::State,
};
use std::sync::Arc;

/// `getHelmReleaseDetails` with the `kubectl` view in YAML, as `HelmSurface.svelte` asks.
pub(super) async fn helm_release(
    release: HelmReleaseSummary,
) -> Result<HelmReleaseDetails, AppError> {
    commands::get_helm_release_details(
        release.cluster,
        release.namespace,
        release.storage_kind,
        release.storage_name,
        None,
        Some(YamlViewMode::Kubectl),
        Some(YamlEncoding::Yaml),
    )
    .await
}

pub(super) async fn helm_reconciliation(
    release: HelmReleaseSummary,
) -> Result<HelmReleaseReconciliation, AppError> {
    commands::get_helm_release_reconciliation(
        release.cluster,
        release.namespace,
        release.storage_kind,
        release.storage_name,
        None,
    )
    .await
}

/// `getArgoApplicationInspector` with `transport: "kubernetes"`. The store
/// stays empty, so no connected profile can be chosen.
pub(super) async fn argo_inspector(
    application: ArgoApplicationRef,
    store: Arc<ArgoConnectionStore>,
    cancellations: BackendCancellationRegistry,
) -> Result<ArgoApplicationInspector, AppError> {
    let context = application.context.clone().unwrap_or_default();
    commands::get_argo_application_inspector(
        context,
        None,
        None,
        "kubernetes".into(),
        application,
        None,
        None,
        State::new(&*store),
        State::new(&cancellations),
    )
    .await
}

/// The discovered kind a dynamic row carries, which `get_dynamic_resource_details` reads.
pub(super) fn dynamic_kind(resource: &ResourceSummary) -> Option<DiscoveredResourceKind> {
    Some(DiscoveredResourceKind {
        group: resource.group.clone().unwrap_or_default(),
        version: resource.version.clone()?,
        api_version: resource.api_version.clone()?,
        kind: resource.kind.clone(),
        plural: resource.plural.clone()?,
        short_names: Vec::new(),
        namespaced: resource.namespaced.unwrap_or(resource.namespace.is_some()),
    })
}

/// `getDynamicResourceDetails` for custom resources such as Argo CD Applications.
pub(super) async fn dynamic_details(
    resource: ResourceSummary,
    kind: DiscoveredResourceKind,
    cancellations: BackendCancellationRegistry,
) -> Result<ResourceDetailsFull, AppError> {
    commands::get_dynamic_resource_details(
        resource.cluster,
        kind,
        resource.name,
        resource.namespace,
        None,
        None,
        None,
        None,
        None,
        State::new(&cancellations),
    )
    .await
}
