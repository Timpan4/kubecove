use super::{discovery, helm::manifest, helpers, live_store, resources::topology};
use crate::models::{
    DiscoveredResourceKind, HelmManifestResourceSummary, HelmManifestSummary,
    OwnerReferenceSummary, ResourceHealth, ResourceSummary, ResourceTopology, YamlEncoding,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub struct BenchTopologyInput {
    pub kind: &'static str,
    pub name: String,
    pub namespace: String,
    pub uid: String,
    pub owner: Option<BenchOwner>,
}

#[derive(Clone)]
pub struct BenchCustomOverlayInputs(Vec<topology::TopologyInputResource>);

pub struct BenchOwner {
    pub api_version: &'static str,
    pub kind: &'static str,
    pub name: String,
    pub uid: String,
}

pub fn helm_manifest_resources(manifest: Option<&str>) -> Vec<HelmManifestResourceSummary> {
    manifest::manifest_resources(manifest)
}

pub fn helm_manifest_summary(resources: &[HelmManifestResourceSummary]) -> HelmManifestSummary {
    manifest::manifest_summary_from_resources(resources)
}

pub fn serialize_backend_yaml(value: &Value, encoding: YamlEncoding) -> String {
    helpers::serialize_json_value_document(value, encoding).expect("benchmark value serializes")
}

pub fn sample_topology_inputs(apps: usize) -> Vec<BenchTopologyInput> {
    let mut inputs = Vec::with_capacity(apps * 4);
    for index in 0..apps {
        let namespace = format!("namespace-{}", index % 100);
        let deployment_uid = format!("deployment-{index}");
        let replica_set_uid = format!("replicaset-{index}");
        let app = format!("checkout-{index}");

        inputs.push(BenchTopologyInput {
            kind: "Deployment",
            name: app.clone(),
            namespace: namespace.clone(),
            uid: deployment_uid.clone(),
            owner: None,
        });
        inputs.push(BenchTopologyInput {
            kind: "ReplicaSet",
            name: format!("{app}-rs"),
            namespace: namespace.clone(),
            uid: replica_set_uid.clone(),
            owner: Some(BenchOwner {
                api_version: "apps/v1",
                kind: "Deployment",
                name: app.clone(),
                uid: deployment_uid,
            }),
        });
        inputs.push(BenchTopologyInput {
            kind: "Pod",
            name: format!("{app}-pod"),
            namespace: namespace.clone(),
            uid: format!("pod-{index}"),
            owner: Some(BenchOwner {
                api_version: "apps/v1",
                kind: "ReplicaSet",
                name: format!("{app}-rs"),
                uid: replica_set_uid,
            }),
        });
        inputs.push(BenchTopologyInput {
            kind: "Service",
            name: format!("{app}-service"),
            namespace,
            uid: format!("service-{index}"),
            owner: None,
        });
    }
    inputs
}

pub fn build_bench_topology(inputs: &[BenchTopologyInput]) -> ResourceTopology {
    let resources = inputs
        .iter()
        .map(|input| topology::TopologyInputResource {
            uid: input.uid.clone(),
            owner: input.owner.as_ref().map(|owner| OwnerReferenceSummary {
                api_version: owner.api_version.to_string(),
                kind: owner.kind.to_string(),
                name: owner.name.clone(),
                uid: owner.uid.clone(),
            }),
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: ResourceSummary {
                kind: input.kind.to_string(),
                cluster: "prod".to_string(),
                name: input.name.clone(),
                namespace: Some(input.namespace.clone()),
                age: "1m".to_string(),
                api_version: Some(api_version_for_kind(input.kind).to_string()),
                group: None,
                version: None,
                plural: None,
                namespaced: Some(true),
                dynamic: None,
                health: ResourceHealth::Healthy,
                created_at: None,
                status: Some("Running".to_string()),
                ready: Some("True".to_string()),
                restarts: Some(0),
                owner_ref: input.owner.as_ref().map(|owner| owner.name.clone()),
                argo_app: None,
                helm_release: None,
                git_ops_owner: None,
            },
        })
        .collect();
    topology::build_resource_topology(resources)
}

pub fn sample_custom_resource_kinds(count: usize) -> Vec<DiscoveredResourceKind> {
    let mut kinds = Vec::with_capacity(count + count / 10);
    for index in (0..count).rev() {
        let group = format!("operator-{}.example.com", index % 40);
        let kind = format!("ManagedThing{index}");
        let resource_kind = DiscoveredResourceKind {
            group: group.clone(),
            version: if index % 3 == 0 { "v1beta1" } else { "v1" }.to_string(),
            api_version: format!("{group}/v1"),
            kind: kind.clone(),
            plural: format!("managedthings{index}"),
            namespaced: index % 7 != 0,
        };
        kinds.push(resource_kind.clone());
        if index % 10 == 0 {
            kinds.push(resource_kind);
        }
    }
    kinds
}

pub fn sort_custom_resource_catalog(
    kinds: &[DiscoveredResourceKind],
) -> Vec<DiscoveredResourceKind> {
    let mut kinds = kinds.to_vec();
    discovery::sort_and_dedup_kinds(&mut kinds);
    kinds
}

pub fn present_custom_resource_scope_key(namespaces: &[String]) -> String {
    live_store::present_custom_resource_kinds_cache_key("default", "admin@solid-k8s", namespaces)
}

pub fn custom_overlay_bench_inputs(apps: usize) -> BenchCustomOverlayInputs {
    let mut resources = Vec::with_capacity(apps * 5);
    for index in 0..apps {
        let namespace = format!("namespace-{}", index % 100);
        let app = format!("postgres-{index}");
        let cluster_uid = format!("cluster-{index}");
        let deployment_uid = format!("deployment-{index}");
        let replica_set_uid = format!("replicaset-{index}");

        resources.push(topology::TopologyInputResource {
            uid: cluster_uid.clone(),
            owner: None,
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: resource_summary(
                "Cluster",
                "postgresql.cnpg.io/v1",
                "clusters",
                true,
                true,
                &app,
                &namespace,
            ),
        });
        resources.push(topology::TopologyInputResource {
            uid: deployment_uid.clone(),
            owner: Some(OwnerReferenceSummary {
                api_version: "postgresql.cnpg.io/v1".to_string(),
                kind: "Cluster".to_string(),
                name: app.clone(),
                uid: cluster_uid,
            }),
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: resource_summary(
                "Deployment",
                "apps/v1",
                "deployments",
                true,
                false,
                &app,
                &namespace,
            ),
        });
        resources.push(topology::TopologyInputResource {
            uid: replica_set_uid.clone(),
            owner: Some(OwnerReferenceSummary {
                api_version: "apps/v1".to_string(),
                kind: "Deployment".to_string(),
                name: app.clone(),
                uid: deployment_uid,
            }),
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: resource_summary(
                "ReplicaSet",
                "apps/v1",
                "replicasets",
                true,
                false,
                &format!("{app}-rs"),
                &namespace,
            ),
        });
        resources.push(topology::TopologyInputResource {
            uid: format!("pod-{index}"),
            owner: Some(OwnerReferenceSummary {
                api_version: "apps/v1".to_string(),
                kind: "ReplicaSet".to_string(),
                name: format!("{app}-rs"),
                uid: replica_set_uid,
            }),
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: resource_summary(
                "Pod",
                "v1",
                "pods",
                true,
                false,
                &format!("{app}-pod"),
                &namespace,
            ),
        });
        resources.push(topology::TopologyInputResource {
            uid: format!("service-{index}"),
            owner: None,
            labels: BTreeMap::new(),
            port_hints: Vec::new(),
            summary: resource_summary(
                "Service",
                "v1",
                "services",
                true,
                false,
                &format!("{app}-rw"),
                &namespace,
            ),
        });
    }
    BenchCustomOverlayInputs(resources)
}

pub fn build_custom_overlay_bench_topology(apps: usize) -> ResourceTopology {
    let resources = custom_overlay_bench_inputs(apps);
    build_custom_overlay_bench_topology_from_inputs(resources)
}

pub fn build_custom_overlay_bench_topology_from_inputs(
    resources: BenchCustomOverlayInputs,
) -> ResourceTopology {
    topology::build_resource_topology(resources.0)
}

fn resource_summary(
    kind: &str,
    api_version: &str,
    plural: &str,
    namespaced: bool,
    dynamic: bool,
    name: &str,
    namespace: &str,
) -> ResourceSummary {
    let (group, version) = api_version.split_once('/').map_or(
        (String::new(), api_version.to_string()),
        |(group, version)| (group.to_string(), version.to_string()),
    );
    ResourceSummary {
        kind: kind.to_string(),
        cluster: "prod".to_string(),
        name: name.to_string(),
        namespace: namespaced.then(|| namespace.to_string()),
        age: "1m".to_string(),
        api_version: Some(api_version.to_string()),
        group: Some(group),
        version: Some(version),
        plural: Some(plural.to_string()),
        namespaced: Some(namespaced),
        dynamic: Some(dynamic),
        health: ResourceHealth::Healthy,
        created_at: None,
        status: Some("Running".to_string()),
        ready: Some("True".to_string()),
        restarts: Some(0),
        owner_ref: None,
        argo_app: None,
        helm_release: None,
        git_ops_owner: None,
    }
}

fn api_version_for_kind(kind: &str) -> &'static str {
    match kind {
        "Deployment" | "ReplicaSet" => "apps/v1",
        _ => "v1",
    }
}
