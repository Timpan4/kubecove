use crate::commands::helpers::k8s_timestamp_to_datetime;
use crate::models::{HealthAssessment, OwnerReferenceSummary, ResourceHealth, ResourceSummary};

mod flux_inventory;
mod ownership;

pub use flux_inventory::{
    enrich_resource_summaries_with_flux_inventory, fetch_flux_ownership_index,
    filter_flux_ownership_index, read_flux_ownership_index, FluxOwnershipIndex,
};
#[cfg(feature = "bench-support")]
pub use flux_inventory::{
    flux_ownership_index_from_fetched_objects, MAX_FLUX_INVENTORY_ENTRIES, MAX_FLUX_OWNER_OBJECTS,
};
pub use ownership::extract_git_ops_owner;

const LABEL_HELM_RELEASE_NAME: &str = "helm.sh/release";

pub fn extract_owner_ref(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| refs.iter().next())
        .map(|r| r.name.clone())
}

pub fn extract_owner_ref_summary(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<OwnerReferenceSummary> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| {
            refs.iter()
                .find(|reference| reference.controller == Some(true))
        })
        .map(|r| OwnerReferenceSummary {
            api_version: r.api_version.clone(),
            kind: r.kind.clone(),
            name: r.name.clone(),
            uid: r.uid.clone(),
        })
}

pub fn extract_helm_release(
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
) -> Option<String> {
    metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(LABEL_HELM_RELEASE_NAME))
        .cloned()
}

/// Identity, timestamps and ownership evidence shared by every summary of a typed object.
pub fn base_resource_summary(
    kind: &str,
    api_version: &str,
    cluster: &str,
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
    age: String,
    health_assessment: HealthAssessment,
) -> ResourceSummary {
    // Argo CD ownership is the git-ops owner when present, so derive it instead of rebuilding it.
    let git_ops_owner = extract_git_ops_owner(metadata);
    let argo_app = git_ops_owner
        .as_ref()
        .filter(|owner| owner.provider == "argo")
        .map(|owner| owner.name.clone());
    ResourceSummary {
        kind: kind.to_string(),
        cluster: cluster.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone(),
        age,
        api_version: Some(api_version.to_string()),
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        health: ResourceHealth::Unknown,
        health_assessment,
        created_at: metadata
            .creation_timestamp
            .as_ref()
            .and_then(|t| k8s_timestamp_to_datetime(&t.0).map(|dt| dt.to_rfc3339())),
        status: None,
        ready: None,
        restarts: None,
        owner_ref: extract_owner_ref(metadata),
        argo_app,
        helm_release: extract_helm_release(metadata),
        git_ops_owner,
        git_ops_ownership_partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};

    #[test]
    fn owner_ref_summary_is_absent_without_owner_references() {
        let metadata = ObjectMeta {
            name: Some("api".to_string()),
            ..Default::default()
        };

        assert_eq!(extract_owner_ref_summary(&metadata), None);
    }

    #[test]
    fn owner_ref_summary_selects_controller_reference() {
        let metadata = ObjectMeta {
            owner_references: Some(vec![
                OwnerReference {
                    api_version: "example/v1".to_string(),
                    kind: "Observer".to_string(),
                    name: "audit".to_string(),
                    uid: "uid-observer".to_string(),
                    controller: Some(false),
                    ..Default::default()
                },
                OwnerReference {
                    api_version: "apps/v1".to_string(),
                    kind: "ReplicaSet".to_string(),
                    name: "api-abc".to_string(),
                    uid: "uid-controller".to_string(),
                    controller: Some(true),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        assert_eq!(extract_owner_ref(&metadata), Some("audit".to_string()));
        assert_eq!(
            extract_owner_ref_summary(&metadata),
            Some(crate::models::OwnerReferenceSummary {
                api_version: "apps/v1".to_string(),
                kind: "ReplicaSet".to_string(),
                name: "api-abc".to_string(),
                uid: "uid-controller".to_string(),
            })
        );
    }

    #[test]
    fn owner_ref_summary_is_absent_without_controller_reference() {
        let metadata = ObjectMeta {
            owner_references: Some(vec![OwnerReference {
                api_version: "example/v1".to_string(),
                kind: "Observer".to_string(),
                name: "audit".to_string(),
                uid: "uid-observer".to_string(),
                controller: Some(false),
                ..Default::default()
            }]),
            ..Default::default()
        };

        assert_eq!(extract_owner_ref(&metadata), Some("audit".to_string()));
        assert_eq!(extract_owner_ref_summary(&metadata), None);
    }
}
