use crate::commands::helpers::k8s_timestamp_to_datetime;
use crate::models::{GitOpsOwnerSummary, OwnerReferenceSummary, ResourceSummary};

const ANNOTATION_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/name";
const ANNOTATION_ARGOCD_TRACKING_ID: &str = "argocd.argoproj.io/tracking-id";
const LABEL_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/application";
const LABEL_APP_KUBERNETES_IO_INSTANCE: &str = "app.kubernetes.io/instance";
const LABEL_HELM_RELEASE_NAME: &str = "helm.sh/release";
const LABEL_FLUX_KUSTOMIZE_NAME: &str = "kustomize.toolkit.fluxcd.io/name";
const LABEL_FLUX_KUSTOMIZE_NAMESPACE: &str = "kustomize.toolkit.fluxcd.io/namespace";
const LABEL_FLUX_HELM_NAME: &str = "helm.toolkit.fluxcd.io/name";
const LABEL_FLUX_HELM_NAMESPACE: &str = "helm.toolkit.fluxcd.io/namespace";

pub(crate) fn extract_owner_ref(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| refs.iter().next())
        .map(|r| r.name.clone())
}

pub(crate) fn extract_owner_ref_summary(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<OwnerReferenceSummary> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| refs.iter().next())
        .map(|r| OwnerReferenceSummary {
            api_version: r.api_version.clone(),
            kind: r.kind.clone(),
            name: r.name.clone(),
            uid: r.uid.clone(),
        })
}

