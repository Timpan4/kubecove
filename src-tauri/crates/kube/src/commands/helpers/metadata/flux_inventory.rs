use crate::commands::flux_catalog::FLUX_KINDS;
use crate::models::AppErrorKind;
use crate::models::{AppError, GitOpsOwnerSummary, ResourceSummary};
use kube::{
    Client,
    api::{Api, ApiResource, DynamicObject, ListParams},
};
use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::{Hash, Hasher},
};

pub const MAX_FLUX_OWNER_OBJECTS: usize = 64;
pub const MAX_FLUX_INVENTORY_ENTRIES: usize = 2048;

#[derive(Debug, Clone, Default)]
pub struct FluxOwnershipIndex {
    owners: HashMap<ResourceKey<String>, GitOpsOwnerSummary>,
    pub partial: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ResourceKey<S> {
    namespace: Option<S>,
    api_version: S,
    kind: S,
    name: S,
}

// The same identity can be hashed without allocating the strings of the owned key.
trait ResourceIdentity {
    fn key(&self) -> ResourceKey<&str>;
}

impl<S: AsRef<str>> ResourceIdentity for ResourceKey<S> {
    fn key(&self) -> ResourceKey<&str> {
        ResourceKey {
            namespace: self.namespace.as_ref().map(AsRef::as_ref),
            api_version: self.api_version.as_ref(),
            kind: self.kind.as_ref(),
            name: self.name.as_ref(),
        }
    }
}

impl Hash for dyn ResourceIdentity + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl PartialEq for dyn ResourceIdentity + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for dyn ResourceIdentity + '_ {}

impl<'a> Borrow<dyn ResourceIdentity + 'a> for ResourceKey<String> {
    fn borrow(&self) -> &(dyn ResourceIdentity + 'a) {
        self
    }
}

impl FluxOwnershipIndex {
    fn owner(&self, summary: &ResourceSummary) -> Option<&GitOpsOwnerSummary> {
        let key = ResourceKey {
            namespace: summary.namespace.as_deref(),
            api_version: summary.api_version.as_deref().unwrap_or_default(),
            kind: summary.kind.as_str(),
            name: summary.name.as_str(),
        };
        let identity: &dyn ResourceIdentity = &key;
        self.owners.get(identity)
    }

    /// Copies a row only when scoped Flux inventory changes its ownership or partialness.
    pub fn enriched_summary(
        &self,
        summary: &ResourceSummary,
        namespaces: &[String],
    ) -> Option<ResourceSummary> {
        let owner = if summary.git_ops_owner.is_none() {
            self.owner(summary)
                .filter(|_| {
                    namespaces.is_empty()
                        || target_namespace_allowed(summary.namespace.as_deref(), namespaces)
                })
                .cloned()
                .map(|mut owner| {
                    owner.partial = self.partial;
                    owner
                })
        } else {
            None
        };

        let needs_partial = self.partial
            && (!summary.git_ops_ownership_partial
                || summary
                    .git_ops_owner
                    .as_ref()
                    .is_some_and(|owner| !owner.partial));
        if owner.is_none() && !needs_partial {
            return None;
        }

        let mut summary = summary.clone();
        apply_enrichment(&mut summary, owner, self.partial);
        Some(summary)
    }

    pub fn retained_item_count(&self) -> usize {
        self.owners.len()
    }

    pub(crate) fn shallow_payload_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.owners.len() * std::mem::size_of::<(ResourceKey<String>, GitOpsOwnerSummary)>()
    }
}

pub(crate) fn parse_inventory_id(id: &str) -> Option<(Option<String>, String, String, String)> {
    let mut parts = id.split('_');
    let namespace = parts.next()?.trim();
    let name = parts.next()?.trim();
    let mut rest: Vec<_> = parts.collect();
    let kind = rest.pop()?.trim();
    let group = rest.join("_");
    if name.is_empty() || kind.is_empty() {
        return None;
    }
    Some((
        (!namespace.is_empty()).then(|| namespace.to_string()),
        name.to_string(),
        group,
        kind.to_string(),
    ))
}

fn normalized_api_version(group: &str, version: Option<&str>) -> Option<String> {
    let version = version?.trim();
    if version.is_empty() {
        return None;
    }
    let group = group.trim();
    Some(if group.is_empty() {
        version.to_string()
    } else {
        format!("{group}/{version}")
    })
}

fn has_continuation_token(token: Option<&str>) -> bool {
    token.is_some_and(|token| !token.is_empty())
}

fn target_namespace_allowed(namespace: Option<&str>, requested: &[String]) -> bool {
    namespace.is_none()
        || requested
            .iter()
            .any(|wanted| Some(wanted.as_str()) == namespace)
}

pub async fn read_flux_ownership_index(
    client: Client,
    namespaces: &[String],
) -> Result<FluxOwnershipIndex, AppError> {
    let index = fetch_flux_ownership_index(client).await?;
    Ok(filter_flux_ownership_index(index, namespaces))
}

