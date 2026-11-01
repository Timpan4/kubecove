//! One built-in object must produce the same summary as a list row, in details and as a
//! topology node. Each case also pins the status, readiness and health its kind reports.
use super::super::{
    details::resource_details_from,
    summary::resources_summary_from,
    topology::build_resource_topology,
    topology_collection::{collect_network_topology_inputs, collect_topology_inputs},
    topology_network::build_network_flow_topology,
};
use crate::commands::{
    builtin_kinds::{builtin_kind, BUILTIN_KINDS},
    FixedClusterAccess,
};
use crate::models::{HealthAssessment, HealthAssessmentState, ResourceHealth, ResourceSummary};
use http::{Request, Response};
use kube::{client::Body, Client};
use serde_json::{json, Value};
use HealthAssessmentState::{Degraded, Healthy, NeedsAttention, NotEvaluated, Unknown};

const CONTEXT: &str = "parity";
const NAMESPACE: &str = "tenant";

#[derive(Clone, Copy, PartialEq)]
enum Topology {
    /// Production topology does not collect this kind.
    Absent,
    Ownership,
    /// Network flow also shows the kind as a node.
    Network,
    /// Topology lists metadata only, so the node has identity and ownership without status.
    MetadataOnly,
}

struct Case {
    label: &'static str,
    object: Value,
    status: Option<&'static str>,
    ready: Option<&'static str>,
    restarts: Option<i32>,
    health: HealthAssessmentState,
    topology: Topology,
}

/// `label: object => status, ready, restarts, health, topology;`
macro_rules! cases {
    ($($label:literal: $object:expr => $status:expr, $ready:expr, $restarts:expr, $health:ident, $topology:ident;)*) => {
        vec![$(Case {
            label: $label, object: $object, status: $status, ready: $ready, restarts: $restarts,
            health: $health, topology: Topology::$topology,
        }),*]
    };
}

