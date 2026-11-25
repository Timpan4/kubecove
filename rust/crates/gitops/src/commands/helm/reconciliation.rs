use super::storage::get_helm_storage_record;
use crate::commands::ClusterAccess;
use crate::commands::helpers::list_params;
use crate::models::{
    AppError, DiscoveredResourceKind, HelmManifestResourceSummary, HelmReconciliationResource,
    HelmReconciliationStatus, HelmReconciliationTotals, HelmReleaseReconciliation,
    HelmReleaseSummary, ResourceSummary,
};
use futures_util::{StreamExt, future::Either, stream};
use kube::{
    Client, Error as KubeError,
    api::{Api, DynamicObject},
};
use kubecove_kube::commands::dynamic_projection::{
    api_resource_from_discovered, dynamic_resource_summary,
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "reconciliation_reads.rs"]
mod reads;
use reads::{conservative_label_scan_kinds, discovered_resources, label_scan};

const HELM_RELEASE_LABEL: &str = "helm.sh/release";
// Match the Kubernetes inspection read limit in inspection/src/commands/mod.rs.
const MAX_READ_CONCURRENCY: usize = 16;
const CONSERVATIVE_SCAN_WARNING: &str = "Label-only reconciliation scans explicit Helm release labels for manifest kinds and common built-in namespaced kinds; it does not exhaustively scan every API group or CRD.";
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResourceKey {
    api_version: String,
    kind: String,
    namespace: Option<String>,
    name: String,
}

#[derive(Debug, Clone)]
struct ResourceIdentity {
    key: ResourceKey,
    resource_kind: DiscoveredResourceKind,
}

struct PreparedManifest {
    reads: Vec<(ResourceIdentity, usize)>,
    unavailable_rows: Vec<HelmReconciliationResource>,
    scan_kinds: BTreeSet<(String, String)>,
}

pub async fn get_helm_release_reconciliation(
    cluster_context: String,
    namespace: String,
    storage_kind: String,
    storage_name: String,
    kubeconfig_env_var: Option<String>,
) -> Result<HelmReleaseReconciliation, AppError> {
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    helm_release_reconciliation_from(
        &access,
        &cluster_context,
        &namespace,
        &storage_kind,
        &storage_name,
    )
    .await
}

/// `get_helm_release_reconciliation` through an access the caller already resolved.
pub async fn helm_release_reconciliation_from(
    access: &ClusterAccess,
    cluster_context: &str,
    namespace: &str,
    storage_kind: &str,
    storage_name: &str,
) -> Result<HelmReleaseReconciliation, AppError> {
    let client = access.read_client(cluster_context).await?;
    let record = get_helm_storage_record(
        client.clone(),
        cluster_context,
        namespace,
        storage_kind,
        storage_name,
    )
    .await?;
    let (discovered, discovery_warnings) =
        discovered_resources(client.clone(), &record.manifest_resources).await?;
    let (resources, mut warnings, totals) = reconcile_release(
        client,
        cluster_context,
        &record.summary,
        &record.manifest_resources,
        &discovered,
    )
    .await;
    warnings.extend(discovery_warnings);

    Ok(HelmReleaseReconciliation {
        summary: record.summary,
        totals,
        resources,
        warnings,
    })
}

