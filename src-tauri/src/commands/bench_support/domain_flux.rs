use kube::{api::ApiResource, core::DynamicObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    commands::{bench_support::sample_custom_resource_kinds, flux},
    models::{FluxResourceKind, FluxResourceSummary},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluxListProjection {
    pub installed_kinds: Vec<FluxResourceKind>,
    pub summaries: Vec<FluxResourceSummary>,
}

#[derive(Clone)]
pub struct FluxObjectBatch {
    pub kind: FluxResourceKind,
    pub objects: Vec<DynamicObject>,
}

pub fn discovery_resources() -> Vec<ApiResource> {
    let flux_kinds = flux::flux_kinds();
    let mut resources = sample_custom_resource_kinds(1_000)
        .into_iter()
        .take(1_000 - flux_kinds.len())
        .map(|kind| ApiResource {
            group: kind.group,
            version: kind.version,
            api_version: kind.api_version,
            kind: kind.kind,
            plural: kind.plural,
        })
        .collect::<Vec<_>>();
    for kind in &flux_kinds {
        let version = if kind.kind == "GitRepository" {
            "v1beta2"
        } else {
            kind.version.as_str()
        };
        resources.push(ApiResource {
            group: kind.group.clone(),
            version: version.to_string(),
            api_version: format!("{}/{version}", kind.group),
            kind: kind.kind.clone(),
            plural: if kind.kind == "GitRepository" {
                "gitrepositories-v1beta2".to_string()
            } else {
                kind.plural.clone()
            },
        });
    }
    resources.push(ApiResource {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1".to_string(),
        api_version: "source.toolkit.fluxcd.io/v1".to_string(),
        kind: "GitRepository".to_string(),
        plural: "gitrepositories".to_string(),
    });
    resources
}

pub fn flux_object_batches() -> Vec<FluxObjectBatch> {
    let kinds = flux::flux_kinds();
    ["GitRepository", "Kustomization", "HelmRelease"]
        .into_iter()
        .map(|kind_name| {
            let kind = kinds
                .iter()
                .find(|kind| kind.kind == kind_name)
                .expect("Flux fixture kind");
            let objects = (0..250)
                .map(|index| {
                    let entries = if kind_name == "GitRepository" && index == 0 {
                        (0..=flux::MAX_FLUX_INVENTORY_ENTRIES)
                            .map(|entry| match entry {
                                1 => json!({"id":"   ", "v":"v1"}),
                                2 => json!({"v":"v1"}),
                                3 => serde_json::Value::Null,
                                _ => {
                                    let id = format!(
                                        "payments_apps_Deployment_tenant-{}/deployment-{entry:04}",
                                        entry % 25
                                    );
                                    if entry % 2 == 0 {
                                        json!({"id":id,"version":"v1"})
                                    } else {
                                        json!({"id":id,"v":"v1"})
                                    }
                                }
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let name = format!("{}-{index:03}", kind_name.to_ascii_lowercase());
                    let ready_condition = if index == 1 {
                        json!({
                            "type":"Ready",
                            "reason":"SourceArtifactReady",
                            "message":format!("{}-{index:03} status", kind_name)
                        })
                    } else {
                        json!({
                            "type":"Ready",
                            "status":if index % 3 == 0 {"False"} else {"True"},
                            "reason":"Reconciliation",
                            "message":format!("{}-{index:03} status", kind_name)
                        })
                    };
                    let mut status = json!({
                        "conditions":[
                            {"type":"Progressing", "status":"True", "message":"not Ready"},
                            ready_condition,
                            {"type":"Ready", "status":"True", "message":"later Ready condition"}
                        ],
                        "lastAppliedRevision":format!("main@sha1:{index:040x}"),
                        "inventory":{"entries":entries}
                    });
                    if index == 1 {
                        let status = status.as_object_mut().expect("fixture status object");
                        status.remove("lastAppliedRevision");
                        status.insert(
                            "artifact".to_string(),
                            json!({"revision":"artifact-revision-001"}),
                        );
                    }
                    DynamicObject::new(
                        &name,
                        &kube::core::ApiResource {
                            group: kind.group.clone(),
                            version: kind.version.clone(),
                            api_version: kind.api_version.clone(),
                            kind: kind.kind.clone(),
                            plural: kind.plural.clone(),
                        },
                    )
                    .within("flux-system")
                    .data(json!({
                        "spec": {
                            "suspend": index % 2 == 0,
                            "interval":"5m",
                            "sourceRef":{"kind":"GitRepository", "name":"payments", "namespace":"flux-system"}
                        },
                        "status": status
                    }))
                })
                .collect();
            FluxObjectBatch {
                kind: kind.clone(),
                objects,
            }
        })
        .collect()
}

pub fn project_list(discovery: &[ApiResource], batches: &[FluxObjectBatch]) -> FluxListProjection {
    let installed_kinds = flux::benchmark_installed_flux_kinds_from_resources(discovery);
    let summaries = batches
        .iter()
        .flat_map(|batch| {
            let kind = installed_kinds
                .iter()
                .find(|kind| kind.kind == batch.kind.kind && kind.group == batch.kind.group)
                .expect("fixture Flux kind installed");
            batch
                .objects
                .iter()
                .map(|object| flux::benchmark_flux_summary("benchmark", kind, object))
                .collect::<Vec<_>>()
        })
        .collect();
    FluxListProjection {
        installed_kinds,
        summaries,
    }
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    super::encode_complete_response(value)
}