fn cases() -> Vec<Case> {
    cases! {
        "running ready Pod": pod("Running", Some("True"), 2) => Some("Running"), Some("True"), Some(2), Healthy, Ownership;
        "succeeded Pod with false readiness and restarts": pod("Succeeded", Some("False"), 2) => Some("Succeeded"), Some("False"), Some(2), Healthy, Ownership;
        "running Pod with false readiness": pod("Running", Some("False"), 0) => Some("Running"), Some("False"), None, Degraded, Ownership;
        "pending Pod": pod("Pending", Some("False"), 0) => Some("Starting"), Some("False"), None, NeedsAttention, Ownership;
        "Pod in unknown phase": pod("Unknown", None, 0) => Some("Unknown"), None, None, Unknown, Ownership;
        "Pod with empty phase": pod("", None, 0) => None, None, None, NotEvaluated, Ownership;
        "available Deployment": apps("Deployment", json!({"replicas": 3, "updatedReplicas": 3, "readyReplicas": 3, "availableReplicas": 3})) => Some("Available: 3"), Some("3/3"), None, Healthy, Ownership;
        "unready Deployment": apps("Deployment", json!({"replicas": 3})) => Some("Rolling out"), Some("0/3"), None, NeedsAttention, Ownership;
        "scaled-to-zero Deployment": apps("Deployment", json!({})) => None, Some("0/0"), None, Unknown, Ownership;
        "ReplicaSet": apps("ReplicaSet", json!({"replicas": 3, "readyReplicas": 2, "availableReplicas": 1})) => Some("Starting"), Some("2/3"), None, NeedsAttention, Ownership;
        "StatefulSet": apps("StatefulSet", json!({"replicas": 3, "readyReplicas": 3})) => None, Some("3/3"), None, Healthy, Ownership;
        "DaemonSet": apps("DaemonSet", json!({"currentNumberScheduled": 5, "desiredNumberScheduled": 5, "numberMisscheduled": 0, "numberReady": 4, "numberAvailable": 4})) => None, Some("4/5"), None, NeedsAttention, Ownership;
        "failed Job": job(json!({"failed": 1, "succeeded": 3})) => Some("Failed"), Some("3/3"), None, Degraded, Ownership;
        "complete Job": job(json!({"succeeded": 1})) => Some("Complete"), Some("1/3"), None, Healthy, Ownership;
        "active Job": job(json!({"active": 1})) => Some("Active"), Some("0/3"), None, NeedsAttention, Ownership;
        "Job without counts": job(json!({})) => Some("Pending"), Some("0/3"), None, NeedsAttention, Ownership;
        "active CronJob": cron_job(json!({"active": [{"name": "run-1"}, {"name": "run-2"}]})) => Some("2 active"), None, None, Unknown, Ownership;
        "idle CronJob": cron_job(json!({})) => None, None, None, NotEvaluated, Ownership;
        "addressed Ingress": ingress(json!({"ingress": [{"ip": "10.0.0.1"}]})) => Some("Ready"), Some("true"), None, Healthy, Network;
        "unaddressed Ingress": ingress(json!({})) => Some("Pending"), Some("false"), None, NeedsAttention, Network;
        "Service": namespaced("Service", "v1", json!({"spec": {"type": "ClusterIP", "selector": {"app": "workload"}}})) => None, None, None, NotEvaluated, Network;
        "ConfigMap": namespaced("ConfigMap", "v1", json!({"data": {"key": "value"}})) => None, None, None, NotEvaluated, MetadataOnly;
        "Secret": namespaced("Secret", "v1", json!({"data": {"password": "c2VjcmV0"}})) => None, None, None, NotEvaluated, MetadataOnly;
        "bound PersistentVolumeClaim": namespaced("PersistentVolumeClaim", "v1", json!({"status": {"phase": "Bound"}})) => Some("Bound"), None, None, NotEvaluated, Ownership;
        "pending PersistentVolumeClaim": namespaced("PersistentVolumeClaim", "v1", json!({"status": {"phase": "Pending"}})) => Some("Pending"), None, None, NotEvaluated, Ownership;
        "ready Node": node("True") => Some("True"), Some("1/1"), None, Healthy, Absent;
        "unready Node": node("False") => Some("False"), Some("0/1"), None, NeedsAttention, Absent;
        "bound PersistentVolume": cluster("PersistentVolume", "v1", json!({"status": {"phase": "Bound"}})) => Some("Bound"), None, None, Unknown, Absent;
        "StorageClass": cluster("StorageClass", "storage.k8s.io/v1", json!({"provisioner": "example.csi"})) => None, None, None, NotEvaluated, Ownership;
        "established CustomResourceDefinition": crd() => Some("Established: True"), None, None, Unknown, MetadataOnly;
    }
}

fn object(kind: &str, api_version: &str, body: Value) -> Value {
    let mut value = json!({
        "apiVersion": api_version,
        "kind": kind,
        "metadata": {
            "name": "workload",
            "uid": "uid-workload",
            "creationTimestamp": "2026-10-01T00:00:00Z",
            "labels": {"helm.sh/release": "release", "app": "workload"},
            "annotations": {"argocd.argoproj.io/tracking-id": "app:/Workload:tenant/workload"},
            "ownerReferences": [{
                "apiVersion": "example.com/v1", "kind": "Owner", "name": "owner",
                "uid": "uid-owner", "controller": true
            }]
        }
    });
    let fields = value.as_object_mut().unwrap();
    fields.extend(body.as_object().unwrap().clone());
    value
}

fn namespaced(kind: &str, api_version: &str, body: Value) -> Value {
    let mut value = object(kind, api_version, body);
    value["metadata"]["namespace"] = json!(NAMESPACE);
    value
}

fn cluster(kind: &str, api_version: &str, body: Value) -> Value {
    object(kind, api_version, body)
}

