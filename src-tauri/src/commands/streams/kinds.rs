use crate::commands::builtin_kinds::builtin_kind;
use crate::models::AppErrorKind;
use crate::models::{AppError, WatchResourceKind};
use kube::api::ApiResource;

fn known_resource_kind(kind: &str) -> Option<WatchResourceKind> {
    builtin_kind(kind).map(|entry| WatchResourceKind {
        kind: entry.kind.to_string(),
        group: Some(entry.group.to_string()),
        version: Some(entry.version.to_string()),
        api_version: Some(entry.api_version()),
        plural: Some(entry.plural.to_string()),
        namespaced: Some(entry.namespaced),
    })
}

pub(super) fn normalize_resource_kind(
    kind: &WatchResourceKind,
) -> Result<WatchResourceKind, AppError> {
    if kind.kind.trim().is_empty() {
        return Err(AppError::new(
            "resource kind is required",
            AppErrorKind::Validation,
        ));
    }

    if let (Some(api_version), Some(plural), Some(namespaced)) =
        (&kind.api_version, &kind.plural, kind.namespaced)
    {
        let group = kind
            .group
            .clone()
            .unwrap_or_else(|| group_from_api_version(api_version));
        return Ok(WatchResourceKind {
            kind: kind.kind.clone(),
            group: Some(group),
            version: Some(kind.version.clone().unwrap_or_else(|| {
                api_version
                    .rsplit('/')
                    .next()
                    .unwrap_or(api_version)
                    .to_string()
            })),
            api_version: Some(api_version.clone()),
            plural: Some(plural.clone()),
            namespaced: Some(namespaced),
        });
    }

    known_resource_kind(&kind.kind).ok_or_else(|| {
        AppError::new(
            format!("watch metadata missing for {}", kind.kind),
            AppErrorKind::Validation,
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
    let resource = ApiResource {
        group: resource_kind.group.unwrap_or_default(),
        version: resource_kind.version.ok_or_else(|| {
            AppError::new("resource version is required", AppErrorKind::Validation)
        })?,
        api_version: resource_kind.api_version.ok_or_else(|| {
            AppError::new("resource apiVersion is required", AppErrorKind::Validation)
        })?,
        kind: resource_kind.kind,
        plural: resource_kind.plural.ok_or_else(|| {
            AppError::new("resource plural is required", AppErrorKind::Validation)
        })?,
    };
    crate::commands::helpers::validate_api_resource(&resource)?;
    Ok(resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitops_watch_derives_version_from_api_version() {
        let kind = api_resource_from_kind(&WatchResourceKind {
            kind: "Application".into(),
            group: None,
            version: None,
            api_version: Some("argoproj.io/v1alpha1".into()),
            plural: Some("applications".into()),
            namespaced: Some(true),
        })
        .expect("GitOps watch metadata");
        assert_eq!(kind.group, "argoproj.io");
        assert_eq!(kind.version, "v1alpha1");
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

        assert_eq!(err.kind, AppErrorKind::Validation);
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
}
