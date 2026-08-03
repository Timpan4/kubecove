use crate::models::GitOpsOwnerSummary;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

const ANNOTATION_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/name";
const ANNOTATION_ARGOCD_TRACKING_ID: &str = "argocd.argoproj.io/tracking-id";
const LABEL_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/application";
const LABEL_FLUX_KUSTOMIZE_NAME: &str = "kustomize.toolkit.fluxcd.io/name";
const LABEL_FLUX_KUSTOMIZE_NAMESPACE: &str = "kustomize.toolkit.fluxcd.io/namespace";
const LABEL_FLUX_HELM_NAME: &str = "helm.toolkit.fluxcd.io/name";
const LABEL_FLUX_HELM_NAMESPACE: &str = "helm.toolkit.fluxcd.io/namespace";

pub(super) fn argo_app_from_tracking_id(tracking_id: &str) -> Option<String> {
    tracking_id
        .split_once(':')
        .map(|(app, _)| app)
        .filter(|app| !app.trim().is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_argo_app(metadata: &ObjectMeta) -> Option<String> {
    argo_owner(metadata).map(|owner| owner.name)
}

pub(crate) fn extract_git_ops_owner(metadata: &ObjectMeta) -> Option<GitOpsOwnerSummary> {
    argo_owner(metadata).or_else(|| flux_owner(metadata))
}

fn argo_owner(metadata: &ObjectMeta) -> Option<GitOpsOwnerSummary> {
    let (name, confidence, provenance) = metadata
        .annotations
        .as_ref()
        .and_then(|annotations| {
            annotations
                .get(ANNOTATION_ARGOCD_APP_NAME)
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .map(|name| (name, "metadata", "argocd-name-annotation"))
                .or_else(|| {
                    annotations
                        .get(ANNOTATION_ARGOCD_TRACKING_ID)
                        .and_then(|value| argo_app_from_tracking_id(value))
                        .map(|name| (name, "metadata", "argocd-tracking-annotation"))
                })
        })
        .or_else(|| {
            metadata.labels.as_ref().and_then(|labels| {
                labels
                    .get(LABEL_ARGOCD_APP_NAME)
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .map(|name| (name, "label", "argocd-application-label"))
            })
        })?;

    Some(owner(
        "argo",
        "Application",
        name,
        None,
        confidence,
        provenance,
    ))
}

fn flux_owner(metadata: &ObjectMeta) -> Option<GitOpsOwnerSummary> {
    let labels = metadata.labels.as_ref()?;
    labels
        .get(LABEL_FLUX_KUSTOMIZE_NAME)
        .filter(|value| !value.trim().is_empty())
        .map(|name| {
            owner(
                "flux",
                "Kustomization",
                name.clone(),
                non_empty(labels.get(LABEL_FLUX_KUSTOMIZE_NAMESPACE)),
                "label",
                "flux-kustomization-labels",
            )
        })
        .or_else(|| {
            labels
                .get(LABEL_FLUX_HELM_NAME)
                .filter(|value| !value.trim().is_empty())
                .map(|name| {
                    owner(
                        "flux",
                        "HelmRelease",
                        name.clone(),
                        non_empty(labels.get(LABEL_FLUX_HELM_NAMESPACE)),
                        "label",
                        "flux-helmrelease-labels",
                    )
                })
        })
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).cloned()
}

fn owner(
    provider: &str,
    kind: &str,
    name: String,
    namespace: Option<String>,
    confidence: &str,
    provenance: &str,
) -> GitOpsOwnerSummary {
    GitOpsOwnerSummary {
        provider: provider.to_string(),
        kind: kind.to_string(),
        name,
        namespace,
        confidence: confidence.to_string(),
        provenance: provenance.to_string(),
        partial: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn argo_adapter_preserves_precedence_and_provenance() {
        let metadata = ObjectMeta {
            annotations: Some(BTreeMap::from([
                (
                    ANNOTATION_ARGOCD_APP_NAME.to_string(),
                    "payments".to_string(),
                ),
                (
                    ANNOTATION_ARGOCD_TRACKING_ID.to_string(),
                    "tracking-app:/Deployment:default/api".to_string(),
                ),
            ])),
            labels: Some(BTreeMap::from([
                (LABEL_ARGOCD_APP_NAME.to_string(), "label-app".to_string()),
                (
                    LABEL_FLUX_KUSTOMIZE_NAME.to_string(),
                    "flux-apps".to_string(),
                ),
            ])),
            ..Default::default()
        };

        assert_eq!(
            extract_git_ops_owner(&metadata),
            Some(owner(
                "argo",
                "Application",
                "payments".to_string(),
                None,
                "metadata",
                "argocd-name-annotation",
            ))
        );
    }

    #[test]
    fn generic_instance_label_is_not_argo_ownership_evidence() {
        let metadata = ObjectMeta {
            labels: Some(BTreeMap::from([(
                "app.kubernetes.io/instance".to_string(),
                "generic-instance".to_string(),
            )])),
            ..Default::default()
        };

        assert_eq!(extract_argo_app(&metadata), None);
        assert_eq!(extract_git_ops_owner(&metadata), None);
    }

    #[test]
    fn flux_adapter_normalizes_scoped_partial_evidence() {
        let metadata = ObjectMeta {
            labels: Some(BTreeMap::from([
                (LABEL_FLUX_HELM_NAME.to_string(), "worker".to_string()),
                (
                    LABEL_FLUX_HELM_NAMESPACE.to_string(),
                    "flux-system".to_string(),
                ),
            ])),
            ..Default::default()
        };

        assert_eq!(
            extract_git_ops_owner(&metadata),
            Some(owner(
                "flux",
                "HelmRelease",
                "worker".to_string(),
                Some("flux-system".to_string()),
                "label",
                "flux-helmrelease-labels",
            ))
        );
    }

    #[test]
    fn empty_provider_values_are_not_evidence() {
        let metadata = ObjectMeta {
            annotations: Some(BTreeMap::from([
                (ANNOTATION_ARGOCD_APP_NAME.to_string(), " ".to_string()),
                (
                    ANNOTATION_ARGOCD_TRACKING_ID.to_string(),
                    " :Deployment:default/api".to_string(),
                ),
            ])),
            labels: Some(BTreeMap::from([
                (LABEL_ARGOCD_APP_NAME.to_string(), String::new()),
                (LABEL_FLUX_KUSTOMIZE_NAME.to_string(), " ".to_string()),
            ])),
            ..Default::default()
        };

        assert_eq!(extract_git_ops_owner(&metadata), None);
    }
}
