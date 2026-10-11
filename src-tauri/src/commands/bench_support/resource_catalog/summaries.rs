use crate::{
    commands::{discovery, namespaces, resources},
    models::{DiscoveredResourceKind, NamespaceSummary, ResourceSummary},
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet},
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service},
    networking::v1::Ingress,
    storage::v1::StorageClass,
};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

const CLUSTER: &str = "catalog-cluster";
const BASE_TIME: &str = "2026-09-30T12:00:00Z";

#[derive(Clone)]
pub struct ResourceSummaryFixture {
    pub pods: Vec<Pod>,
    pub service: Service,
    pub config_map: ConfigMap,
    pub secret: Secret,
    pub claim: PersistentVolumeClaim,
    pub deployment: Deployment,
    pub replica_set: ReplicaSet,
    pub stateful_set: StatefulSet,
    pub daemon_set: DaemonSet,
    pub ingress: Ingress,
    pub job: Job,
    pub cron_job: CronJob,
    pub node: Node,
    pub storage_class: StorageClass,
    pub volume: PersistentVolume,
    pub crd: CustomResourceDefinition,
}

#[derive(Clone)]
pub struct DynamicSummaryFixture {
    pub kind: DiscoveredResourceKind,
    pub objects: Vec<DynamicObject>,
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("valid benchmark object")
}

fn metadata(name: String, index: usize) -> Value {
    json!({
        "name": name,
        "namespace": format!("tenant-{}", index % 100),
        "uid": format!("uid-{index}"),
        "creationTimestamp": BASE_TIME,
        "labels": {
            "app.kubernetes.io/name": "ledger",
            "app.kubernetes.io/part-of": "payments",
            "kustomize.toolkit.fluxcd.io/name": "ledger-flux",
            "kustomize.toolkit.fluxcd.io/namespace": "flux-system"
        },
        "annotations": {
            "argocd.argoproj.io/name": "ledger-argo",
            "argocd.argoproj.io/tracking-id": "tracking-fallback:apps/Deployment:tenant/api",
            "kubectl.kubernetes.io/last-applied-configuration": "{\"apiVersion\":\"v1\",\"kind\":\"Pod\"}"
        },
        "ownerReferences": [{
            "apiVersion": "apps/v1",
            "kind": "ReplicaSet",
            "name": "ledger-65d7f44f4b",
            "uid": "replicaset-ledger",
            "controller": true
        }]
    })
}

fn pod_value(index: usize) -> Value {
    let restart_count = i32::try_from(index % 4).expect("restart remainder fits in i32");
    let sidecar_restart_count = i32::try_from(index % 3).expect("restart remainder fits in i32");
    let phase = match index % 7 {
        0 => "Succeeded",
        1 => "Failed",
        _ => "Running",
    };
    let conditions = match index % 4 {
        0 => json!([
            {"type":"PodScheduled","status":"True"},
            {"type":"Ready","status":if phase == "Running" {"True"} else {"False"}},
            {"type":"ContainersReady","status":if phase == "Running" {"True"} else {"False"}}
        ]),
        1 => json!([
            {"type":"Ready","status":"False"},
            {"type":"Initialized","status":"True"},
            {"type":"ContainersReady","status":"False"}
        ]),
        2 => json!([
            {"type":"Ready","status":"True"},
            {"type":"Ready","status":"False"},
            {"type":"ContainersReady","status":"False"}
        ]),
        _ => json!([
            {"type":"PodScheduled","status":"True"},
            {"type":"Initialized","status":"True"}
        ]),
    };
    let status = if index.is_multiple_of(19) {
        json!({})
    } else {
        json!({
            "phase": phase,
            "conditions": conditions,
            "containerStatuses": [
                {
                    "name": "ledger",
                    "ready": phase == "Running",
                    "restartCount": restart_count,
                    "image": "ghcr.io/example/ledger:1.0.0",
                    "imageID": "containerd://image",
                    "containerID": format!("containerd://container-{index}"),
                    "state": {"running": {"startedAt": BASE_TIME}},
                    "lastState": if index.is_multiple_of(4) { json!({}) } else { json!({"terminated":{"containerID":format!("containerd://previous-{index}"),"exitCode":1,"finishedAt":BASE_TIME,"reason":"Error"}}) }
                },
                {
                    "name": "metrics",
                    "ready": phase == "Running" && index.is_multiple_of(2),
                    "restartCount": sidecar_restart_count,
                    "image": "ghcr.io/example/metrics:1.0.0",
                    "imageID": "containerd://metrics-image",
                    "containerID": format!("containerd://metrics-{index}"),
                    "state": {"running": {"startedAt": BASE_TIME}},
                    "lastState": {}
                }
            ]
        })
    };
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": metadata(format!("ledger-{index}"), index),
        "spec": {"containers": [
            {"name": "ledger", "image": "ghcr.io/example/ledger:1.0.0", "ports": [{"name": "http", "containerPort": 8080}]},
            {"name": "metrics", "image": "ghcr.io/example/metrics:1.0.0"}
        ]},
        "status": status
    })
}

