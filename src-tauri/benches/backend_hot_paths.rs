use codspeed_divan_compat::black_box;
use kubecove_lib::{
    commands::bench_support::{
        build_bench_topology, build_custom_overlay_bench_topology_from_inputs,
        custom_overlay_bench_inputs, helm_manifest_resources, helm_manifest_summary,
        present_custom_resource_scope_key, sample_custom_resource_kinds, sample_topology_inputs,
        serialize_backend_yaml, sort_custom_resource_catalog,
    },
    models::YamlEncoding,
};
use serde_json::json;
use std::sync::LazyLock;

static HELM_MANIFEST: LazyLock<String> = LazyLock::new(|| build_manifest(250));
static HELM_RESOURCES: LazyLock<Vec<kubecove_lib::models::HelmManifestResourceSummary>> =
    LazyLock::new(|| helm_manifest_resources(Some(HELM_MANIFEST.as_str())));
static YAML_VALUE: LazyLock<serde_json::Value> = LazyLock::new(build_resource_document);
static TOPOLOGY_INPUTS: LazyLock<Vec<kubecove_lib::commands::bench_support::BenchTopologyInput>> =
    LazyLock::new(|| sample_topology_inputs(500));
static CUSTOM_RESOURCE_KINDS: LazyLock<Vec<kubecove_lib::models::DiscoveredResourceKind>> =
    LazyLock::new(|| sample_custom_resource_kinds(1_000));
static WORKSPACE_NAMESPACES: LazyLock<Vec<String>> = LazyLock::new(|| {
    (0..250)
        .rev()
        .map(|index| format!("namespace-{index}"))
        .collect()
});
static CUSTOM_OVERLAY_INPUTS: LazyLock<
    kubecove_lib::commands::bench_support::BenchCustomOverlayInputs,
> = LazyLock::new(|| custom_overlay_bench_inputs(500));

mod helm_manifest_parse_250_resources {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        black_box(helm_manifest_resources(black_box(Some(
            HELM_MANIFEST.as_str(),
        ))))
        .len()
    }
}

mod helm_manifest_summary_250_resources {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        black_box(helm_manifest_summary(black_box(&HELM_RESOURCES))).resource_count
    }
}

mod serialize_large_resource_yaml {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
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
    fn run() -> usize {
        black_box(serialize_backend_yaml(
            black_box(&YAML_VALUE),
            YamlEncoding::Kyaml,
        ))
        .len()
    }
}

mod build_ownership_topology_500_apps {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        let topology = black_box(build_bench_topology(black_box(&TOPOLOGY_INPUTS)));
        topology.nodes.len() + topology.edges.len()
    }
}

mod sort_custom_resource_catalog_1000_kinds {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        black_box(sort_custom_resource_catalog(black_box(
            &CUSTOM_RESOURCE_KINDS,
        )))
        .len()
    }
}

mod present_custom_resource_scope_key_250_namespaces {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        black_box(present_custom_resource_scope_key(black_box(
            &WORKSPACE_NAMESPACES,
        )))
        .len()
    }
}

mod build_ownership_topology_500_apps_with_crd_overlay {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn run() -> usize {
        let topology = black_box(build_custom_overlay_bench_topology_from_inputs(black_box(
            CUSTOM_OVERLAY_INPUTS.clone(),
        )));
        topology.nodes.len() + topology.edges.len()
    }
}

fn main() {
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