async fn reconcile_release(
    client: Client,
    cluster_context: &str,
    release: &HelmReleaseSummary,
    manifest_refs: &[HelmManifestResourceSummary],
    discovered: &[DiscoveredResourceKind],
) -> (
    Vec<HelmReconciliationResource>,
    Vec<String>,
    HelmReconciliationTotals,
) {
    let discovered_by_ref = discovered_ref_index(discovered);
    let prepared = prepare_manifest(manifest_refs, &release.namespace, &discovered_by_ref);
    let mut rows_by_key = BTreeMap::new();
    let mut warnings = vec![CONSERVATIVE_SCAN_WARNING.to_string()];
    let scan_kinds = match crate::commands::helpers::validate_namespace(Some(&release.namespace)) {
        Ok(()) => conservative_label_scan_kinds(discovered, &prepared.scan_kinds),
        Err(error) => {
            warnings.push(error.message);
            Vec::new()
        }
    };
    let params = list_params().labels(&format!("{HELM_RELEASE_LABEL}={}", release.name));
    let mut tasks = Vec::with_capacity(prepared.reads.len() + scan_kinds.len());
    let mut manifest_iter = prepared.reads.into_iter();
    let mut scan_iter = scan_kinds.into_iter();
    loop {
        match (manifest_iter.next(), scan_iter.next()) {
            (Some(manifest_read), Some(kind)) => {
                tasks.push(Either::Left(manifest_read));
                tasks.push(Either::Right(kind));
            }
            (Some(manifest_read), None) => tasks.push(Either::Left(manifest_read)),
            (None, Some(kind)) => tasks.push(Either::Right(kind)),
            (None, None) => break,
        }
    }
    let reads = stream::iter(tasks)
        .map(|read| {
            let client = client.clone();
            let params = &params;
            async move {
                match read {
                    Either::Left((identity, manifest_index)) => {
                        let row = live_row_for_manifest(
                            client,
                            cluster_context,
                            release,
                            &manifest_refs[manifest_index],
                            &identity,
                        )
                        .await;
                        Either::Left((identity.key, row))
                    }
                    Either::Right(kind) => {
                        let items = label_scan(client, &release.namespace, &kind, params).await;
                        Either::Right((kind, items))
                    }
                }
            }
        })
        .buffered(MAX_READ_CONCURRENCY);
    futures_util::pin_mut!(reads);
    // Ordered results keep the last manifest row and the first label-only row for each key.
    // A manifest row replaces a label-only row even when its GET failed.
    while let Some(read) = reads.next().await {
        match read {
            Either::Left((key, row)) => {
                rows_by_key.insert(key, row);
            }
            Either::Right((kind, Ok(items))) => {
                for object in items {
                    let summary = dynamic_resource_summary(cluster_context, &kind, &object);
                    if has_explicit_helm_label(&summary, release) && !summary.name.is_empty() {
                        rows_by_key
                            .entry(key_from_summary(&summary))
                            .or_insert_with(|| label_only_row(summary));
                    }
                }
            }
            Either::Right((_, Err(warning))) => warnings.push(warning),
        }
    }

    assemble_reconciliation_rows(rows_by_key, prepared.unavailable_rows, warnings)
}

fn prepare_manifest(
    manifest_refs: &[HelmManifestResourceSummary],
    namespace: &str,
    discovered_by_ref: &BTreeMap<(String, String), DiscoveredResourceKind>,
) -> PreparedManifest {
    let mut prepared = PreparedManifest {
        reads: Vec::with_capacity(manifest_refs.len()),
        unavailable_rows: Vec::new(),
        scan_kinds: BTreeSet::new(),
    };

    for (manifest_index, manifest_ref) in manifest_refs.iter().enumerate() {
        match prepare_manifest_ref(manifest_ref, namespace, discovered_by_ref) {
            Ok(identity) => {
                prepared
                    .scan_kinds
                    .insert((identity.key.api_version.clone(), identity.key.kind.clone()));
                prepared.reads.push((identity, manifest_index));
            }
            Err(row) => prepared.unavailable_rows.push(row),
        }
    }
    prepared
}

fn assemble_reconciliation_rows(
    rows_by_key: BTreeMap<ResourceKey, HelmReconciliationResource>,
    mut unavailable_rows: Vec<HelmReconciliationResource>,
    mut warnings: Vec<String>,
) -> (
    Vec<HelmReconciliationResource>,
    Vec<String>,
    HelmReconciliationTotals,
) {
    let mut rows: Vec<HelmReconciliationResource> = rows_by_key.into_values().collect();
    rows.append(&mut unavailable_rows);
    rows.sort_by_key(row_sort_key);
    warnings.sort();
    warnings.dedup();
    let totals = reconciliation_totals(&rows);
    (rows, warnings, totals)
}

#[cfg(feature = "bench-support")]
pub struct BenchmarkReconciliationPreparation {
    pub discovered_index_size: usize,
    pub valid_identity_count: usize,
    pub unavailable_rows: Vec<HelmReconciliationResource>,
    pub scan_kinds: Vec<DiscoveredResourceKind>,
}

#[cfg(feature = "bench-support")]
pub fn benchmark_reconciliation_prepare(
    manifest_refs: &[HelmManifestResourceSummary],
    discovered: &[DiscoveredResourceKind],
) -> BenchmarkReconciliationPreparation {
    let discovered_by_ref = discovered_ref_index(discovered);
    let prepared = prepare_manifest(manifest_refs, "payments", &discovered_by_ref);
    let scan_kinds = conservative_label_scan_kinds(discovered, &prepared.scan_kinds);
    BenchmarkReconciliationPreparation {
        discovered_index_size: discovered_by_ref.len(),
        valid_identity_count: std::hint::black_box(&prepared.reads).len(),
        unavailable_rows: prepared.unavailable_rows,
        scan_kinds,
    }
}

