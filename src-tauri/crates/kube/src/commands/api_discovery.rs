//! Discovery of the resources in a few named API groups.

use crate::models::AppError;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::APIGroup;
use kube::{
    Client,
    core::GroupVersion,
    discovery::{self, ApiCapabilities, ApiGroup, ApiResource},
};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct DiscoveredResources {
    /// One entry per kind, at the highest-priority available version that
    /// serves it.
    pub resources: Vec<(ApiResource, ApiCapabilities)>,
    /// Each `group/version` that answered 503 and was skipped, sorted.
    pub unavailable_versions: Vec<String>,
}

/// The resources that the cluster serves in `groups`.
///
/// This reads the per-group discovery endpoints because kube's aggregated
/// discovery parser fails for the whole cluster when any API publishes
/// `"verbs": null`, as `subresources.kubevirt.io` does. Any error from the
/// group lists fails the call. A non-core version whose aggregated API server
/// answers 503 is skipped and reported, as aggregated discovery leaves out
/// unavailable APIs.
pub async fn discover_resources(
    client: &Client,
    groups: &[&str],
) -> Result<DiscoveredResources, AppError> {
    let mut discovered = DiscoveredResources::default();
    if groups.contains(&ApiGroup::CORE_GROUP) {
        let core = discovery::group(client, ApiGroup::CORE_GROUP).await?;
        discovered.resources.extend(core.recommended_resources());
    }
    if groups.iter().all(|group| group.is_empty()) {
        return Ok(discovered);
    }
    let served = client.list_api_groups().await?;
    for group in served
        .groups
        .iter()
        .filter(|group| groups.contains(&group.name.as_str()))
    {
        add_group_resources(client, group, &mut discovered).await?;
    }
    discovered.unavailable_versions.sort_unstable();
    Ok(discovered)
}

/// Adds each kind in `group` once, from the first available version that
/// serves it: the preferred version, then the others in the server's order.
async fn add_group_resources(
    client: &Client,
    group: &APIGroup,
    discovered: &mut DiscoveredResources,
) -> Result<(), AppError> {
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
    for version in preferred.into_iter().chain(others) {
        let group_version = GroupVersion::gv(&group.name, version);
        match discovery::pinned_group(client, &group_version).await {
            Ok(api_group) => {
                for (resource, capabilities) in api_group.recommended_resources() {
                    if kinds.insert(resource.kind.clone()) {
                        discovered.resources.push((resource, capabilities));
                    }
                }
            }
            Err(kube::Error::Api(status)) if status.code == 503 => {
                discovered
                    .unavailable_versions
                    .push(group_version.api_version());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
