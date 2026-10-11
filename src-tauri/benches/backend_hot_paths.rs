use codspeed_divan_compat::black_box;
use kubecove_lib::{
    commands::bench_support::{
        build_bench_topology, build_custom_overlay_bench_topology_from_inputs,
        custom_overlay_bench_inputs, helm_manifest_resources, sample_custom_resource_kinds,
        sample_topology_inputs, serialize_backend_yaml, sort_custom_resource_catalog,
    },
    models::YamlEncoding,
};
use serde_json::json;
use std::sync::LazyLock;

mod domain_catalog;
#[path = "backend_hot_paths/resource_catalog.rs"]
mod resource_catalog;

static HELM_MANIFEST: LazyLock<String> = LazyLock::new(|| build_manifest(250));
static YAML_VALUE: LazyLock<serde_json::Value> = LazyLock::new(build_resource_document);
static TOPOLOGY_INPUTS: LazyLock<Vec<kubecove_lib::commands::bench_support::BenchTopologyInput>> =
    LazyLock::new(|| sample_topology_inputs(500));
static CUSTOM_RESOURCE_KINDS: LazyLock<Vec<kubecove_lib::models::DiscoveredResourceKind>> =
    LazyLock::new(|| sample_custom_resource_kinds(1_000));
static CUSTOM_OVERLAY_INPUTS: LazyLock<
    kubecove_lib::commands::bench_support::BenchCustomOverlayInputs,
> = LazyLock::new(|| custom_overlay_bench_inputs(500));

// Model the affected cluster's 63 CRDs and roughly 20,413 schema nodes, without
// recording cluster data. Both decoders consume the same generated JSON.
static LARGE_CRD_LIST: LazyLock<String> = LazyLock::new(|| {
    let items: Vec<_> = (0..63)
        .map(|index| {
            let nodes = 20_413 / 63 + usize::from(index < 20_413 % 63);
            let properties: serde_json::Map<String, serde_json::Value> = (0..nodes)
                .map(|field| (format!("field{field}"), json!({
                    "type": "string", "description": "x".repeat(1_588_534 / 20_413)
                })))
                .collect();
            json!({
                "apiVersion": "apiextensions.k8s.io/v1",
                "kind": "CustomResourceDefinition",
                "metadata": {"name": format!("widgets{index}.example.com")},
                "spec": {
                    "group": "example.com",
                    "names": {"kind": format!("Widget{index}"), "plural": format!("widgets{index}")},
                    "scope": "Namespaced",
                    "versions": [{"name": "v1", "served": true, "storage": true,
                        "schema": {"openAPIV3Schema": {"type": "object", "properties": properties}}
                    }]
                }
            })
        })
        .collect();
    json!({"items": items}).to_string()
});

mod decode_crd_catalog_63_large_schemas {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        black_box(kubecove_lib::commands::bench_support::decode_crd_catalog(
            black_box(LARGE_CRD_LIST.as_str()),
        ))
        .len()
    }
}

mod decode_full_crds_63_large_schemas_reference {
    use super::*;
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        let list: kube::api::ObjectList<CustomResourceDefinition> =
            serde_json::from_str(black_box(LARGE_CRD_LIST.as_str())).expect("benchmark CRD list");
        black_box(list).items.len()
    }
}

// R27: preserve the direct production manifest-parser signal.
mod helm_manifest_parse_250_resources {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_manifest_parse_250_resources() -> usize {
        black_box(helm_manifest_resources(black_box(Some(
            HELM_MANIFEST.as_str(),
        ))))
        .len()
    }
}

// R20: prepared-document emission variants complement full document preparation.
mod serialize_large_resource_yaml {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn serialize_large_resource_yaml() -> usize {
        black_box(serialize_backend_yaml(
            black_box(&YAML_VALUE),
            YamlEncoding::Yaml,
        ))
        .len()
    }
}

mod serialize_large_resource_kyaml {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn serialize_large_resource_kyaml() -> usize {
        black_box(serialize_backend_yaml(
            black_box(&YAML_VALUE),
            YamlEncoding::Kyaml,
        ))
        .len()
    }
}

// R08: includes the caller's input conversion and cloning.
mod build_ownership_topology_500_apps {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn build_ownership_topology_500_apps() -> usize {
        let topology = black_box(build_bench_topology(black_box(&TOPOLOGY_INPUTS)));
        topology.nodes.len() + topology.edges.len()
    }
}

// R17: catalog ordering and deduplication after projection.
mod sort_custom_resource_catalog_1000_kinds {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn sort_custom_resource_catalog_1000_kinds() -> usize {
        black_box(sort_custom_resource_catalog(black_box(
            &CUSTOM_RESOURCE_KINDS,
        )))
        .len()
    }
}

mod build_ownership_topology_500_apps_with_crd_overlay {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn build_ownership_topology_500_apps_with_crd_overlay() -> usize {
        let topology = black_box(build_custom_overlay_bench_topology_from_inputs(black_box(
            CUSTOM_OVERLAY_INPUTS.clone(),
        )));
        topology.nodes.len() + topology.edges.len()
    }
}

fn main() {
    // Input generation and LazyLock initialization are outside every measurement.
    LazyLock::force(&HELM_MANIFEST);
    LazyLock::force(&YAML_VALUE);
    LazyLock::force(&TOPOLOGY_INPUTS);
    LazyLock::force(&CUSTOM_RESOURCE_KINDS);
    LazyLock::force(&CUSTOM_OVERLAY_INPUTS);
    LazyLock::force(&LARGE_CRD_LIST);
    codspeed_divan_compat::main();
}

fn build_manifest(resources: usize) -> String {
    (0..resources)
        .map(|index| {
            let namespace = format!("namespace-{}", index % 50);
            let app = format!("checkout-{index}");
            [
                "apiVersion: apps/v1".to_string(),
                "kind: Deployment".to_string(),
                "metadata:".to_string(),
                format!("  name: {app}"),
                format!("  namespace: {namespace}"),
                "  labels:".to_string(),
                format!("    app.kubernetes.io/name: {app}"),
                "spec:".to_string(),
                "  replicas: 3".to_string(),
                "---".to_string(),
            ]
            .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_resource_document() -> serde_json::Value {
    let containers = (0..40)
        .map(|index| {
            json!({
                "name": format!("worker-{index}"),
                "image": format!("ghcr.io/kubecove/worker-{index}:1.0.0"),
                "env": [
                    { "name": "WORKER_INDEX", "value": index.to_string() },
                    { "name": "RUST_LOG", "value": "info" }
                ],
                "resources": {
                    "requests": {
                        "cpu": format!("{}m", 100 + index),
                        "memory": "128Mi"
                    },
                    "limits": {
                        "memory": "256Mi"
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "checkout-api",
            "namespace": "checkout",
            "labels": {
                "app.kubernetes.io/name": "checkout",
                "app.kubernetes.io/part-of": "payments"
            },
            "managedFields": [
                { "manager": "kubectl", "operation": "Apply" }
            ]
        },
        "spec": {
            "replicas": 5,
            "template": {
                "metadata": {
                    "labels": {
                        "app.kubernetes.io/name": "checkout"
                    }
                },
                "spec": {
                    "containers": containers
                }
            }
        },
        "status": {
            "readyReplicas": 5,
            "updatedReplicas": 5
        }
    })
}
