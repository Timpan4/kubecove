use crate::models::{AppError, DiscoveredResourceKind, HelmManifestResourceSummary};
use kube::{
    Client,
    api::{Api, DynamicObject, ListParams},
    discovery::{Scope, verbs},
};
use kubecove_kube::commands::discover_resources;
use kubecove_kube::commands::dynamic_projection::api_resource_from_discovered;
use std::collections::{BTreeMap, BTreeSet};

/// Built-in kinds the label scan covers even when the manifest omits them, as
/// `(group, kind)`. A same-named kind in another group is scanned only when the
/// manifest references it.
const COMMON_LABEL_SCAN_KINDS: &[(&str, &str)] = &[
    ("", "Pod"),
    ("apps", "Deployment"),
    ("apps", "StatefulSet"),
    ("apps", "DaemonSet"),
    ("", "Service"),
    ("networking.k8s.io", "Ingress"),
    ("", "ConfigMap"),
    ("", "Secret"),
    ("", "PersistentVolumeClaim"),
    ("batch", "Job"),
    ("batch", "CronJob"),
];

/// Discovers only the groups the manifest and the label scan use, with a
/// warning for each version that was unavailable. A manifest group that is not
/// served or is unavailable is left out, so only its own rows read as
/// unavailable.
pub(super) async fn discovered_resources(
    client: Client,
    manifest_refs: &[HelmManifestResourceSummary],
) -> Result<(Vec<DiscoveredResourceKind>, Vec<String>), AppError> {
    let mut groups: BTreeSet<&str> = COMMON_LABEL_SCAN_KINDS
        .iter()
        .map(|(group, _)| *group)
        .collect();
    groups.extend(manifest_refs.iter().filter_map(|manifest_ref| {
        let api_version = manifest_ref.api_version.as_deref()?;
        Some(api_version.rsplit_once('/').map_or("", |(group, _)| group))
    }));
    let groups: Vec<&str> = groups.into_iter().collect();
    let discovered = discover_resources(&client, &groups).await?;
    let warnings = discovered
        .unavailable_versions
        .iter()
        .map(|api_version| {
            format!(
                "API {api_version} is unavailable, so resources at that version were not checked."
            )
        })
        .collect();
    let mut kinds = Vec::new();
    for (api_resource, capabilities) in discovered.resources {
        if !capabilities.supports_operation(verbs::GET)
            && !capabilities.supports_operation(verbs::LIST)
        {
            continue;
        }
        kinds.push(DiscoveredResourceKind {
            group: api_resource.group,
            version: api_resource.version,
            api_version: api_resource.api_version,
            kind: api_resource.kind,
            plural: api_resource.plural,
            short_names: Vec::new(),
            namespaced: matches!(capabilities.scope, Scope::Namespaced),
        });
    }
    kinds.sort_by(|a, b| {
        a.api_version
            .cmp(&b.api_version)
            .then(a.kind.cmp(&b.kind))
            .then(a.plural.cmp(&b.plural))
    });
    kinds.dedup_by(|a, b| a.api_version == b.api_version && a.kind == b.kind);
    Ok((kinds, warnings))
}

pub(super) async fn label_scan(
    client: Client,
    namespace: &str,
    kind: &DiscoveredResourceKind,
    params: &ListParams,
) -> Result<Vec<DynamicObject>, String> {
    let api_resource = api_resource_from_discovered(kind).map_err(|err| {
        format!(
            "Could not scan {} {}: {}",
            kind.api_version, kind.kind, err.message
        )
    })?;
    let api: Api<DynamicObject> = if kind.namespaced {
        Api::namespaced_with(client, namespace, &api_resource)
    } else {
        Api::all_with(client, &api_resource)
    };
    api.list(params)
        .await
        .map(|items| items.items)
        .map_err(|err| {
            format!(
                "Could not scan {} {} for explicit Helm labels: {err}",
                kind.api_version, kind.kind
            )
        })
}

pub(super) fn conservative_label_scan_kinds(
    discovered: &[DiscoveredResourceKind],
    manifest_scan_kinds: &BTreeSet<(String, String)>,
) -> Vec<DiscoveredResourceKind> {
    let mut by_key = BTreeMap::new();
    for kind in discovered {
        let common = COMMON_LABEL_SCAN_KINDS.contains(&(kind.group.as_str(), kind.kind.as_str()));
        if manifest_scan_kinds.contains(&(kind.api_version.clone(), kind.kind.clone()))
            || (kind.namespaced && common)
        {
            by_key.insert((kind.api_version.clone(), kind.kind.clone()), kind.clone());
        }
    }
    by_key.into_values().collect()
}