pub fn resource_summary_fixture(pod_count: usize) -> ResourceSummaryFixture {
    let pods = (0..pod_count)
        .map(|index| parse(pod_value(index)))
        .collect();
    let md = |name: &str| metadata(name.to_string(), 0);
    ResourceSummaryFixture {
        pods,
        service: parse(
            json!({"apiVersion":"v1","kind":"Service","metadata":md("ledger"),"spec":{"type":"ClusterIP","selector":{"app.kubernetes.io/name":"ledger"},"ports":[{"name":"http","port":8080,"targetPort":8080}]}}),
        ),
        config_map: parse(
            json!({"apiVersion":"v1","kind":"ConfigMap","metadata":md("ledger-config"),"data":{"LOG_LEVEL":"info"}}),
        ),
        secret: parse(
            json!({"apiVersion":"v1","kind":"Secret","metadata":md("ledger-secret"),"type":"Opaque","data":{"password":"c2VjcmV0"}}),
        ),
        claim: parse(
            json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":md("ledger-data"),"spec":{"accessModes":["ReadWriteOnce"],"resources":{"requests":{"storage":"10Gi"}},"storageClassName":"fast"},"status":{"phase":"Bound"}}),
        ),
        deployment: parse(
            json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":md("ledger"),"spec":{"replicas":3,"selector":{"matchLabels":{"app":"ledger"}},"template":{"metadata":{"labels":{"app":"ledger"}},"spec":{"containers":[{"name":"ledger","image":"ghcr.io/example/ledger:1.0.0"}]}}},"status":{"replicas":3,"readyReplicas":2,"availableReplicas":2}}),
        ),
        replica_set: parse(
            json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":md("ledger-65d7f44f4b"),"spec":{"selector":{"matchLabels":{"app":"ledger"}},"template":{"metadata":{"labels":{"app":"ledger"}},"spec":{"containers":[{"name":"ledger","image":"ghcr.io/example/ledger:1.0.0"}]}}},"status":{"replicas":3,"readyReplicas":2,"availableReplicas":2}}),
        ),
        stateful_set: parse(
            json!({"apiVersion":"apps/v1","kind":"StatefulSet","metadata":md("ledger"),"spec":{"serviceName":"ledger","replicas":3,"selector":{"matchLabels":{"app":"ledger"}},"template":{"metadata":{"labels":{"app":"ledger"}},"spec":{"containers":[{"name":"ledger","image":"ghcr.io/example/ledger:1.0.0","volumeMounts":[{"name":"data","mountPath":"/var/lib/ledger"}]}]}},"volumeClaimTemplates":[{"metadata":{"name":"data"},"spec":{"accessModes":["ReadWriteOnce"],"resources":{"requests":{"storage":"10Gi"}}}}]},"status":{"replicas":3,"readyReplicas":2}}),
        ),
        daemon_set: parse(
            json!({"apiVersion":"apps/v1","kind":"DaemonSet","metadata":md("node-agent"),"spec":{"selector":{"matchLabels":{"app":"node-agent"}},"template":{"metadata":{"labels":{"app":"node-agent"}},"spec":{"containers":[{"name":"agent","image":"ghcr.io/example/agent:1.0.0"}]}}},"status":{"desiredNumberScheduled":5,"numberReady":4,"numberAvailable":4}}),
        ),
        ingress: parse(
            json!({"apiVersion":"networking.k8s.io/v1","kind":"Ingress","metadata":md("ledger"),"spec":{"rules":[{"host":"ledger.example.com","http":{"paths":[{"path":"/","pathType":"Prefix","backend":{"service":{"name":"ledger","port":{"number":8080}}}}]}}]},"status":{"loadBalancer":{"ingress":[{"hostname":"lb.example.com"}]}}}),
        ),
        job: parse(
            json!({"apiVersion":"batch/v1","kind":"Job","metadata":md("ledger-migrate"),"spec":{"completions":1,"template":{"spec":{"restartPolicy":"Never","containers":[{"name":"migrate","image":"ghcr.io/example/ledger:1.0.0"}]}}},"status":{"active":0,"succeeded":1,"failed":0}}),
        ),
        cron_job: parse(
            json!({"apiVersion":"batch/v1","kind":"CronJob","metadata":md("ledger-backup"),"spec":{"schedule":"0 * * * *","jobTemplate":{"spec":{"template":{"spec":{"restartPolicy":"Never","containers":[{"name":"backup","image":"ghcr.io/example/ledger:1.0.0"}]}}}}},"status":{"active":[]}}),
        ),
        node: parse(
            json!({"apiVersion":"v1","kind":"Node","metadata":{"name":"worker-1","uid":"node-1","creationTimestamp":BASE_TIME},"status":{"conditions":[{"type":"Ready","status":"True","lastHeartbeatTime":BASE_TIME,"lastTransitionTime":BASE_TIME,"reason":"KubeletReady","message":"ready"}]}}),
        ),
        storage_class: parse(
            json!({"apiVersion":"storage.k8s.io/v1","kind":"StorageClass","metadata":{"name":"fast","uid":"sc-1","creationTimestamp":BASE_TIME},"provisioner":"example.csi","reclaimPolicy":"Delete","volumeBindingMode":"WaitForFirstConsumer"}),
        ),
        volume: parse(
            json!({"apiVersion":"v1","kind":"PersistentVolume","metadata":{"name":"ledger-volume","uid":"pv-1","creationTimestamp":BASE_TIME},"spec":{"capacity":{"storage":"10Gi"},"accessModes":["ReadWriteOnce"],"persistentVolumeReclaimPolicy":"Delete","storageClassName":"fast","volumeMode":"Filesystem"},"status":{"phase":"Bound"}}),
        ),
        crd: crd_definition_fixture(1).pop().expect("one CRD"),
    }
}

