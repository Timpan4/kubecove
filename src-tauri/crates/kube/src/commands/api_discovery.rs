//! Discovery of a few named API groups.

use crate::models::AppError;
use kube::{
    discovery::{self, ApiGroup},
    error::DiscoveryError,
    Client,
};

/// Discovers each of `groups` through its per-group endpoints.
///
/// kube's aggregated discovery parser fails for the whole cluster when any API
/// publishes `"verbs": null`, as `subresources.kubevirt.io` does, so this does
/// not use it. A group the cluster does not serve, or whose aggregated API
/// server answers 503, is left out, as aggregated discovery leaves out
/// unavailable APIs. Any other error fails the call.
pub async fn discover_groups(client: &Client, groups: &[&str]) -> Result<Vec<ApiGroup>, AppError> {
    let mut found = Vec::with_capacity(groups.len());
    for group in groups {
        match discovery::group(client, group).await {
            Ok(api_group) => found.push(api_group),
            Err(kube::Error::Discovery(DiscoveryError::MissingApiGroup(_))) => {}
            Err(kube::Error::Api(status)) if status.code == 503 => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(found)
}
