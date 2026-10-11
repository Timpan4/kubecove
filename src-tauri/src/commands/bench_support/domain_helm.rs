use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::{write::GzEncoder, Compression};
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::api::ObjectMeta;
use std::collections::BTreeMap;

use crate::{
    commands::{
        bench_support::domain_catalog::encode_complete_response,
        bench_support::{build_bench_topology, sample_topology_inputs},
        helm::{
            benchmark_invalid_storage_record_is_retained, benchmark_reconciliation_assemble,
            benchmark_reconciliation_identity_namespace, benchmark_reconciliation_prepare,
            benchmark_storage_record_error, benchmark_storage_record_projection,
            benchmark_storage_records as build_storage_records_to_latest,
            MAX_DECODED_RELEASE_BYTES, MAX_STORED_RELEASE_BYTES,
        },
    },
    models::{
        DiscoveredResourceKind, HelmManifestResourceSummary, HelmManifestSummary,
        HelmReconciliationResource, HelmReconciliationStatus, HelmReleaseList,
        HelmReleaseReconciliation, HelmReleaseSummary, HelmValuesSummary,
    },
};

pub const STORAGE_RECORD_MAX_BYTES: usize = MAX_STORED_RELEASE_BYTES;
pub const DECODED_RELEASE_MAX_BYTES: u64 = MAX_DECODED_RELEASE_BYTES;

pub type HelmReconciliationAssemblyInputs = (
    HelmReleaseSummary,
    Vec<(
        String,
        String,
        Option<String>,
        String,
        HelmReconciliationResource,
    )>,
    Vec<HelmReconciliationResource>,
    Vec<String>,
);

pub fn storage_records() -> (Vec<Secret>, Vec<ConfigMap>) {
    let mut secrets = Vec::with_capacity(125);
    let mut configmaps = Vec::with_capacity(125);
    for index in 0..250 {
        let name = format!("release-{:02}", index % 50);
        let revision = if index == 200 { 4 } else { index / 50 + 1 };
        let namespace = if index == 0 { "other" } else { "payments" };
        let payload = release_payload_for(&name, (index != 249).then_some(revision), namespace);
        let mut metadata = storage_metadata(&name, revision);
        if index == 249 {
            metadata
                .labels
                .as_mut()
                .expect("storage labels fixture")
                .remove("version");
        }
        if index % 2 == 0 {
            let data = if index == 248 {
                b"not-base64".to_vec()
            } else {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
                std::io::Write::write_all(&mut encoder, payload.as_bytes())
                    .expect("fixture gzip input");
                STANDARD
                    .encode(encoder.finish().expect("fixture gzip output"))
                    .into_bytes()
            };
            secrets.push(Secret {
                metadata,
                data: Some(BTreeMap::from([(
                    "release".to_string(),
                    k8s_openapi::ByteString(data),
                )])),
                type_: Some("helm.sh/release.v1".to_string()),
                ..Default::default()
            });
        } else {
            configmaps.push(ConfigMap {
                metadata,
                data: Some(BTreeMap::from([(
                    "release".to_string(),
                    STANDARD.encode(payload),
                )])),
                ..Default::default()
            });
        }
    }
    secrets.reverse();
    configmaps.reverse();
    (secrets, configmaps)
}

pub fn storage_bound_records() -> Vec<Secret> {
    let oversized_stored = Secret {
        metadata: storage_metadata("oversized-stored", 1),
        data: Some(BTreeMap::from([(
            "release".to_string(),
            k8s_openapi::ByteString(vec![b'x'; STORAGE_RECORD_MAX_BYTES + 1]),
        )])),
        type_: Some("helm.sh/release.v1".to_string()),
        ..Default::default()
    };
    let expanded = format!(
        "{{\"name\":\"oversized-decoded\",\"namespace\":\"payments\",\"version\":1,\"manifest\":\"{}\"}}",
        "x".repeat(DECODED_RELEASE_MAX_BYTES as usize + 1)
    );
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::Write::write_all(&mut encoder, expanded.as_bytes()).expect("fixture gzip input");
    let compressed = STANDARD.encode(encoder.finish().expect("fixture gzip output"));
    assert!(compressed.len() <= STORAGE_RECORD_MAX_BYTES);
    let oversized_decoded = Secret {
        metadata: storage_metadata("oversized-decoded", 1),
        data: Some(BTreeMap::from([(
            "release".to_string(),
            k8s_openapi::ByteString(compressed.into_bytes()),
        )])),
        type_: Some("helm.sh/release.v1".to_string()),
        ..Default::default()
    };
    vec![oversized_stored, oversized_decoded]
}