fn pod(phase: &str, ready: Option<&str>, restarts: i32) -> Value {
    let conditions: Vec<Value> = ready
        .map(|status| json!({"type": "Ready", "status": status}))
        .into_iter()
        .collect();
    let container = json!({"name": "app", "image": "app", "imageID": "id", "ready": false, "restartCount": restarts});
    namespaced(
        "Pod",
        "v1",
        json!({
            "spec": {"containers": [{"name": "app", "ports": [{"name": "http", "containerPort": 8080}]}]},
            "status": {"phase": phase, "conditions": conditions, "containerStatuses": [container]}
        }),
    )
}

fn apps(kind: &str, status: Value) -> Value {
    let spec = json!({"selector": {}, "template": {}});
    namespaced(kind, "apps/v1", json!({"spec": spec, "status": status}))
}

fn job(status: Value) -> Value {
    let spec = json!({"completions": 3, "template": {}});
    namespaced("Job", "batch/v1", json!({"spec": spec, "status": status}))
}

fn cron_job(status: Value) -> Value {
    let spec = json!({"schedule": "* * * * *", "jobTemplate": {}});
    namespaced(
        "CronJob",
        "batch/v1",
        json!({"spec": spec, "status": status}),
    )
}

fn ingress(load_balancer: Value) -> Value {
    let status = json!({"loadBalancer": load_balancer});
    namespaced(
        "Ingress",
        "networking.k8s.io/v1",
        json!({"spec": {}, "status": status}),
    )
}

fn node(ready: &str) -> Value {
    let conditions = json!([{"type": "Ready", "status": ready}]);
    cluster("Node", "v1", json!({"status": {"conditions": conditions}}))
}

fn crd() -> Value {
    let names = json!({"kind": "Widget", "plural": "widgets"});
    let conditions = json!([{"type": "NamesAccepted", "status": "True"}, {"type": "Established", "status": "True"}]);
    cluster(
        "CustomResourceDefinition",
        "apiextensions.k8s.io/v1",
        json!({
            "spec": {"group": "example.com", "names": names, "scope": "Namespaced", "versions": []},
            "status": {"conditions": conditions, "acceptedNames": names, "storedVersions": ["v1"]}
        }),
    )
}

/// Serves `object` for its own GET and inside its kind's lists; every other list is empty.
fn mock_client(object: Value) -> Client {
    let kind = object["kind"].as_str().unwrap();
    let plural = builtin_kind(kind).expect("built-in kind").plural;
    let item_path = format!("/{plural}/workload");
    let list_path = format!("/{plural}");
    let service = tower::service_fn(move |request: Request<Body>| {
        let path = request.uri().path();
        let body = if path.ends_with(&item_path) {
            object.clone()
        } else {
            let items: Vec<Value> = path
                .ends_with(&list_path)
                .then(|| object.clone())
                .into_iter()
                .collect();
            json!({"apiVersion": "v1", "kind": "List", "metadata": {}, "items": items})
        };
        let body = body.to_string().into_bytes();
        async move { Ok::<_, std::convert::Infallible>(Response::new(Body::from(body))) }
    });
    Client::new(service, "default")
}

/// Ages are computed from the wall clock on every surface, so compare everything else.
fn comparable(summary: &ResourceSummary) -> Value {
    let mut value = serde_json::to_value(summary).unwrap();
    value["age"] = json!("compared separately");
    value
}

fn without_status(summary: &ResourceSummary) -> ResourceSummary {
    ResourceSummary {
        status: None,
        ready: None,
        restarts: None,
        health: ResourceHealth::default(),
        health_assessment: HealthAssessment::default(),
        ..summary.clone()
    }
}

#[test]
fn every_listed_builtin_kind_has_a_parity_case() {
    let cases = cases();
    for entry in BUILTIN_KINDS.iter().filter(|entry| entry.table) {
        assert!(
            cases.iter().any(|case| case.object["kind"] == entry.kind),
            "{} has no parity case",
            entry.kind
        );
    }
}

