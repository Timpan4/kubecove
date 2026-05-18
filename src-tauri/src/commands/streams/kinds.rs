use crate::models::{AppError, WatchResourceKind};
use kube::api::ApiResource;

fn known_resource_kind(kind: &str) -> Option<WatchResourceKind> {
    let (api_version, group, version, plural, namespaced) = match kind {
        "Pod" => ("v1", "", "v1", "pods", true),
        "Service" => ("v1", "", "v1", "services", true),
        "ConfigMap" => ("v1", "", "v1", "configmaps", true),
        "Secret" => ("v1", "", "v1", "secrets", true),
        "PersistentVolumeClaim" => ("v1", "", "v1", "persistentvolumeclaims", true),
        "Namespace" => ("v1", "", "v1", "namespaces", false),
        "Node" => ("v1", "", "v1", "nodes", false),
        "PersistentVolume" => ("v1", "", "v1", "persistentvolumes", false),
        "Deployment" => ("apps/v1", "apps", "v1", "deployments", true),
        "StatefulSet" => ("apps/v1", "apps", "v1", "statefulsets", true),
        "DaemonSet" => ("apps/v1", "apps", "v1", "daemonsets", true),
        "Ingress" => (
            "networking.k8s.io/v1",
            "networking.k8s.io",
            "v1",
            "ingresses",
            true,
        ),
        "Job" => ("batch/v1", "batch", "v1", "jobs", true),
        "CronJob" => ("batch/v1", "batch", "v1", "cronjobs", true),
        "StorageClass" => (
            "storage.k8s.io/v1",
            "storage.k8s.io",
            "v1",
            "storageclasses",
            false,
        ),
        _ => return None,
    };

    Some(WatchResourceKind {
        kind: kind.to_string(),
        group: Some(group.to_string()),
        version: Some(version.to_string()),
        api_version: Some(api_version.to_string()),
        plural: Some(plural.to_string()),
        namespaced: Some(namespaced),
    })
}

pub(super) fn normalize_resource_kind(
    kind: &WatchResourceKind,
) -> Result<WatchResourceKind, AppError> {
    if kind.kind.trim().is_empty() {
        return Err(AppError::new("resource kind is required", "validation"));
    }

    if let (Some(version), Some(api_version), Some(plural), Some(namespaced)) = (
        &kind.version,
        &kind.api_version,
        &kind.plural,
        kind.namespaced,
    ) {
        let group = kind
            .group
            .clone()
            .unwrap_or_else(|| group_from_api_version(api_version));
        return Ok(WatchResourceKind {
            kind: kind.kind.clone(),
            group: Some(group),
            version: Some(version.clone()),
            api_version: Some(api_version.clone()),
            plural: Some(plural.clone()),
            namespaced: Some(namespaced),
        });
    }

    known_resource_kind(&kind.kind).ok_or_else(|| {
        AppError::new(
            format!("watch metadata missing for {}", kind.kind),
            "validation",
        )
    })
}

fn group_from_api_version(api_version: &str) -> String {
    api_version
        .split_once('/')
        .map(|(group, _)| group.to_string())
        .unwrap_or_default()
}

pub(super) fn api_resource_from_kind(kind: &WatchResourceKind) -> Result<ApiResource, AppError> {
    let resource_kind = normalize_resource_kind(kind)?;
    Ok(ApiResource {
        group: resource_kind.group.unwrap_or_default(),
        version: resource_kind
            .version
            .ok_or_else(|| AppError::new("resource version is required", "validation"))?,
        api_version: resource_kind
            .api_version
            .ok_or_else(|| AppError::new("resource apiVersion is required", "validation"))?,
        kind: resource_kind.kind,
        plural: resource_kind
            .plural
            .ok_or_else(|| AppError::new("resource plural is required", "validation"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_resource_kind_fills_watch_metadata() {
        let pod = normalize_resource_kind(&WatchResourceKind {
            kind: "Pod".to_string(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: None,
        })
        .expect("pod metadata");

        assert_eq!(pod.api_version.as_deref(), Some("v1"));
        assert_eq!(pod.plural.as_deref(), Some("pods"));
        assert_eq!(pod.namespaced, Some(true));
    }

    #[test]
    fn rejects_unknown_kind_without_watch_metadata() {
        let err = normalize_resource_kind(&WatchResourceKind {
            kind: "Widget".to_string(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: None,
        })
        .expect_err("missing dynamic metadata should fail");

        assert_eq!(err.kind, "validation");
    }

    #[test]
    fn dynamic_kind_derives_group_from_api_version() {
        let deployment = normalize_resource_kind(&WatchResourceKind {
            kind: "Deployment".to_string(),
            group: None,
            version: Some("v1".to_string()),
            api_version: Some("apps/v1".to_string()),
            plural: Some("deployments".to_string()),
            namespaced: Some(true),
        })
        .expect("dynamic metadata");

        assert_eq!(deployment.group.as_deref(), Some("apps"));
    }

    #[test]
    fn dynamic_core_kind_keeps_empty_group() {
        let pod = normalize_resource_kind(&WatchResourceKind {
            kind: "Pod".to_string(),
            group: None,
            version: Some("v1".to_string()),
            api_version: Some("v1".to_string()),
            plural: Some("pods".to_string()),
            namespaced: Some(true),
        })
        .expect("core metadata");

        assert_eq!(pod.group.as_deref(), Some(""));
    }

    #[test]
    fn known_cluster_scoped_kind_sets_namespaced_false() {
        let node = normalize_resource_kind(&WatchResourceKind {
            kind: "Node".to_string(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: None,
        })
        .expect("node metadata");

        assert_eq!(node.api_version.as_deref(), Some("v1"));
        assert_eq!(node.plural.as_deref(), Some("nodes"));
        assert_eq!(node.namespaced, Some(false));
    }
}
