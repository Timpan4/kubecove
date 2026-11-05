use super::*;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use serde_json::json;

#[tokio::test]
async fn metrics_api_is_found_beside_an_api_with_null_verbs() {
    use kubecove_kube::commands::discovery_fixture::{self, FixtureKind};
    let metrics = |kind, plural, namespaced| FixtureKind {
        group: "metrics.k8s.io",
        version: "v1beta1",
        kind,
        plural,
        namespaced,
    };
    let client = discovery_fixture::client(&[
        metrics("PodMetrics", "pods", true),
        metrics("NodeMetrics", "nodes", false),
    ]);

    assert!(has_metrics_api(client).await.expect("metrics discovery"));
}

// Failure mode: a registered metrics API whose server is down (metrics-server
// not running) reads as a discovery failure with a raw error warning instead
// of an absent metrics API.
#[tokio::test]
async fn metrics_api_with_a_stopped_server_reads_as_unavailable() {
    use crate::commands::FixedClusterAccess;
    use kubecove_kube::commands::discovery_fixture;
    let access = FixedClusterAccess::new(discovery_fixture::client_with_unavailable_groups(
        &[],
        &[("metrics.k8s.io", "v1beta1")],
    ))
    .access();

    let summary = resource_metrics_from(&access, "fixture".into(), Vec::new())
        .await
        .expect("metrics summary");

    assert_eq!(
        summary.availability.status,
        ResourceMetricsAvailabilityStatus::Unavailable
    );
    assert_eq!(
        summary.availability.message.as_deref(),
        Some("metrics API unavailable")
    );
    assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
}

#[test]
fn workload_overflow_stays_unavailable_after_later_valid_samples() {
    let mut pods = Vec::new();
    let mut samples = Vec::new();
    for (index, memory) in [i64::MAX, 1, 1].into_iter().enumerate() {
        let name = format!("api-{index}");
        let mut pod = Pod::default();
        pod.metadata.name = Some(name.clone());
        pod.metadata.namespace = Some("default".into());
        pod.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "apps/v1".into(),
            kind: "StatefulSet".into(),
            name: "api".into(),
            uid: "owner".into(),
            controller: Some(true),
            block_owner_deletion: None,
        }]);
        pods.push(pod);
        samples.push(ResourceMetricSummary {
            kind: "Pod".into(),
            cluster: "dev".into(),
            name,
            namespace: Some("default".into()),
            cpu_millicores: Some(f64::MAX),
            memory_bytes: Some(memory),
            sampled_at: None,
            source_pods: Vec::new(),
        });
    }
    let workloads = aggregate_workload_metrics("dev", &pods, &[], &samples);
    assert_eq!(workloads.len(), 1);
    assert_eq!(workloads[0].memory_bytes, None);
    assert_eq!(workloads[0].cpu_millicores, None);
}

#[test]
fn parses_metrics_quantities() {
    assert_eq!(parse_cpu_millicores("250m"), Some(250.0));
    assert_eq!(parse_cpu_millicores("1"), Some(1000.0));
    assert_eq!(parse_cpu_millicores("125000000n"), Some(125.0));
    assert_eq!(parse_memory_bytes("64Mi"), Some(67_108_864));
    assert_eq!(parse_memory_bytes("1536Ki"), Some(1_572_864));
}

#[test]
fn rejects_non_finite_negative_and_out_of_range_metrics() {
    for value in ["NaN", "inf", "-1", "1e309"] {
        assert_eq!(parse_cpu_millicores(value), None, "{value}");
    }
    for value in [
        "NaNMi",
        "infMi",
        "-1Mi",
        "-0.0001Ki",
        "9223372036854775808Ki",
    ] {
        assert_eq!(parse_memory_bytes(value), None, "{value}");
    }
}

#[test]
fn overflowing_container_memory_is_unavailable() {
    let data = json!({"containers": [
        {"usage": {"memory": i64::MAX.to_string()}},
        {"usage": {"memory": "1"}},
        {"usage": {"memory": "1"}}
    ]});
    assert_eq!(container_usage_totals(&data).1, None);
}