pub fn storage_identity_precedence_record() -> Secret {
    let payload = release_payload("decoded-name", 1);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::Write::write_all(&mut encoder, payload.as_bytes()).expect("fixture gzip input");
    let data = STANDARD
        .encode(encoder.finish().expect("fixture gzip output"))
        .into_bytes();
    let mut metadata = storage_metadata("label-name", 1);
    metadata.name = Some("sh.helm.release.v1.storage-name.v1".to_string());
    Secret {
        metadata,
        data: Some(BTreeMap::from([(
            "release".to_string(),
            k8s_openapi::ByteString(data),
        )])),
        type_: Some("helm.sh/release.v1".to_string()),
        ..Default::default()
    }
}

pub fn manifest_fixture() -> String {
    let embedded_config = "chart-value-".repeat(80);
    let mut documents = Vec::with_capacity(250);
    for index in 0..250 {
        let (api_version, kind) = match index % 3 {
            0 => ("apps/v1", "Deployment"),
            1 => ("rbac.authorization.k8s.io/v1", "ClusterRole"),
            _ => ("operator.example.io/v1", "ManagedThing"),
        };
        let namespace = if kind == "ClusterRole" {
            String::new()
        } else {
            format!("tenant-{}", index % 25)
        };
        documents.push(format!(
            "apiVersion: {api_version}\nkind: {kind}\nmetadata:\n  name: resource-{index:03}\n{}spec:\n  chartConfig: \"{embedded_config}\"\n  replicas: 3",
            if namespace.is_empty() {
                String::new()
            } else {
                format!("  namespace: {namespace}\n")
            }
        ));
    }
    documents.push(String::new());
    documents.push("{}".to_string());
    documents.push("[not-a-resource]".to_string());
    documents.push("apiVersion: apps/v1\nkind: [".to_string());
    documents.join("\n---\n")
}

pub fn reconciliation_inputs() -> (
    Vec<HelmManifestResourceSummary>,
    Vec<DiscoveredResourceKind>,
) {
    let discovered = super::super::sample_custom_resource_kinds(1_000);
    let mut refs = discovered
        .iter()
        .take(250)
        .enumerate()
        .map(|(index, kind)| HelmManifestResourceSummary {
            api_version: Some(kind.api_version.clone()),
            kind: Some(kind.kind.clone()),
            name: Some(format!("resource-{index:03}")),
            namespace: (kind.namespaced && index % 7 != 0)
                .then(|| format!("tenant-{}", index % 25)),
        })
        .collect::<Vec<_>>();
    refs.extend([
        HelmManifestResourceSummary {
            api_version: Some("operator.example.io/v1".to_string()),
            kind: Some("MissingDiscoveryKind".to_string()),
            name: Some("unavailable".to_string()),
            namespace: Some("tenant-0".to_string()),
        },
        HelmManifestResourceSummary {
            api_version: Some("operator.example.io/v1".to_string()),
            kind: Some("MissingName".to_string()),
            name: None,
            namespace: None,
        },
    ]);
    (refs, discovered)
}

pub fn reconciliation_assembly_inputs() -> HelmReconciliationAssemblyInputs {
    let topology = build_bench_topology(&sample_topology_inputs(1));
    let live_resource = topology.nodes[0].summary.clone();
    let summary = HelmReleaseSummary {
        cluster: "benchmark".to_string(),
        name: "checkout".to_string(),
        namespace: "payments".to_string(),
        age: "1d".to_string(),
        updated_at: Some("2026-10-10T00:00:00Z".to_string()),
        created_at: Some("2026-10-09T00:00:00Z".to_string()),
        chart: Some("checkout-1.0.0".to_string()),
        app_version: Some("1.0.0".to_string()),
        revision: Some(8),
        status: Some("deployed".to_string()),
        storage_kind: "Secret".to_string(),
        storage_name: "sh.helm.release.v1.checkout.v8".to_string(),
    };
    let mut rows = Vec::with_capacity(251);
    for index in 0..250 {
        let (status, in_manifest, explicit, live) = match index % 5 {
            0 => (
                HelmReconciliationStatus::Tracked,
                true,
                true,
                Some(live_resource.clone()),
            ),
            1 => (
                HelmReconciliationStatus::UnlabeledLive,
                true,
                false,
                Some(live_resource.clone()),
            ),
            2 => (HelmReconciliationStatus::Missing, true, false, None),
            3 => (
                HelmReconciliationStatus::LabelOnly,
                false,
                true,
                Some(live_resource.clone()),
            ),
            _ => (HelmReconciliationStatus::Unavailable, true, false, None),
        };
        let api_version = "apps/v1".to_string();
        let kind = "Deployment".to_string();
        let namespace = Some(format!("tenant-{}", index % 25));
        let name = format!("resource-{index:03}");
        rows.push((
            api_version.clone(),
            kind.clone(),
            namespace.clone(),
            name.clone(),
            HelmReconciliationResource {
                api_version: Some(api_version),
                kind: Some(kind),
                namespace,
                name: Some(name),
                status,
                status_message: "deterministic fixture status".to_string(),
                in_manifest,
                explicit_helm_label: explicit,
                live_resource: live,
            },
        ));
    }
    let duplicate_row = rows[0].clone();
    rows.push(duplicate_row);
    let unavailable_rows = (0..3)
        .map(|index| HelmReconciliationResource {
            api_version: Some("operator.example.io/v1".to_string()),
            kind: Some("MissingDiscoveryKind".to_string()),
            namespace: Some("payments".to_string()),
            name: Some(format!("unavailable-{index}")),
            status: HelmReconciliationStatus::Unavailable,
            status_message: "resource kind was not discovered".to_string(),
            in_manifest: true,
            explicit_helm_label: false,
            live_resource: None,
        })
        .collect();
    (
        summary,
        rows,
        unavailable_rows,
        vec!["scan incomplete".to_string(), "scan incomplete".to_string()],
    )
}

