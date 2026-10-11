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