pub(crate) fn argo_app_from_tracking_id(tracking_id: &str) -> Option<String> {
    tracking_id
        .split_once(':')
        .map(|(app, _)| app)
        .filter(|app| !app.trim().is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_argo_app(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    if let Some(annotations) = &metadata.annotations {
        if let Some(name) = annotations
            .get(ANNOTATION_ARGOCD_APP_NAME)
            .filter(|value| !value.trim().is_empty())
        {
            return Some(name.clone());
        }
        if let Some(id) = annotations.get(ANNOTATION_ARGOCD_TRACKING_ID) {
            if let Some(app) = argo_app_from_tracking_id(id) {
                return Some(app);
            }
        }
    }
    if let Some(labels) = &metadata.labels {
        if let Some(name) = labels
            .get(LABEL_ARGOCD_APP_NAME)
            .filter(|value| !value.trim().is_empty())
        {
            return Some(name.clone());
        }
        if let Some(instance) = labels
            .get(LABEL_APP_KUBERNETES_IO_INSTANCE)
            .filter(|value| !value.trim().is_empty())
        {
            return Some(instance.clone());
        }
    }
    None
}

pub(crate) fn extract_helm_release(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(LABEL_HELM_RELEASE_NAME))
        .cloned()
}

pub(crate) fn extract_git_ops_owner(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<GitOpsOwnerSummary> {
    if let Some(argo_app) = extract_argo_app(metadata) {
        return Some(GitOpsOwnerSummary {
            provider: "argo".to_string(),
            kind: "Application".to_string(),
            name: argo_app,
            namespace: None,
            confidence: "metadata".to_string(),
        });
    }

    let labels = metadata.labels.as_ref()?;
    if let Some(name) = labels
        .get(LABEL_FLUX_KUSTOMIZE_NAME)
        .filter(|value| !value.trim().is_empty())
    {
        return Some(GitOpsOwnerSummary {
            provider: "flux".to_string(),
            kind: "Kustomization".to_string(),
            name: name.clone(),
            namespace: labels.get(LABEL_FLUX_KUSTOMIZE_NAMESPACE).cloned(),
            confidence: "label".to_string(),
        });
    }
    labels
        .get(LABEL_FLUX_HELM_NAME)
        .filter(|value| !value.trim().is_empty())
        .map(|name| GitOpsOwnerSummary {
            provider: "flux".to_string(),
            kind: "HelmRelease".to_string(),
            name: name.clone(),
            namespace: labels.get(LABEL_FLUX_HELM_NAMESPACE).cloned(),
            confidence: "label".to_string(),
        })
}

pub(crate) fn base_resource_summary(
    kind: &str,
    cluster: &str,
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
    age: String,
) -> ResourceSummary {
    ResourceSummary {
        kind: kind.to_string(),
        cluster: cluster.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone(),
        age,
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        created_at: metadata
            .creation_timestamp
            .as_ref()
            .and_then(|t| k8s_timestamp_to_datetime(&t.0).map(|dt| dt.to_rfc3339())),
        status: None,
        ready: None,
        restarts: None,
        owner_ref: extract_owner_ref(metadata),
        argo_app: extract_argo_app(metadata),
        helm_release: extract_helm_release(metadata),
        git_ops_owner: extract_git_ops_owner(metadata),
    }
}

fn opt_i32_to_str(opt: Option<i32>) -> String {
    opt.map_or_else(|| "0".to_string(), |v| v.to_string())
}

pub(crate) fn fmt_ready(ready: Option<i32>, desired: i32) -> String {
    format!("{}/{}", opt_i32_to_str(ready), desired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
    use std::collections::BTreeMap;

    #[test]
    fn extracts_app_name_from_argocd_tracking_id() {
        let mut annotations = BTreeMap::new();
        annotations.insert(
            ANNOTATION_ARGOCD_TRACKING_ID.to_string(),
            "argocd:/ConfigMap:argocd/argocd-cm".to_string(),
        );
        let metadata = ObjectMeta {
            annotations: Some(annotations),
            ..Default::default()
        };

        assert_eq!(extract_argo_app(&metadata), Some("argocd".to_string()));
    }

    #[test]
    fn extracts_owner_argo_and_helm_metadata() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_ARGOCD_APP_NAME.to_string(), "payments".to_string());
        labels.insert(
            LABEL_HELM_RELEASE_NAME.to_string(),
            "payments-api".to_string(),
        );
        let metadata = ObjectMeta {
            labels: Some(labels),
            owner_references: Some(vec![OwnerReference {
                api_version: "apps/v1".to_string(),
                kind: "Deployment".to_string(),
                name: "api".to_string(),
                uid: "uid-1".to_string(),
                ..Default::default()
            }]),
            ..Default::default()
        };

        assert_eq!(extract_owner_ref(&metadata), Some("api".to_string()));
        assert_eq!(
            extract_owner_ref_summary(&metadata),
            Some(crate::models::OwnerReferenceSummary {
                api_version: "apps/v1".to_string(),
                kind: "Deployment".to_string(),
                name: "api".to_string(),
                uid: "uid-1".to_string(),
            })
        );
        assert_eq!(extract_argo_app(&metadata), Some("payments".to_string()));
        assert_eq!(
            extract_git_ops_owner(&metadata),
            Some(GitOpsOwnerSummary {
                provider: "argo".to_string(),
                kind: "Application".to_string(),
                name: "payments".to_string(),
                namespace: None,
                confidence: "metadata".to_string(),
            })
        );
        assert_eq!(
            extract_helm_release(&metadata),
            Some("payments-api".to_string())
        );
    }

    #[test]
    fn owner_ref_summary_is_absent_without_owner_references() {
        let metadata = ObjectMeta {
            name: Some("api".to_string()),
            ..Default::default()
        };

        assert_eq!(extract_owner_ref_summary(&metadata), None);
    }

    #[test]
    fn argocd_annotation_precedes_instance_label() {
        let mut annotations = BTreeMap::new();
        annotations.insert(
            ANNOTATION_ARGOCD_APP_NAME.to_string(),
            "annotation-app".to_string(),
        );
        let mut labels = BTreeMap::new();
        labels.insert(
            LABEL_APP_KUBERNETES_IO_INSTANCE.to_string(),
            "instance-app".to_string(),
        );
        let metadata = ObjectMeta {
            annotations: Some(annotations),
            labels: Some(labels),
            ..Default::default()
        };

        assert_eq!(
            extract_argo_app(&metadata),
            Some("annotation-app".to_string())
        );
    }

    #[test]
    fn skips_empty_argocd_values_and_uses_fallbacks() {
        let mut annotations = BTreeMap::new();
        annotations.insert(ANNOTATION_ARGOCD_APP_NAME.to_string(), String::new());
        annotations.insert(
            ANNOTATION_ARGOCD_TRACKING_ID.to_string(),
            "   :ConfigMap:argocd/argocd-cm".to_string(),
        );
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_ARGOCD_APP_NAME.to_string(), " ".to_string());
        labels.insert(
            LABEL_APP_KUBERNETES_IO_INSTANCE.to_string(),
            "fallback-app".to_string(),
        );
        let metadata = ObjectMeta {
            annotations: Some(annotations),
            labels: Some(labels),
            ..Default::default()
        };

        assert_eq!(
            extract_argo_app(&metadata),
            Some("fallback-app".to_string())
        );
    }

    #[test]
    fn extracts_flux_gitops_owner_from_kustomization_labels() {
        let labels = BTreeMap::from([
            (LABEL_FLUX_KUSTOMIZE_NAME.to_string(), "apps".to_string()),
            (
                LABEL_FLUX_KUSTOMIZE_NAMESPACE.to_string(),
                "flux-system".to_string(),
            ),
        ]);
        let metadata = ObjectMeta {
            labels: Some(labels),
            ..Default::default()
        };

        assert_eq!(
            extract_git_ops_owner(&metadata),
            Some(GitOpsOwnerSummary {
                provider: "flux".to_string(),
                kind: "Kustomization".to_string(),
                name: "apps".to_string(),
                namespace: Some("flux-system".to_string()),
                confidence: "label".to_string(),
            })
        );
    }
}