#[test]
fn permission_classification_uses_status_not_resource_name() {
    let error = Error::Api(Box::new(kube::core::Status {
        code: 404,
        reason: "NotFound".to_string(),
        message: "pod forbidden-403 not found".to_string(),
        ..Default::default()
    }));
    assert_eq!(
        classify_metrics_error(&error),
        MetricsListStatus::Unavailable
    );
}

#[test]
fn normalizes_pod_metrics_from_container_usage() {
    let resource = metrics_api_resource("PodMetrics", "pods");
    let object = DynamicObject::new("api-0", &resource)
        .within("payments")
        .data(json!({
            "timestamp": "2026-05-22T12:00:00Z",
            "containers": [
                { "name": "api", "usage": { "cpu": "150m", "memory": "64Mi" } },
                { "name": "sidecar", "usage": { "cpu": "50000000n", "memory": "8Mi" } }
            ]
        }));

    let metric = metric_from_object("kind-dev", "Pod", &object);

    assert_eq!(metric.kind, "Pod");
    assert_eq!(metric.namespace.as_deref(), Some("payments"));
    assert_eq!(metric.cpu_millicores, Some(200.0));
    assert_eq!(metric.memory_bytes, Some(75_497_472));
    assert_eq!(metric.sampled_at.as_deref(), Some("2026-05-22T12:00:00Z"));
}

#[test]
fn aggregates_workload_metrics_from_owned_pods() {
    let mut pod = Pod::default();
    pod.metadata.name = Some("api-0".to_string());
    pod.metadata.namespace = Some("payments".to_string());
    pod.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "api-7d9".to_string(),
        uid: "rs-1".to_string(),
        controller: Some(true),
        block_owner_deletion: None,
    }]);
    let pod_metric = ResourceMetricSummary {
        kind: "Pod".to_string(),
        cluster: "kind-dev".to_string(),
        name: "api-0".to_string(),
        namespace: Some("payments".to_string()),
        cpu_millicores: Some(125.0),
        memory_bytes: Some(128),
        sampled_at: Some("2026-05-22T12:00:00Z".to_string()),
        source_pods: Vec::new(),
    };

    let workloads = aggregate_workload_metrics("kind-dev", &[pod], &[], &[pod_metric]);

    assert_eq!(workloads.len(), 1);
    assert_eq!(workloads[0].kind, "ReplicaSet");
    assert_eq!(workloads[0].name, "api-7d9");
    assert_eq!(workloads[0].cpu_millicores, Some(125.0));
    assert_eq!(workloads[0].source_pods, vec!["api-0"]);
}

#[test]
fn rolls_up_replicaset_metrics_to_deployment_owner() {
    let mut pod = Pod::default();
    pod.metadata.name = Some("api-0".to_string());
    pod.metadata.namespace = Some("payments".to_string());
    pod.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "api-7d9".to_string(),
        uid: "rs-1".to_string(),
        controller: Some(true),
        block_owner_deletion: None,
    }]);
    let mut replicaset = ReplicaSet::default();
    replicaset.metadata.name = Some("api-7d9".to_string());
    replicaset.metadata.namespace = Some("payments".to_string());
    replicaset.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: "api".to_string(),
        uid: "deploy-1".to_string(),
        controller: Some(true),
        block_owner_deletion: None,
    }]);
    let pod_metric = ResourceMetricSummary {
        kind: "Pod".to_string(),
        cluster: "kind-dev".to_string(),
        name: "api-0".to_string(),
        namespace: Some("payments".to_string()),
        cpu_millicores: Some(125.0),
        memory_bytes: Some(128),
        sampled_at: Some("2026-05-22T12:00:00Z".to_string()),
        source_pods: Vec::new(),
    };

    let workloads = aggregate_workload_metrics("kind-dev", &[pod], &[replicaset], &[pod_metric]);

    assert_eq!(workloads.len(), 2);
    let deployment = workloads
        .iter()
        .find(|metric| metric.kind == "Deployment" && metric.name == "api")
        .expect("deployment rollup");
    let replicaset = workloads
        .iter()
        .find(|metric| metric.kind == "ReplicaSet" && metric.name == "api-7d9")
        .expect("replicaset rollup");
    assert_eq!(deployment.cpu_millicores, Some(125.0));
    assert_eq!(deployment.memory_bytes, Some(128));
    assert_eq!(replicaset.cpu_millicores, Some(125.0));
    assert_eq!(replicaset.memory_bytes, Some(128));
}