#[tokio::test]
async fn list_details_and_topology_share_one_summary_per_builtin_kind() {
    for case in cases() {
        let label = case.label;
        let kind = case.object["kind"].as_str().unwrap().to_string();
        let namespace = case.object["metadata"]["namespace"]
            .as_str()
            .map(str::to_string);
        let client = mock_client(case.object.clone());
        let access = FixedClusterAccess::new(client.clone()).access();

        let rows = resources_summary_from(&access, CONTEXT.into(), kind.clone(), namespace.clone())
            .await
            .unwrap_or_else(|error| panic!("{label}: list failed: {}", error.message));
        let [list] = rows.as_slice() else {
            panic!("{label}: expected one list row, got {}", rows.len());
        };
        let identity = (list.api_version.as_deref(), list.namespace.as_deref());
        let expected_identity = (case.object["apiVersion"].as_str(), namespace.as_deref());
        assert_eq!(identity, expected_identity, "{label}");
        let created_at = list.created_at.as_deref();
        assert_eq!(created_at, Some("2026-10-01T00:00:00+00:00"), "{label}");
        let owner = list
            .git_ops_owner
            .as_ref()
            .expect("GitOps ownership evidence");
        let ownership = (
            owner.provider.as_str(),
            list.argo_app.as_deref(),
            list.helm_release.as_deref(),
            list.owner_ref.as_deref(),
        );
        let expected_ownership = ("argo", Some("app"), Some("release"), Some("owner"));
        assert_eq!(ownership, expected_ownership, "{label}");
        assert!(!list.git_ops_ownership_partial, "{label}");
        let status = (list.status.as_deref(), list.ready.as_deref(), list.restarts);
        assert_eq!(status, (case.status, case.ready, case.restarts), "{label}");
        assert_eq!(list.health_assessment.state, case.health, "{label}");

        let details = resource_details_from(
            &access,
            CONTEXT.into(),
            kind.clone(),
            "workload".into(),
            namespace.clone(),
        )
        .await
        .unwrap_or_else(|error| panic!("{label}: details failed: {}", error.message));
        let details = comparable(&details.summary);
        assert_eq!(details, comparable(list), "{label}: details");

        if case.topology == Topology::Absent {
            continue;
        }
        let namespaces = vec![NAMESPACE.to_string()];
        let collection = Box::pin(collect_topology_inputs(
            client.clone(),
            CONTEXT,
            &namespaces,
            Vec::new(),
            true,
        ));
        let inputs = collection
            .await
            .unwrap_or_else(|error| panic!("{label}: topology failed: {}", error.message));
        let topology = build_resource_topology(inputs.resources);
        let node = topology.nodes.iter().find(|node| node.kind == kind);
        let node = node.unwrap_or_else(|| panic!("{label}: missing topology node"));
        let expected = if case.topology == Topology::MetadataOnly {
            without_status(list)
        } else {
            list.clone()
        };
        let summary = comparable(&node.summary);
        assert_eq!(summary, comparable(&expected), "{label}: topology");
        let node_status = (node.status.as_deref(), node.health);
        let expected_status = (expected.status.as_deref(), expected.health);
        assert_eq!(node_status, expected_status, "{label}: topology node");

        if case.topology != Topology::Network {
            continue;
        }
        let inputs = collect_network_topology_inputs(client, CONTEXT, &namespaces)
            .await
            .unwrap_or_else(|error| panic!("{label}: network failed: {}", error.message));
        let topology = build_network_flow_topology(inputs);
        let node = topology.nodes.iter().find(|node| node.kind == kind);
        let node = node.unwrap_or_else(|| panic!("{label}: missing network node"));
        let summary = comparable(&node.summary);
        assert_eq!(summary, comparable(list), "{label}: network");
        // Network flow labels Services with their type; the summary keeps the shared status.
        let status = if kind == "Service" {
            Some("ClusterIP")
        } else {
            case.status
        };
        assert_eq!(node.status.as_deref(), status, "{label}: network node");
    }
}
