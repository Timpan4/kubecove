//! Discovery of a few named API groups.

use crate::models::AppError;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::APIGroup;
use kube::{
    core::GroupVersion,
    discovery::{self, ApiGroup},
    Client,
};

/// Discovers each of `groups` that the cluster serves, at its preferred
/// version, through the per-group discovery endpoints.
///
/// kube's aggregated discovery parser fails for the whole cluster when any API
/// publishes `"verbs": null`, as `subresources.kubevirt.io` does, so this does
/// not use it. Any error from the group lists fails the call. A non-core
/// version whose aggregated API server answers 503 is skipped for the group's
/// next version, and a group with no available version is left out, as
/// aggregated discovery leaves out unavailable APIs.
pub async fn discover_groups(client: &Client, groups: &[&str]) -> Result<Vec<ApiGroup>, AppError> {
    let mut found = Vec::with_capacity(groups.len());
    if groups.contains(&ApiGroup::CORE_GROUP) {
        found.push(discovery::group(client, ApiGroup::CORE_GROUP).await?);
    }
    if groups.iter().all(|group| group.is_empty()) {
        return Ok(found);
    }
    let served = client.list_api_groups().await?;
    for group in served
        .groups
        .iter()
        .filter(|group| groups.contains(&group.name.as_str()))
    {
        if let Some(api_group) = first_available_version(client, group).await? {
            found.push(api_group);
        }
    }
    Ok(found)
}

/// The group at its preferred version, or at the next listed version when the
/// preferred one answers 503.
async fn first_available_version(
    client: &Client,
    group: &APIGroup,
) -> Result<Option<ApiGroup>, AppError> {
    let preferred = group
        .preferred_version
        .as_ref()
        .map(|version| version.version.as_str());
    let others = group
        .versions
        .iter()
        .map(|version| version.version.as_str())
        .filter(|version| Some(*version) != preferred);
    for version in preferred.into_iter().chain(others) {
        match discovery::pinned_group(client, &GroupVersion::gv(&group.name, version)).await {
            Ok(api_group) => return Ok(Some(api_group)),
            Err(kube::Error::Api(status)) if status.code == 503 => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}