pub fn benchmark_storage_records(
    secrets: Vec<Secret>,
    configmaps: Vec<ConfigMap>,
) -> Result<Vec<HelmReleaseSummary>, crate::models::AppError> {
    build_storage_records_to_latest(secrets, configmaps)
}

pub fn invalid_storage_error_is_retained(secret: Secret) -> bool {
    benchmark_invalid_storage_record_is_retained(secret)
}

pub fn storage_record_error(secret: Secret) -> Option<(crate::models::AppErrorKind, String)> {
    benchmark_storage_record_error(secret)
}

pub fn storage_record_projection(
    secret: Secret,
) -> Result<
    (
        HelmReleaseSummary,
        HelmValuesSummary,
        HelmManifestSummary,
        Option<serde_json::Value>,
        bool,
    ),
    crate::models::AppError,
> {
    benchmark_storage_record_projection(secret)
}

pub fn parse_manifest(manifest: &str) -> Vec<HelmManifestResourceSummary> {
    super::super::helm_manifest_resources(Some(manifest))
}

pub fn manifest_summary(resources: &[HelmManifestResourceSummary]) -> HelmManifestSummary {
    super::super::helm_manifest_summary(resources)
}

pub struct HelmReconciliationPreparation {
    pub discovered_index_size: usize,
    pub valid_identity_count: usize,
    pub unavailable_rows: Vec<HelmReconciliationResource>,
    pub scan_kinds: Vec<DiscoveredResourceKind>,
}

pub fn reconciliation_prepare(
    refs: &[HelmManifestResourceSummary],
    discovered: &[DiscoveredResourceKind],
) -> HelmReconciliationPreparation {
    let prepared = benchmark_reconciliation_prepare(refs, discovered);
    HelmReconciliationPreparation {
        discovered_index_size: prepared.discovered_index_size,
        valid_identity_count: prepared.valid_identity_count,
        unavailable_rows: prepared.unavailable_rows,
        scan_kinds: prepared.scan_kinds,
    }
}

pub fn reconciliation_identity_namespace(
    manifest_ref: &HelmManifestResourceSummary,
    release_namespace: &str,
    discovered: &[DiscoveredResourceKind],
) -> Option<String> {
    benchmark_reconciliation_identity_namespace(manifest_ref, release_namespace, discovered)
}

pub fn reconciliation_assemble(
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
    benchmark_reconciliation_assemble(summary, rows, unavailable_rows, warnings)
}

pub fn encode_release_list(releases: Vec<HelmReleaseSummary>) -> String {
    encode_complete_response(&HelmReleaseList {
        releases,
        warnings: Vec::new(),
    })
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    encode_complete_response(value)
}

fn storage_metadata(name: &str, revision: usize) -> ObjectMeta {
    ObjectMeta {
        name: Some(format!("sh.helm.release.v1.{name}.v{revision}")),
        namespace: Some("payments".to_string()),
        labels: Some(BTreeMap::from([
            ("owner".to_string(), "helm".to_string()),
            ("name".to_string(), name.to_string()),
            ("version".to_string(), revision.to_string()),
            ("status".to_string(), "deployed".to_string()),
        ])),
        ..Default::default()
    }
}

fn release_payload(name: &str, revision: usize) -> String {
    release_payload_for(name, Some(revision), "payments")
}

fn release_payload_for(name: &str, revision: Option<usize>, namespace: &str) -> String {
    let mut payload = serde_json::json!({
        "name": name,
        "namespace": namespace,
        "version": revision,
        "info": {
            "status": "deployed",
            "last_deployed": "2026-10-10T00:00:00Z"
        },
        "chart": {
            "metadata": {
                "name": "checkout",
                "version": "1.0.0",
                "appVersion": "1.0.0"
            }
        },
        "config": {
            "password": "must-not-be-in-safe-release-metadata",
            "replicaCount": 5,
            "image": { "tag": "1.0.0" }
        },
        "manifest": "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: checkout\n  namespace: payments\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: checkout\n  namespace: payments"
    });
    if revision.is_none() {
        payload
            .as_object_mut()
            .expect("release payload object")
            .remove("version");
    }
    payload.to_string()
}