pub fn typed_summaries_at(
    fixture: &ResourceSummaryFixture,
    now: DateTime<Utc>,
) -> Vec<ResourceSummary> {
    let mut rows = fixture
        .pods
        .iter()
        .map(|pod| resources::pod_resource_summary_at(CLUSTER, pod, now))
        .collect::<Vec<_>>();
    rows.extend([
        resources::service_resource_summary_at(CLUSTER, &fixture.service, now),
        resources::config_map_resource_summary_at(CLUSTER, &fixture.config_map, now),
        resources::secret_resource_summary_at(CLUSTER, &fixture.secret, now),
        resources::persistent_volume_claim_resource_summary_at(CLUSTER, &fixture.claim, now),
        resources::deployment_resource_summary_at(CLUSTER, &fixture.deployment, now),
        resources::replicaset_resource_summary_at(CLUSTER, &fixture.replica_set, now),
        resources::statefulset_resource_summary_at(CLUSTER, &fixture.stateful_set, now),
        resources::daemonset_resource_summary_at(CLUSTER, &fixture.daemon_set, now),
        resources::ingress_resource_summary_at(CLUSTER, &fixture.ingress, now),
        resources::job_resource_summary_at(CLUSTER, &fixture.job, now),
        resources::cronjob_resource_summary_at(CLUSTER, &fixture.cron_job, now),
        resources::node_resource_summary_at(CLUSTER, &fixture.node, now),
        resources::storage_class_resource_summary_at(CLUSTER, &fixture.storage_class, now),
        resources::persistent_volume_resource_summary_at(CLUSTER, &fixture.volume, now),
        resources::custom_resource_definition_resource_summary_at(CLUSTER, &fixture.crd, now),
    ]);
    rows
}

