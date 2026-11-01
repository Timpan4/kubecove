//! Discovery of the resources in a few named API groups.

use crate::models::AppError;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::APIGroup;
use kube::{
    core::GroupVersion,
    discovery::{self, ApiCapabilities, ApiGroup, ApiResource},
    Client,
};
use std::collections::BTreeSet;

/// The resources that the cluster serves in `groups`, one per kind, each at
/// the highest-priority available version that serves that kind.
///
/// This reads the per-group discovery endpoints because kube's aggregated
/// discovery parser fails for the whole cluster when any API publishes
/// `"verbs": null`, as `subresources.kubevirt.io` does. Any error from the
/// group lists fails the call. A non-core version whose aggregated API server
/// answers 503 is skipped, as aggregated discovery leaves out unavailable APIs,
/// so a group with no available version contributes nothing.
pub async fn discover_resources(
    client: &Client,
    groups: &[&str],
) -> Result<Vec<(ApiResource, ApiCapabilities)>, AppError> {
    let mut resources = Vec::new();
    if groups.contains(&ApiGroup::CORE_GROUP) {
        let core = discovery::group(client, ApiGroup::CORE_GROUP).await?;
        resources.extend(core.recommended_resources());
    }
    if groups.iter().all(|group| group.is_empty()) {
        return Ok(resources);
    }
    let served = client.list_api_groups().await?;
    for group in served
        .groups
        .iter()
        .filter(|group| groups.contains(&group.name.as_str()))
    {
        resources.extend(group_resources(client, group).await?);
    }
    Ok(resources)
}

/// Each kind in `group` once, from the first available version that serves
/// it: the preferred version, then the others in the server's listed order.
async fn group_resources(
    client: &Client,
    group: &APIGroup,
) -> Result<Vec<(ApiResource, ApiCapabilities)>, AppError> {
    let preferred = group
        .preferred_version
        .as_ref()
        .map(|version| version.version.as_str());
    let others = group
        .versions
        .iter()
        .map(|version| version.version.as_str())
        .filter(|version| Some(*version) != preferred);
    let mut kinds = BTreeSet::new();
    let mut resources = Vec::new();
    for version in preferred.into_iter().chain(others) {
        match discovery::pinned_group(client, &GroupVersion::gv(&group.name, version)).await {
            Ok(api_group) => {
                for (resource, capabilities) in api_group.recommended_resources() {
                    if kinds.insert(resource.kind.clone()) {
                        resources.push((resource, capabilities));
                    }
                }
            }
            Err(kube::Error::Api(status)) if status.code == 503 => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(resources)
}
