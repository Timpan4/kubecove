use super::topology::{
    build_resource_topology, topology_node_id, topology_root_kinds, topology_standalone_kinds,
    TopologyInputResource,
};
use crate::models::{OwnerReferenceSummary, ResourceSummary, TopologyRelation};

fn resource(kind: &str, name: &str, namespace: &str, uid: &str) -> TopologyInputResource {
    TopologyInputResource {
        uid: uid.to_string(),
        owner: None,
        summary: ResourceSummary {
            kind: kind.to_string(),
            cluster: "kind-dev".to_string(),
            name: name.to_string(),
            namespace: Some(namespace.to_string()),
            age: "1m".to_string(),
            api_version: Some(
                match kind {
                    "Deployment" | "DaemonSet" | "ReplicaSet" | "StatefulSet" => "apps/v1",
                    "Job" | "CronJob" => "batch/v1",
                    _ => "v1",
                }
                .to_string(),
            ),
            group: None,
            version: None,
            plural: None,
            namespaced: Some(true),
            dynamic: None,
            created_at: None,
            status: Some("Running".to_string()),
            ready: Some("true".to_string()),
            restarts: None,
            owner_ref: None,
            argo_app: None,
            helm_release: None,
        },
    }
}

fn owned(
    kind: &str,
    name: &str,
    namespace: &str,
    uid: &str,
    owner_kind: &str,
    owner_name: &str,
    owner_uid: &str,
) -> TopologyInputResource {
    let mut input = resource(kind, name, namespace, uid);
    input.owner = Some(OwnerReferenceSummary {
        api_version: match owner_kind {
            "Deployment" | "DaemonSet" | "ReplicaSet" | "StatefulSet" => "apps/v1",
            "Job" | "CronJob" => "batch/v1",
            _ => "v1",
        }
        .to_string(),
        kind: owner_kind.to_string(),
        name: owner_name.to_string(),
        uid: owner_uid.to_string(),
    });
    input.summary.owner_ref = Some(owner_name.to_string());
    input
}

#[test]
fn topology_root_kinds_include_daemonset() {
    assert!(topology_root_kinds().contains(&"DaemonSet"));
}

#[test]
fn topology_standalone_kinds_include_ownerless_resources() {
    assert!(topology_standalone_kinds().contains(&"Service"));
    assert!(topology_standalone_kinds().contains(&"Ingress"));
    assert!(topology_standalone_kinds().contains(&"ConfigMap"));
    assert!(topology_standalone_kinds().contains(&"Secret"));
}

#[test]
fn builds_deployment_to_replicaset_to_pod_edges_from_owner_uids() {
    let topology = build_resource_topology(vec![
        resource("Deployment", "api", "default", "deploy-1"),
        owned(
            "ReplicaSet",
            "api-7d9",
            "default",
            "rs-1",
            "Deployment",
            "api",
            "deploy-1",
        ),
        owned(
            "Pod",
            "api-7d9-x",
            "default",
            "pod-1",
            "ReplicaSet",
            "api-7d9",
            "rs-1",
        ),
    ]);

    let deploy_id = topology_node_id("kind-dev", "apps/v1", "Deployment", Some("default"), "api");
    let rs_id = topology_node_id(
        "kind-dev",
        "apps/v1",
        "ReplicaSet",
        Some("default"),
        "api-7d9",
    );
    let pod_id = topology_node_id("kind-dev", "v1", "Pod", Some("default"), "api-7d9-x");

    assert!(topology.edges.iter().any(|edge| {
        edge.source == deploy_id && edge.target == rs_id && edge.relation == TopologyRelation::Owns
    }));
    assert!(topology.edges.iter().any(|edge| {
        edge.source == rs_id && edge.target == pod_id && edge.relation == TopologyRelation::Owns
    }));
    assert!(topology.warnings.is_empty());
}

#[test]
fn deduplicates_nodes_by_resource_identity() {
    let topology = build_resource_topology(vec![
        resource("Service", "api", "default", "svc-1"),
        resource("Service", "api", "default", "svc-1"),
    ]);

    assert_eq!(topology.nodes.len(), 1);
    assert_eq!(topology.nodes[0].kind, "Service");
    assert_eq!(topology.nodes[0].name, "api");
    assert!(topology.edges.is_empty());
}