#[test]
fn leaves_workload_metrics_empty_when_pod_sample_has_no_usage() {
    let mut pod = Pod::default();
    pod.metadata.name = Some("api-0".to_string());
    pod.metadata.namespace = Some("payments".to_string());
    pod.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "StatefulSet".to_string(),
        name: "api".to_string(),
        uid: "sts-1".to_string(),
        controller: Some(true),
        block_owner_deletion: None,
    }]);
    let pod_metric = ResourceMetricSummary {
        kind: "Pod".to_string(),
        cluster: "kind-dev".to_string(),
        name: "api-0".to_string(),
        namespace: Some("payments".to_string()),
        cpu_millicores: None,
        memory_bytes: None,
        sampled_at: Some("2026-05-22T12:00:00Z".to_string()),
        source_pods: Vec::new(),
    };

    let workloads = aggregate_workload_metrics("kind-dev", &[pod], &[], &[pod_metric]);

    assert_eq!(workloads.len(), 1);
    assert_eq!(workloads[0].cpu_millicores, None);
    assert_eq!(workloads[0].memory_bytes, None);
}

fn owner(kind: &str, name: &str, controller: Option<bool>) -> serde_json::Value {
    json!({ "apiVersion": "apps/v1", "kind": kind, "name": name, "uid": format!("{name}-uid"), "controller": controller })
}

fn full_pod(name: &str, owners: serde_json::Value) -> serde_json::Value {
    json!({
        "apiVersion": "v1", "kind": "Pod",
        "metadata": {
            "name": name, "namespace": "payments", "uid": format!("{name}-uid"),
            "labels": { "app": "api" },
            "annotations": { "kubectl.kubernetes.io/last-applied-configuration": "{}" },
            "managedFields": [{ "manager": "kubelet", "operation": "Update", "fieldsV1": { "f:status": {} } }],
            "ownerReferences": owners,
        },
        "spec": { "containers": [{ "name": "api", "image": "api:1", "ports": [{ "containerPort": 8080 }] }], "nodeName": "node-a" },
        "status": { "phase": "Running", "conditions": [{ "type": "Ready", "status": "True" }] },
    })
}

fn object_list(items: Vec<serde_json::Value>) -> serde_json::Value {
    json!({ "apiVersion": "v1", "kind": "List", "metadata": { "resourceVersion": "1" }, "items": items })
}

