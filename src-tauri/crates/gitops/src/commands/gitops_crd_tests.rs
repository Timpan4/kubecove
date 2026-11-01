use crate::commands::{detect_argocd, detect_flux, FixedClusterAccess};
use kubecove_kube::commands::discovery_fixture::{self, FixtureKind};

const ARGO_APPLICATION: FixtureKind = FixtureKind {
    group: "argoproj.io",
    version: "v1alpha1",
    kind: "Application",
    plural: "applications",
    namespaced: true,
};

const FLUX_GIT_REPOSITORY: FixtureKind = FixtureKind {
    group: "source.toolkit.fluxcd.io",
    version: "v1",
    kind: "GitRepository",
    plural: "gitrepositories",
    namespaced: true,
};

#[tokio::test]
async fn provider_detection_ignores_unrelated_api_with_null_verbs() {
    let scope = FixedClusterAccess::new(discovery_fixture::client(&[
        ARGO_APPLICATION,
        FLUX_GIT_REPOSITORY,
    ]))
    .scope();

    let argo = scope
        .clone()
        .run(detect_argocd("fixture".into(), None))
        .await
        .expect("Argo CD detection");
    let flux = scope
        .run(detect_flux("fixture".into(), None))
        .await
        .expect("Flux detection");

    assert!(argo);
    assert!(flux.detected);
    assert_eq!(
        flux.kinds
            .iter()
            .map(|kind| kind.kind.as_str())
            .collect::<Vec<_>>(),
        ["GitRepository"]
    );
}

#[tokio::test]
async fn providers_without_installed_kinds_are_not_detected() {
    let scope = FixedClusterAccess::new(discovery_fixture::client(&[])).scope();

    let argo = scope
        .clone()
        .run(detect_argocd("fixture".into(), None))
        .await
        .expect("Argo CD detection");
    let flux = scope
        .run(detect_flux("fixture".into(), None))
        .await
        .expect("Flux detection");

    assert!(!argo);
    assert!(!flux.detected);
}

// Failure mode: an overloaded API server answers the group list with 503, and
// detection reports every provider as not installed instead of failing.
#[tokio::test]
async fn provider_detection_fails_when_the_group_list_is_unavailable() {
    let scope = FixedClusterAccess::new(discovery_fixture::client_with_unavailable_group_list(&[
        ARGO_APPLICATION,
        FLUX_GIT_REPOSITORY,
    ]))
    .scope();

    let argo = scope
        .clone()
        .run(detect_argocd("fixture".into(), None))
        .await;
    let flux = scope.run(detect_flux("fixture".into(), None)).await;

    assert!(argo.is_err(), "Argo CD detection: {argo:?}");
    assert!(flux.is_err(), "Flux detection: {flux:?}");
}
