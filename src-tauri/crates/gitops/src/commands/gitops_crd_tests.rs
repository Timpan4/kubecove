use crate::commands::{detect_argocd, detect_flux, FixedClusterAccess};
use crate::models::AppErrorKind;
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

// Failure mode: Flux serves `Receiver` at v1 and `Alert` at v1beta3 in one
// group, the group prefers v1, and detection drops the installed `Alert`.
#[tokio::test]
async fn flux_detection_finds_kinds_served_only_at_a_non_preferred_version() {
    let notification = |version, kind, plural| FixtureKind {
        group: "notification.toolkit.fluxcd.io",
        version,
        kind,
        plural,
        namespaced: true,
    };
    let scope = FixedClusterAccess::new(discovery_fixture::client(&[
        notification("v1", "Receiver", "receivers"),
        notification("v1beta3", "Alert", "alerts"),
    ]))
    .scope();

    let flux = scope
        .run(detect_flux("fixture".into(), None))
        .await
        .expect("Flux detection");

    let mut kinds: Vec<(&str, &str)> = flux
        .kinds
        .iter()
        .map(|kind| (kind.api_version.as_str(), kind.kind.as_str()))
        .collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        [
            ("notification.toolkit.fluxcd.io/v1", "Receiver"),
            ("notification.toolkit.fluxcd.io/v1beta3", "Alert"),
        ]
    );
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

// Failure mode: a provider's API version answers 503, and detection reports
// the installed provider as not installed instead of failing.
#[tokio::test]
async fn provider_detection_fails_when_a_provider_api_version_is_unavailable() {
    let scope = FixedClusterAccess::new(discovery_fixture::client_with_unavailable_groups(
        &[ARGO_APPLICATION, FLUX_GIT_REPOSITORY],
        &[
            (ARGO_APPLICATION.group, ARGO_APPLICATION.version),
            (FLUX_GIT_REPOSITORY.group, FLUX_GIT_REPOSITORY.version),
        ],
    ))
    .scope();

    let argo = scope
        .clone()
        .run(detect_argocd("fixture".into(), None))
        .await
        .expect_err("Argo CD detection");
    let flux = scope
        .run(detect_flux("fixture".into(), None))
        .await
        .expect_err("Flux detection");

    assert_eq!(argo.kind, AppErrorKind::ProviderDiscoveryUnavailable);
    assert_eq!(flux.kind, AppErrorKind::ProviderDiscoveryUnavailable);
}
