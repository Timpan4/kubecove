use crate::{commands::metrics, models::ResourceMetricsSummary};
use k8s_openapi::api::{apps::v1::ReplicaSet, core::v1::Pod};
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct ResourceMetricsFixture {
    pod_metrics: Vec<DynamicObject>,
    node_metrics: Vec<DynamicObject>,
    pods: Vec<Pod>,
    replica_sets: Vec<ReplicaSet>,
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("valid metrics benchmark object")
}

pub fn resource_metrics_fixture() -> ResourceMetricsFixture {
    let mut pods = Vec::with_capacity(10_007);
    let mut pod_metrics = Vec::with_capacity(10_007);
    for index in 0..10_000 {
        let name = format!("profile-pod-{index}");
        let pod: Pod = parse(json!({
            "apiVersion":"v1","kind":"Pod",
            "metadata":{"name":name,"namespace":"profile","uid":name,"creationTimestamp":"2026-01-01T00:00:00Z","labels":{"app":"profile"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"profile-rs","uid":"profile-rs","controller":true}]},
            "spec":{"containers":[{"name":"app","image":"fixture","ports":[{"containerPort":8080}]}],"nodeName":"profile-node"},
            "status":{"phase":"Running","conditions":[{"type":"Ready","status":"True"}],"containerStatuses":[{"name":"app","ready":true,"restartCount":0,"image":"fixture","imageID":"fixture","state":{"running":{"startedAt":"2026-01-01T00:00:00Z"}}}]}
        }));
        let metric_name = if index == 9_999 {
            "missing-pod-metric".to_string()
        } else {
            name.clone()
        };
        let (timestamp, containers) = match index {
            0 => (
                "2026-01-01T00:05:00Z",
                json!([
                    {"name":"app","usage":{"cpu":"10m","memory":"1024Ki"}},
                    {"name":"sidecar","usage":{"cpu":"50m","memory":"512Ki"}}
                ]),
            ),
            1 => (
                "2026-01-01T00:10:00Z",
                json!([
                    {"name":"app","usage":{"cpu":"20m","memory":"2Mi"}}
                ]),
            ),
            2 => (
                "2026-01-01T00:02:00Z",
                json!([
                    {"name":"invalid","usage":{"cpu":"NaN","memory":"-1Mi"}},
                    {"name":"overflow-a","usage":{"cpu":"5m","memory":"9223372036854775807"}},
                    {"name":"overflow-b","usage":{"memory":"1"}}
                ]),
            ),
            _ => (
                "2026-01-01T00:00:00Z",
                json!([
                    {"name":"app","usage":{"cpu":"10m","memory":"1024Ki"}}
                ]),
            ),
        };
        pod_metrics.push(parse(json!({
            "apiVersion":"metrics.k8s.io/v1beta1","kind":"PodMetrics",
            "metadata":{"name":metric_name,"namespace":"profile","creationTimestamp":"2026-01-01T00:00:00Z"},
            "timestamp":timestamp,"window":"30s","containers":containers
        })));
        pods.push(pod);
    }

    let node_metrics = vec![parse(json!({
        "apiVersion":"metrics.k8s.io/v1beta1","kind":"NodeMetrics",
        "metadata":{"name":"profile-node","creationTimestamp":"2026-01-01T00:00:00Z"},
        "timestamp":"2026-01-01T00:00:00Z","window":"30s",
        "usage":{"cpu":"2","memory":"2Gi"}
    }))];
    let replica_set: ReplicaSet = parse(json!({
        "apiVersion":"apps/v1","kind":"ReplicaSet",
        "metadata":{"name":"profile-rs","namespace":"profile","uid":"profile-rs","ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":"profile-deployment","uid":"profile-deployment","controller":true}]},
        "spec":{"replicas":10_000,"selector":{"matchLabels":{"app":"profile"}},"template":{"metadata":{"labels":{"app":"profile"}},"spec":{"containers":[{"name":"app","image":"fixture"}]}}}
    }));
    let mut replica_sets = vec![replica_set];
    for (namespace, name, uid, deployment, deployment_uid) in [
        (
            "tenant-a",
            "shared-rs",
            "shared-rs-a",
            "shared-deploy",
            "shared-deploy-a",
        ),
        (
            "tenant-b",
            "shared-rs",
            "shared-rs-b",
            "shared-deploy",
            "shared-deploy-b",
        ),
        (
            "overflow",
            "overflow-rs",
            "overflow-rs",
            "overflow-deploy",
            "overflow-deploy",
        ),
    ] {
        replica_sets.push(parse(json!({
            "apiVersion":"apps/v1","kind":"ReplicaSet",
            "metadata":{"name":name,"namespace":namespace,"uid":uid,"ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":deployment,"uid":deployment_uid,"controller":true}]},
            "spec":{"replicas":1,"selector":{"matchLabels":{"app":"fixture"}},"template":{"metadata":{"labels":{"app":"fixture"}},"spec":{"containers":[{"name":"app","image":"fixture"}]}}}
        })));
    }

    let sentinels = [
        (
            "direct-early",
            "direct",
            "StatefulSet",
            "direct-ledger",
            "direct-ledger",
            "2026-01-01T00:01:00Z",
            json!([{"name":"app","usage":{"cpu":"25m","memory":"512Ki"}}]),
        ),
        (
            "direct-late",
            "direct",
            "StatefulSet",
            "direct-ledger",
            "direct-ledger",
            "2026-01-01T00:03:00Z",
            json!([{"name":"app","usage":{"cpu":"75m","memory":"1Mi"}}]),
        ),
        (
            "collision-pod",
            "tenant-a",
            "ReplicaSet",
            "shared-rs",
            "shared-rs-a",
            "2026-01-01T00:11:00Z",
            json!([{"name":"app","usage":{"cpu":"100m","memory":"1Mi"}}]),
        ),
        (
            "collision-pod",
            "tenant-b",
            "ReplicaSet",
            "shared-rs",
            "shared-rs-b",
            "2026-01-01T00:12:00Z",
            json!([{"name":"app","usage":{"cpu":"300m","memory":"2Mi"}}]),
        ),
        (
            "overflow-a",
            "overflow",
            "ReplicaSet",
            "overflow-rs",
            "overflow-rs",
            "2026-01-01T00:40:00Z",
            json!([{"name":"app","usage":{"cpu":"1e305","memory":"9223372036854775807"}}]),
        ),
        (
            "overflow-b",
            "overflow",
            "ReplicaSet",
            "overflow-rs",
            "overflow-rs",
            "2026-01-01T00:50:00Z",
            json!([{"name":"app","usage":{"cpu":"1e305","memory":"1"}}]),
        ),
        (
            "unavailable-pod",
            "unavailable",
            "StatefulSet",
            "unavailable-ledger",
            "unavailable-ledger",
            "2026-01-01T00:04:00Z",
            json!([{"name":"app","usage":{"cpu":"NaN","memory":"-1Mi"}}]),
        ),
    ];
    for (name, namespace, owner_kind, owner_name, owner_uid, timestamp, containers) in sentinels {
        pods.push(parse(json!({
            "apiVersion":"v1","kind":"Pod",
            "metadata":{"name":name,"namespace":namespace,"uid":format!("pod-{namespace}-{name}"),"creationTimestamp":"2026-01-01T00:00:00Z","ownerReferences":[{"apiVersion":"apps/v1","kind":owner_kind,"name":owner_name,"uid":owner_uid,"controller":true}]},
            "spec":{"containers":[{"name":"app","image":"fixture"}]}
        })));
        pod_metrics.push(parse(json!({
            "apiVersion":"metrics.k8s.io/v1beta1","kind":"PodMetrics",
            "metadata":{"name":name,"namespace":namespace,"creationTimestamp":"2026-01-01T00:00:00Z"},
            "timestamp":timestamp,"window":"30s","containers":containers
        })));
    }

    ResourceMetricsFixture {
        pod_metrics,
        node_metrics,
        pods,
        replica_sets,
    }
}

pub fn project_resource_metrics(fixture: &ResourceMetricsFixture) -> ResourceMetricsSummary {
    metrics::resource_metrics_projection(
        "metrics-cluster",
        &fixture.pod_metrics,
        &fixture.node_metrics,
        &fixture.pods,
        &fixture.replica_sets,
        Vec::new(),
        false,
        false,
    )
}