#[cfg(feature = "bench-support")]
pub fn benchmark_reconciliation_identity_namespace(
    manifest_ref: &HelmManifestResourceSummary,
    release_namespace: &str,
    discovered: &[DiscoveredResourceKind],
) -> Option<String> {
    let discovered_by_ref = discovered_ref_index(discovered);
    prepare_manifest_ref(manifest_ref, release_namespace, &discovered_by_ref)
        .ok()
        .and_then(|identity| identity.key.namespace)
}

// Keep the projected row inline; boxing would add an allocation to each invalid ref.
#[allow(clippy::result_large_err)]
fn prepare_manifest_ref(
    manifest_ref: &HelmManifestResourceSummary,
    release_namespace: &str,
    discovered_by_ref: &BTreeMap<(String, String), DiscoveredResourceKind>,
) -> Result<ResourceIdentity, HelmReconciliationResource> {
    resource_identity_from_manifest(manifest_ref, release_namespace, discovered_by_ref)
        .map_err(|message| unavailable_row(manifest_ref, true, message))
}

#[cfg(feature = "bench-support")]
pub fn benchmark_reconciliation_assemble(
    summary: HelmReleaseSummary,
    rows: Vec<(
        String,
        String,
        Option<String>,
        String,
        HelmReconciliationResource,
    )>,
    unavailable_rows: Vec<HelmReconciliationResource>,
    warnings: Vec<String>,
) -> HelmReleaseReconciliation {
    let mut rows_by_key = BTreeMap::new();
    for (api_version, kind, namespace, name, row) in rows {
        rows_by_key.insert(
            ResourceKey {
                api_version,
                kind,
                namespace,
                name,
            },
            row,
        );
    }
    let (resources, warnings, totals) =
        assemble_reconciliation_rows(rows_by_key, unavailable_rows, warnings);
    HelmReleaseReconciliation {
        summary,
        totals,
        resources,
        warnings,
    }
}

fn discovered_ref_index(
    discovered: &[DiscoveredResourceKind],
) -> BTreeMap<(String, String), DiscoveredResourceKind> {
    discovered
        .iter()
        .map(|kind| ((kind.api_version.clone(), kind.kind.clone()), kind.clone()))
        .collect()
}

fn resource_identity_from_manifest(
    manifest_ref: &HelmManifestResourceSummary,
    release_namespace: &str,
    discovered_by_ref: &BTreeMap<(String, String), DiscoveredResourceKind>,
) -> Result<ResourceIdentity, String> {
    let api_version = required_ref_field(manifest_ref.api_version.as_deref(), "apiVersion")?;
    let kind = required_ref_field(manifest_ref.kind.as_deref(), "kind")?;
    let name = required_ref_field(manifest_ref.name.as_deref(), "name")?;
    let resource_kind = discovered_by_ref
        .get(&(api_version.clone(), kind.clone()))
        .cloned()
        .ok_or_else(|| format!("{api_version} {kind} is not available through discovery"))?;
    let namespace = if resource_kind.namespaced {
        Some(
            manifest_ref
                .namespace
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| release_namespace.to_string()),
        )
    } else {
        None
    };
    crate::commands::helpers::validate_namespace(namespace.as_deref())
        .map_err(|error| error.message)?;
    crate::commands::helpers::validate_path_segment(&name, "manifest resource name")
        .map_err(|error| error.message)?;
    Ok(ResourceIdentity {
        key: ResourceKey {
            api_version,
            kind,
            namespace,
            name,
        },
        resource_kind,
    })
}

fn required_ref_field(value: Option<&str>, label: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("manifest resource is missing {label}"))
}