// Failure modes for the narrow metrics decoders: a changed usage sum, sample
// time, name or namespace; non-string, null or missing usage values or
// timestamps read differently; labels on metrics metadata or managed fields,
// spec and status on Pods changing the result; an owner reference lost from
// the rollup, including a controller listed after a non-controller owner;
// and either the all-namespace or the named-namespace request path diverging.
// The expected summary decodes the same responses completely, as the list
// API did before the projections.
#[tokio::test]
async fn narrow_metrics_decoding_matches_complete_decoding() {
    use crate::commands::FixedClusterAccess;
    use kube::api::ObjectList;
    use kubecove_kube::commands::discovery_fixture::{self, FixtureKind};
    let pods = object_list(vec![
        full_pod("api-0", json!([owner("ReplicaSet", "api-7d9", Some(true))])),
        full_pod("solo", serde_json::Value::Null),
        full_pod(
            "job-0",
            json!([
                owner("Job", "first", None),
                owner("Job", "batch", Some(true))
            ]),
        ),
    ]);
    let replicasets = object_list(vec![json!({
        "apiVersion": "apps/v1", "kind": "ReplicaSet",
        "metadata": { "name": "api-7d9", "namespace": "payments", "ownerReferences": [owner("Deployment", "api", Some(true))] },
        "spec": { "replicas": 1, "selector": { "matchLabels": { "app": "api" } } },
        "status": { "replicas": 1 },
    })]);
    let pod_metrics = object_list(vec![
        json!({ "apiVersion": "metrics.k8s.io/v1beta1", "kind": "PodMetrics",
            "metadata": { "name": "api-0", "namespace": "payments", "labels": { "app": "api" }, "creationTimestamp": "2026-01-01T00:00:00Z" },
            "timestamp": "2026-05-22T12:00:00Z", "window": "30s",
            "containers": [
                { "name": "api", "usage": { "cpu": "150m", "memory": "64Mi" } },
                { "name": "sidecar", "usage": { "cpu": 5, "memory": null } },
            ] }),
        json!({ "apiVersion": "metrics.k8s.io/v1beta1", "kind": "PodMetrics",
            "metadata": { "name": "solo", "namespace": "payments" }, "timestamp": null }),
        json!({ "apiVersion": "metrics.k8s.io/v1beta1", "kind": "PodMetrics",
            "metadata": { "name": "job-0", "namespace": "payments" }, "timestamp": "2026-05-22T12:00:05Z",
            "containers": [{ "name": "job", "usage": { "cpu": "20000000n", "memory": "1Gi" } }] }),
    ]);
    let node_metrics = object_list(vec![
        json!({ "apiVersion": "metrics.k8s.io/v1beta1", "kind": "NodeMetrics",
            "metadata": { "name": "node-a" }, "timestamp": "2026-05-22T12:00:00Z", "usage": { "cpu": "2", "memory": "2Gi" } }),
        json!({ "apiVersion": "metrics.k8s.io/v1beta1", "kind": "NodeMetrics",
            "metadata": { "name": "node-b" }, "timestamp": 7 }),
    ]);
    let complete = resource_metrics_projection(
        "fixture",
        &serde_json::from_value::<ObjectList<DynamicObject>>(pod_metrics.clone())
            .unwrap()
            .items,
        &serde_json::from_value::<ObjectList<DynamicObject>>(node_metrics.clone())
            .unwrap()
            .items,
        &serde_json::from_value::<ObjectList<Pod>>(pods.clone())
            .unwrap()
            .items,
        &serde_json::from_value::<ObjectList<ReplicaSet>>(replicasets.clone())
            .unwrap()
            .items,
        Vec::new(),
        false,
        false,
    );
    assert_eq!(complete.workloads.len(), 3, "fixture exercises the rollup");
    let kinds = [
        FixtureKind {
            group: "metrics.k8s.io",
            version: "v1beta1",
            kind: "PodMetrics",
            plural: "pods",
            namespaced: true,
        },
        FixtureKind {
            group: "metrics.k8s.io",
            version: "v1beta1",
            kind: "NodeMetrics",
            plural: "nodes",
            namespaced: false,
        },
    ];
    for (namespaces, prefix) in [
        (Vec::new(), String::new()),
        (
            vec!["payments".to_string()],
            "/namespaces/payments".to_string(),
        ),
    ] {
        let lists = [
            (format!("/api/v1{prefix}/pods"), pods.clone()),
            (
                format!("/apis/apps/v1{prefix}/replicasets"),
                replicasets.clone(),
            ),
            (
                format!("/apis/metrics.k8s.io/v1beta1{prefix}/pods"),
                pod_metrics.clone(),
            ),
            (
                "/apis/metrics.k8s.io/v1beta1/nodes".to_string(),
                node_metrics.clone(),
            ),
        ];
        let access = FixedClusterAccess::new(discovery_fixture::client_with_lists(
            &kinds,
            lists.into_iter().collect(),
        ))
        .access();
        let narrow = resource_metrics_from(&access, "fixture".into(), namespaces)
            .await
            .expect("metrics summary");
        assert_eq!(narrow, complete, "prefix {prefix:?}");
    }
}