pub fn dynamic_summary_fixture(count: usize) -> DynamicSummaryFixture {
    let group = "kustomize.toolkit.fluxcd.io";
    let kind = DiscoveredResourceKind {
        group: group.to_string(),
        version: "v1".to_string(),
        api_version: format!("{group}/v1"),
        kind: "Kustomization".to_string(),
        plural: "kustomizations".to_string(),
        short_names: vec!["ks".to_string()],
        namespaced: true,
    };
    let objects = (0..count)
        .map(|index| {
            let status = match index % 5 {
                0 => json!({
                    "phase":"Reconciled",
                    "conditions":[
                        {"type":"Ready","status":"False","reason":"ApplyFailed"},
                        {"type":"Stalled","status":"True","reason":"InvalidSpec"}
                    ]
                }),
                1 => json!({
                    "conditions":[
                        {"type":"Ready","status":7},
                        {"type":"Ready","status":"True"},
                        {"type":"Stalled","status":"True"}
                    ]
                }),
                2 => json!({
                    "conditions":[
                        {"type":"Reconciling","status":"True"},
                        {"type":"Ready","status":"False"}
                    ]
                }),
                3 => json!({
                    "conditions":[
                        {"type":"Ready","status":"True"},
                        {"type":"Ready","status":"False"}
                    ]
                }),
                _ => json!({
                    "conditions":[null, {}, {"type":"Ready","status":7}]
                }),
            };
            parse(json!({
                "apiVersion":"kustomize.toolkit.fluxcd.io/v1","kind":"Kustomization",
                "metadata":metadata(format!("ledger-{index}"), index),
                "spec":{"path":"./clusters/prod","prune":true},
                "status":status
            }))
        })
        .collect();
    DynamicSummaryFixture { kind, objects }
}

pub fn dynamic_summaries_at(
    fixture: &DynamicSummaryFixture,
    now: DateTime<Utc>,
) -> Vec<ResourceSummary> {
    resources::dynamic_resource_summaries_at(CLUSTER, &fixture.kind, &fixture.objects, now)
}