#[test]
fn builds_daemonset_to_pod_edges_from_owner_uids() {
    let topology = build_resource_topology(vec![
        resource("DaemonSet", "alloy", "default", "ds-1"),
        owned(
            "Pod",
            "alloy-bqpd5",
            "default",
            "pod-1",
            "DaemonSet",
            "alloy",
            "ds-1",
        ),
    ]);

    let daemonset_id =
        topology_node_id("kind-dev", "apps/v1", "DaemonSet", Some("default"), "alloy");
    let pod_id = topology_node_id("kind-dev", "v1", "Pod", Some("default"), "alloy-bqpd5");

    assert!(topology.edges.iter().any(|edge| {
        edge.source == daemonset_id
            && edge.target == pod_id
            && edge.relation == TopologyRelation::Owns
    }));
    assert!(topology.warnings.is_empty());
}

#[test]
fn builds_cronjob_to_job_to_pod_edges_from_owner_uids() {
    let topology = build_resource_topology(vec![
        resource("CronJob", "nightly", "default", "cron-1"),
        owned(
            "Job",
            "nightly-287",
            "default",
            "job-1",
            "CronJob",
            "nightly",
            "cron-1",
        ),
        owned(
            "Pod",
            "nightly-287-x",
            "default",
            "pod-1",
            "Job",
            "nightly-287",
            "job-1",
        ),
    ]);

    let cronjob_id = topology_node_id(
        "kind-dev",
        "batch/v1",
        "CronJob",
        Some("default"),
        "nightly",
    );
    let job_id = topology_node_id(
        "kind-dev",
        "batch/v1",
        "Job",
        Some("default"),
        "nightly-287",
    );
    let pod_id = topology_node_id("kind-dev", "v1", "Pod", Some("default"), "nightly-287-x");

    assert!(topology.edges.iter().any(|edge| {
        edge.source == cronjob_id
            && edge.target == job_id
            && edge.relation == TopologyRelation::Owns
    }));
    assert!(topology.edges.iter().any(|edge| {
        edge.source == job_id && edge.target == pod_id && edge.relation == TopologyRelation::Owns
    }));
    assert!(topology.warnings.is_empty());
}

#[test]
fn links_statefulset_to_matching_pvc_without_owner_reference() {
    let topology = build_resource_topology(vec![
        resource("StatefulSet", "postgres", "default", "ss-1"),
        owned(
            "Pod",
            "postgres-0",
            "default",
            "pod-1",
            "StatefulSet",
            "postgres",
            "ss-1",
        ),
        resource(
            "PersistentVolumeClaim",
            "data-postgres-0",
            "default",
            "pvc-1",
        ),
    ]);

    let ss_id = topology_node_id(
        "kind-dev",
        "apps/v1",
        "StatefulSet",
        Some("default"),
        "postgres",
    );
    let pvc_id = topology_node_id(
        "kind-dev",
        "v1",
        "PersistentVolumeClaim",
        Some("default"),
        "data-postgres-0",
    );

    assert!(topology.edges.iter().any(|edge| {
        edge.source == ss_id && edge.target == pvc_id && edge.relation == TopologyRelation::Creates
    }));
}

#[test]
fn warns_when_owner_reference_target_is_missing() {
    let topology = build_resource_topology(vec![owned(
        "Pod",
        "api-orphan",
        "default",
        "pod-1",
        "ReplicaSet",
        "api-missing",
        "rs-missing",
    )]);

    assert_eq!(topology.edges.len(), 0);
    assert_eq!(topology.warnings.len(), 1);
    assert!(topology.warnings[0].contains("ReplicaSet/api-missing"));
}

#[test]
fn marks_incomplete_ready_ratios_as_attention() {
    let mut deployment = resource("Deployment", "api", "default", "deploy-1");
    deployment.summary.ready = Some("0/3".to_string());
    deployment.summary.status = Some("Available: 0".to_string());

    let topology = build_resource_topology(vec![deployment]);

    assert_eq!(topology.nodes[0].health, "attention");
}