async fn live_row_for_manifest(
    client: Client,
    cluster_context: &str,
    release: &HelmReleaseSummary,
    manifest_ref: &HelmManifestResourceSummary,
    identity: &ResourceIdentity,
) -> HelmReconciliationResource {
    let api_resource = match api_resource_from_discovered(&identity.resource_kind) {
        Ok(api_resource) => api_resource,
        Err(err) => {
            return unavailable_row(manifest_ref, true, err.message);
        }
    };
    let api: Api<DynamicObject> = if identity.resource_kind.namespaced {
        Api::namespaced_with(
            client,
            identity
                .key
                .namespace
                .as_deref()
                .expect("namespaced identity has namespace"),
            &api_resource,
        )
    } else {
        Api::all_with(client, &api_resource)
    };

    match api.get(&identity.key.name).await {
        Ok(object) => {
            let summary =
                dynamic_resource_summary(cluster_context, &identity.resource_kind, &object);
            let explicit = has_explicit_helm_label(&summary, release);
            let status = if explicit {
                HelmReconciliationStatus::Tracked
            } else {
                HelmReconciliationStatus::UnlabeledLive
            };
            HelmReconciliationResource {
                api_version: Some(identity.key.api_version.clone()),
                kind: Some(identity.key.kind.clone()),
                namespace: identity.key.namespace.clone(),
                name: Some(identity.key.name.clone()),
                status,
                status_message: if explicit {
                    "Manifest resource exists and carries the explicit Helm release label."
                        .to_string()
                } else {
                    "Manifest resource exists, but the explicit Helm release label is missing or different."
                        .to_string()
                },
                in_manifest: true,
                explicit_helm_label: explicit,
                live_resource: Some(summary),
            }
        }
        Err(err) if is_not_found(&err) => HelmReconciliationResource {
            api_version: Some(identity.key.api_version.clone()),
            kind: Some(identity.key.kind.clone()),
            namespace: identity.key.namespace.clone(),
            name: Some(identity.key.name.clone()),
            status: HelmReconciliationStatus::Missing,
            status_message: "Manifest resource was not found in the live cluster.".to_string(),
            in_manifest: true,
            explicit_helm_label: false,
            live_resource: None,
        },
        Err(err) => unavailable_row(manifest_ref, true, err.to_string()),
    }
}

fn has_explicit_helm_label(summary: &ResourceSummary, release: &HelmReleaseSummary) -> bool {
    summary.helm_release.as_deref() == Some(release.name.as_str())
}

fn key_from_summary(summary: &ResourceSummary) -> ResourceKey {
    ResourceKey {
        api_version: summary.api_version.clone().unwrap_or_default(),
        kind: summary.kind.clone(),
        namespace: summary.namespace.clone(),
        name: summary.name.clone(),
    }
}

fn label_only_row(summary: ResourceSummary) -> HelmReconciliationResource {
    HelmReconciliationResource {
        api_version: summary.api_version.clone(),
        kind: Some(summary.kind.clone()),
        namespace: summary.namespace.clone(),
        name: Some(summary.name.clone()),
        status: HelmReconciliationStatus::LabelOnly,
        status_message: "Live resource carries the explicit Helm release label but is absent from the decoded manifest."
            .to_string(),
        in_manifest: false,
        explicit_helm_label: true,
        live_resource: Some(summary),
    }
}

fn unavailable_row(
    manifest_ref: &HelmManifestResourceSummary,
    in_manifest: bool,
    message: impl Into<String>,
) -> HelmReconciliationResource {
    HelmReconciliationResource {
        api_version: manifest_ref.api_version.clone(),
        kind: manifest_ref.kind.clone(),
        namespace: manifest_ref.namespace.clone(),
        name: manifest_ref.name.clone(),
        status: HelmReconciliationStatus::Unavailable,
        status_message: message.into(),
        in_manifest,
        explicit_helm_label: false,
        live_resource: None,
    }
}

fn is_not_found(err: &KubeError) -> bool {
    matches!(err, KubeError::Api(api_error) if api_error.code == 404)
}

fn reconciliation_totals(rows: &[HelmReconciliationResource]) -> HelmReconciliationTotals {
    let mut totals = HelmReconciliationTotals::default();
    for row in rows {
        match row.status {
            HelmReconciliationStatus::Tracked => totals.tracked += 1,
            HelmReconciliationStatus::UnlabeledLive => totals.unlabeled_live += 1,
            HelmReconciliationStatus::Missing => totals.missing += 1,
            HelmReconciliationStatus::LabelOnly => totals.label_only += 1,
            HelmReconciliationStatus::Unavailable => totals.unavailable += 1,
        }
    }
    totals
}

fn row_sort_key(row: &HelmReconciliationResource) -> (String, String, String, String) {
    (
        row.namespace.clone().unwrap_or_default(),
        row.kind.clone().unwrap_or_default(),
        row.name.clone().unwrap_or_default(),
        row.api_version.clone().unwrap_or_default(),
    )
}

#[cfg(test)]
#[path = "reconciliation_tests.rs"]
mod reconciliation_tests;