pub fn dynamic_edge_fixtures() -> Vec<DynamicSummaryFixture> {
    let argo_kind = DiscoveredResourceKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        api_version: "argoproj.io/v1alpha1".to_string(),
        kind: "Application".to_string(),
        plural: "applications".to_string(),
        short_names: vec!["app".to_string()],
        namespaced: true,
    };
    let argo_application = parse(json!({
        "apiVersion":"argoproj.io/v1alpha1","kind":"Application",
        "metadata":{"name":"checkout-app","namespace":"argocd","uid":"argo-checkout","creationTimestamp":BASE_TIME,"annotations":{"argocd.argoproj.io/name":"checkout-app"}},
        "spec":{"destination":{"namespace":"checkout","server":"https://kubernetes.default.svc"}},
        "status":{"health":{"status":"Degraded"},"sync":{"status":"OutOfSync"}}
    }));
    let cluster_kind = DiscoveredResourceKind {
        group: "widgets.example.io".to_string(),
        version: "v1".to_string(),
        api_version: "widgets.example.io/v1".to_string(),
        kind: "Widget".to_string(),
        plural: "widgets".to_string(),
        short_names: vec!["wdg".to_string()],
        namespaced: false,
    };
    let cluster_widget = parse(json!({
        "apiVersion":"widgets.example.io/v1","kind":"Widget",
        "metadata":{"name":"cluster-widget","uid":"widget-cluster","creationTimestamp":BASE_TIME},
        "spec":{"enabled":true},
        "status":{"phase":"Unknown","conditions":[
            {"type":"Ready","status":"True"},
            {"type":"Ready","status":7},
            {}
        ]}
    }));
    vec![
        DynamicSummaryFixture {
            kind: argo_kind,
            objects: vec![argo_application],
        },
        DynamicSummaryFixture {
            kind: cluster_kind,
            objects: vec![cluster_widget],
        },
    ]
}

pub fn dynamic_edge_summaries_at(
    fixtures: &[DynamicSummaryFixture],
    now: DateTime<Utc>,
) -> Vec<ResourceSummary> {
    let mut summaries = Vec::new();
    for fixture in fixtures {
        summaries.extend(dynamic_summaries_at(fixture, now));
    }
    summaries
}

pub fn namespace_summary_fixture(count: usize) -> Vec<k8s_openapi::api::core::v1::Namespace> {
    (0..count)
        .map(|index| {
            parse(json!({
                "apiVersion":"v1","kind":"Namespace",
                "metadata":{"name":format!("tenant-{index}"),"creationTimestamp":if index == 99 {Value::Null} else {Value::String(BASE_TIME.to_string())},"uid":format!("namespace-{index}")},
                "status":{"phase":"Active"}
            }))
        })
        .collect()
}

pub fn namespace_summaries_at(
    namespaces: &[k8s_openapi::api::core::v1::Namespace],
    now: DateTime<Utc>,
) -> Vec<NamespaceSummary> {
    namespaces
        .iter()
        .map(|namespace| namespaces::namespace_summary_from_at(namespace, now))
        .collect()
}

pub fn crd_definition_fixture(count: usize) -> Vec<CustomResourceDefinition> {
    let mut definitions = Vec::with_capacity(count + count / 10);
    for index in 0..count {
        let plural = format!("managedthings{index}");
        let group = format!("operator-{}.example.com", index % 40);
        let value = json!({
            "apiVersion":"apiextensions.k8s.io/v1","kind":"CustomResourceDefinition",
            "metadata":{"name":format!("{plural}.{group}"),"creationTimestamp":if index + 1 == count {Value::Null} else {Value::String(BASE_TIME.to_string())}},
            "spec":{"group":group,"scope":if index.is_multiple_of(7) {"Cluster"} else {"Namespaced"},
                "names":{"plural":plural,"singular":format!("managedthing{index}"),"kind":format!("ManagedThing{index}"),"listKind":format!("ManagedThing{index}List"),"shortNames":[format!("mt{index}")]},
                "versions":[
                    {"name":"v1beta1","served":true,"storage":false,"schema":{"openAPIV3Schema":{"type":"object"}}},
                    {"name":"v1","served":true,"storage":true,"schema":{"openAPIV3Schema":{"type":"object"}}}
                ]
            }
        });
        let crd: CustomResourceDefinition = parse(value);
        if index.is_multiple_of(10) {
            definitions.push(crd.clone());
        }
        definitions.push(crd);
    }
    definitions
}

pub fn crd_resource_catalog(
    definitions: Vec<CustomResourceDefinition>,
) -> Vec<DiscoveredResourceKind> {
    discovery::crd_resource_kinds_from(definitions)
}