/// Lists Flux owner objects cluster-wide so callers can cache the index and
/// scope it per request without re-listing.
pub async fn fetch_flux_ownership_index(client: Client) -> Result<FluxOwnershipIndex, AppError> {
    // The owner kinds are listed together and indexed in catalog order. A
    // cancelled list ends the read at once; other failures mark it partial.
    let lists = FLUX_KINDS
        .iter()
        .filter(|(_, _, kind, _, _, _)| *kind == "Kustomization" || *kind == "HelmRelease")
        .map(|(group, version, kind, plural, _, _)| {
            let resource = ApiResource {
                group: group.to_string(),
                version: version.to_string(),
                api_version: format!("{group}/{version}"),
                kind: kind.to_string(),
                plural: plural.to_string(),
            };
            // Owners are cluster-wide evidence; target namespaces only filter retained inventory identities.
            let api = Api::<DynamicObject>::all_with(client.clone(), &resource);
            async move {
                match api
                    .list(&ListParams::default().limit(MAX_FLUX_OWNER_OBJECTS as u32))
                    .await
                {
                    Ok(rows) => Ok((
                        *kind,
                        rows.items,
                        has_continuation_token(rows.metadata.continue_.as_deref()),
                    )),
                    Err(e) => {
                        let error = AppError::from(e);
                        if error.kind == AppErrorKind::Cancelled {
                            return Err(error);
                        }
                        Ok((*kind, Vec::new(), true))
                    }
                }
            }
        });
    let mut index = FluxOwnershipIndex::default();
    for (kind, objects, batch_partial) in futures_util::future::try_join_all(lists).await? {
        index_flux_owner_objects(&mut index, kind, objects, batch_partial);
    }
    Ok(index)
}

fn index_flux_owner_objects(
    index: &mut FluxOwnershipIndex,
    kind: &str,
    objects: Vec<DynamicObject>,
    batch_partial: bool,
) {
    index.partial |= batch_partial || objects.len() >= MAX_FLUX_OWNER_OBJECTS;
    for object in objects.into_iter().take(MAX_FLUX_OWNER_OBJECTS) {
        let Some(owner_name) = object.metadata.name.clone() else {
            index.partial = true;
            continue;
        };
        let owner_namespace = object.metadata.namespace.clone();
        let entries = object
            .data
            .get("status")
            .and_then(|s| s.get("inventory"))
            .and_then(|i| i.get("entries"))
            .and_then(|e| e.as_array());
        let Some(entries) = entries else {
            index.partial = true;
            continue;
        };
        if entries.len() > MAX_FLUX_INVENTORY_ENTRIES {
            index.partial = true;
        }
        for entry in entries.iter().take(MAX_FLUX_INVENTORY_ENTRIES) {
            let Some(id) = entry.get("id").and_then(|v| v.as_str()) else {
                index.partial = true;
                continue;
            };
            let Some((namespace, name, group, target_kind)) = parse_inventory_id(id) else {
                index.partial = true;
                continue;
            };
            let Some(api_version) = normalized_api_version(
                &group,
                entry
                    .get("version")
                    .or_else(|| entry.get("v"))
                    .and_then(|v| v.as_str()),
            ) else {
                index.partial = true;
                continue;
            };
            index
                .owners
                .entry(ResourceKey {
                    namespace,
                    api_version,
                    kind: target_kind,
                    name,
                })
                .or_insert_with(|| GitOpsOwnerSummary {
                    provider: "flux".into(),
                    kind: kind.to_string(),
                    name: owner_name.clone(),
                    namespace: owner_namespace.clone(),
                    confidence: "inventory".into(),
                    provenance: "flux-inventory".into(),
                    partial: true,
                });
        }
    }
}

#[cfg(feature = "bench-support")]
pub fn flux_ownership_index_from_fetched_objects(
    batches: Vec<(String, Vec<DynamicObject>)>,
) -> FluxOwnershipIndex {
    let mut index = FluxOwnershipIndex::default();
    for (kind, objects) in batches {
        index_flux_owner_objects(&mut index, &kind, objects, false);
    }
    index
}

/// Restricts a fetched index to owners whose inventoried targets live in the
/// requested namespaces and stamps owner visibility with index partialness.
pub fn filter_flux_ownership_index(
    mut index: FluxOwnershipIndex,
    namespaces: &[String],
) -> FluxOwnershipIndex {
    if !namespaces.is_empty() {
        index
            .owners
            .retain(|key, _| target_namespace_allowed(key.namespace.as_deref(), namespaces));
    }
    for owner in index.owners.values_mut() {
        owner.partial = index.partial;
    }
    index
}

pub fn enrich_resource_summaries_with_flux_inventory(
    summaries: &mut [ResourceSummary],
    index: &FluxOwnershipIndex,
) {
    for summary in summaries.iter_mut() {
        let owner = if summary.git_ops_owner.is_none() {
            index.owner(summary).cloned()
        } else {
            None
        };
        apply_enrichment(summary, owner, index.partial);
    }
}

fn apply_enrichment(
    summary: &mut ResourceSummary,
    new_owner: Option<GitOpsOwnerSummary>,
    partial: bool,
) {
    summary.git_ops_ownership_partial |= partial;
    if let Some(owner) = new_owner {
        summary.git_ops_owner = Some(owner);
    }
    if let Some(owner) = &mut summary.git_ops_owner {
        owner.partial |= partial;
    }
}

#[cfg(test)]
#[path = "flux_inventory_tests.rs"]
mod tests;
