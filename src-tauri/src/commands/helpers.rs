use crate::models::{AppError, ResourceSummary};
use chrono::{DateTime, TimeZone, Utc};
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};
use kube::{
    api::{Api, ListParams, Resource},
    Client,
};
use serde::{de::DeserializeOwned, Serialize};

pub(crate) fn list_params() -> ListParams {
    ListParams::default()
}

/// ARGO CD ANNOTATIONS AND LABEL KEYS
const ANNOTATION_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/name";
const ANNOTATION_ARGOCD_TRACKING_ID: &str = "argocd.argoproj.io/tracking-id";
const LABEL_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/application";
const LABEL_APP_KUBERNETES_IO_INSTANCE: &str = "app.kubernetes.io/instance";
const LABEL_HELM_RELEASE_NAME: &str = "helm.sh/release";

/// Extract the name of the first owner reference, if any.
pub(crate) fn extract_owner_ref(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| refs.iter().next())
        .map(|r| r.name.clone())
}

/// Check labels and annotations for Argo CD application tracking signals.
pub(crate) fn argo_app_from_tracking_id(tracking_id: &str) -> Option<String> {
    tracking_id
        .split_once(':')
        .map(|(app, _)| app)
        .filter(|app| !app.is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_argo_app(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    // Try annotation: argocd.argoproj.io/name or argocd.argoproj.io/tracking-id
    if let Some(annotations) = &metadata.annotations {
        if let Some(name) = annotations.get(ANNOTATION_ARGOCD_APP_NAME) {
            return Some(name.clone());
        }
        if let Some(id) = annotations.get(ANNOTATION_ARGOCD_TRACKING_ID) {
            if let Some(app) = argo_app_from_tracking_id(id) {
                return Some(app);
            }
        }
    }
    // Try label: argocd.argoproj.io/application
    if let Some(labels) = &metadata.labels {
        if let Some(name) = labels.get(LABEL_ARGOCD_APP_NAME) {
            return Some(name.clone());
        }
        if let Some(instance) = labels.get(LABEL_APP_KUBERNETES_IO_INSTANCE) {
            return Some(instance.clone());
        }
    }
    None
}

/// Check labels for Helm release signals.
pub(crate) fn extract_helm_release(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(LABEL_HELM_RELEASE_NAME))
        .cloned()
}

/// Build a base ResourceSummary with all Milestone-2 metadata fields pre-populated
/// from common metadata (owner refs, Argo CD, Helm). Individual resource blocks
/// override the status-specific fields after calling this.
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
    }
}

/// Helper to unwrap an Option<i32> for formatting, defaulting to 0.
pub(crate) fn opt_i32_to_str(opt: Option<i32>) -> String {
    opt.map(|v| v.to_string())
        .unwrap_or_else(|| "0".to_string())
}

/// Helper to format any integer-like value for ready count display.
pub(crate) fn fmt_ready(ready: Option<i32>, desired: i32) -> String {
    format!("{}/{}", opt_i32_to_str(ready), desired)
}

/// Redact secret payload fields to prevent exposure to frontend.
pub(crate) fn redact_secret(secret: &mut k8s_openapi::api::core::v1::Secret) {
    if let Some(ref mut data) = secret.data {
        for value in data.values_mut() {
            *value = k8s_openapi::ByteString(b"REDACTED".to_vec());
        }
    }
    if let Some(ref mut string_data) = secret.string_data {
        for value in string_data.values_mut() {
            *value = "REDACTED".to_string();
        }
    }
}

/// Fetch a namespaced resource and serialize it to YAML.
pub(crate) async fn fetch_and_serialize<
    T: Resource<Scope = NamespaceResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    namespace: Option<&str>,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    let resource = api
        .get(name)
        .await
        .map_err(|e: kube::Error| AppError::kube(e.to_string()))?;
    let yaml = serde_yaml::to_string(&resource)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    Ok((resource, yaml))
}

/// Fetch a cluster-scoped resource and serialize it to YAML.
pub(crate) async fn fetch_and_serialize_cluster<
    T: Resource<Scope = ClusterResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = Api::all(client);
    let resource = api
        .get(name)
        .await
        .map_err(|e: kube::Error| AppError::kube(e.to_string()))?;
    let yaml = serde_yaml::to_string(&resource)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    Ok((resource, yaml))
}

pub(crate) fn resource_age(creation_timestamp: Option<DateTime<Utc>>) -> String {
    match creation_timestamp {
        Some(t) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(t);
            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                "<1m".to_string()
            }
        }
        None => "unknown".to_string(),
    }
}

pub(crate) fn k8s_timestamp_to_datetime(
    timestamp: &k8s_openapi::jiff::Timestamp,
) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(timestamp.as_second(), 0).single()
}

pub(crate) fn k8s_creation_timestamp_to_rfc3339(
    timestamp: &Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>,
) -> Option<String> {
    timestamp
        .as_ref()
        .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        .map(|dt| dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::Secret;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
    use k8s_openapi::ByteString;
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
        assert_eq!(extract_argo_app(&metadata), Some("payments".to_string()));
        assert_eq!(
            extract_helm_release(&metadata),
            Some("payments-api".to_string())
        );
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
    fn redacts_secret_data_and_string_data() {
        let mut data = BTreeMap::new();
        data.insert("password".to_string(), ByteString(b"super-secret".to_vec()));
        let mut string_data = BTreeMap::new();
        string_data.insert("token".to_string(), "plain-secret".to_string());
        let mut secret = Secret {
            data: Some(data),
            string_data: Some(string_data),
            ..Default::default()
        };

        redact_secret(&mut secret);

        assert_eq!(
            secret.data.as_ref().unwrap().get("password").unwrap().0,
            b"REDACTED".to_vec()
        );
        assert_eq!(
            secret.string_data.as_ref().unwrap().get("token").unwrap(),
            "REDACTED"
        );
    }
}
