use super::{fixed_now, summaries};
use crate::{commands::resources, models::ResourceTopology};
use k8s_openapi::api::{
    apps::v1::{Deployment, ReplicaSet, StatefulSet},
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, Node, PersistentVolumeClaim, Pod, Service},
    discovery::v1::EndpointSlice,
    networking::v1::Ingress,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

const CLUSTER: &str = "catalog-cluster";
const TIME: &str = "2026-09-30T12:00:00Z";

#[derive(Clone)]
pub struct OwnershipTopologyFixture {
    deployments: Vec<Deployment>,
    replica_sets: Vec<ReplicaSet>,
    pods: Vec<Pod>,
    services: Vec<Service>,
    stateful_sets: Vec<StatefulSet>,
    cron_jobs: Vec<CronJob>,
    jobs: Vec<Job>,
    claims: Vec<PersistentVolumeClaim>,
    nodes: Vec<Node>,
    noise: Vec<ConfigMap>,
    custom_resources: summaries::DynamicSummaryFixture,
    index: crate::commands::helpers::FluxOwnershipIndex,
}

#[derive(Clone)]
pub struct NetworkTopologyFixture {
    pub pods: Vec<Pod>,
    pub services: Vec<Service>,
    pub ingresses: Vec<Ingress>,
    pub slices: Vec<EndpointSlice>,
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("valid topology benchmark object")
}

pub fn ownership_topology_fixture() -> OwnershipTopologyFixture {
    let mut deployments = Vec::with_capacity(500);
    let mut replica_sets = Vec::with_capacity(500);
    let mut pods = Vec::with_capacity(502);
    let mut services = Vec::with_capacity(500);
    for index in 0..500 {
        let namespace = format!("namespace-{}", index % 100);
        let app = format!("checkout-{index}");
        let deployment: Deployment = parse(json!({
            "apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":app,"namespace":namespace,"uid":format!("deployment-{index}"),"creationTimestamp":TIME,"labels":{"app.kubernetes.io/name":app},"annotations":{"deployment.kubernetes.io/revision":format!("{}", index % 9 + 1)}},
            "spec":{"replicas":1,"selector":{"matchLabels":{"app.kubernetes.io/name":app}},"template":{"metadata":{"labels":{"app.kubernetes.io/name":app}},"spec":{"containers":[{"name":"web","image":"ghcr.io/example/checkout:1.0.0","ports":[{"name":"http","containerPort":8080}]}]}}},
            "status":{"replicas":1,"readyReplicas":1,"availableReplicas":1}
        }));
        let rs_name = format!("checkout-{index}-rs");
        let replica_set: ReplicaSet = parse(json!({
            "apiVersion":"apps/v1","kind":"ReplicaSet",
            "metadata":{"name":rs_name,"namespace":namespace,"uid":format!("replicaset-{index}"),"creationTimestamp":TIME,
                "labels":{"app.kubernetes.io/name":app},"ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":app,"uid":format!("deployment-{index}"),"controller":true}]},
            "spec":{"replicas":1,"selector":{"matchLabels":{"app.kubernetes.io/name":app}},"template":{"metadata":{"labels":{"app.kubernetes.io/name":app}},"spec":{"containers":[{"name":"web","image":"ghcr.io/example/checkout:1.0.0"}]}}},
            "status":{"replicas":1,"readyReplicas":1,"availableReplicas":1}
        }));
        let pod: Pod = parse(json!({
            "apiVersion":"v1","kind":"Pod",
            "metadata":{"name":format!("checkout-{index}-pod"),"namespace":namespace,"uid":format!("pod-{index}"),"creationTimestamp":TIME,
                "labels":{"app.kubernetes.io/name":app},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":rs_name,"uid":format!("replicaset-{index}"),"controller":true}]},
            "spec":{"containers":[{"name":"web","image":"ghcr.io/example/checkout:1.0.0","ports":[{"name":"http","containerPort":8080}]}]},
            "status":{"phase":"Running","conditions":[{"type":"Ready","status":"True"}],"containerStatuses":[{"name":"web","ready":true,"restartCount":0,"image":"ghcr.io/example/checkout:1.0.0","imageID":"containerd://image","containerID":format!("containerd://{index}"),"state":{"running":{"startedAt":TIME}}}]}
        }));
        let service: Service = parse(json!({
            "apiVersion":"v1","kind":"Service","metadata":{"name":format!("{app}-service"),"namespace":namespace,"uid":format!("service-{index}"),"creationTimestamp":TIME},
            "spec":{"type":"ClusterIP","selector":{"app.kubernetes.io/name":app},"ports":[{"name":"http","port":8080,"targetPort":8080}]}
        }));
        deployments.push(deployment);
        replica_sets.push(replica_set);
        pods.push(pod);
        services.push(service);
    }
    let mut namespace_collision_replica_set = replica_sets[0].clone();
    namespace_collision_replica_set.metadata.namespace = Some("namespace-collision".to_string());
    namespace_collision_replica_set.metadata.uid = Some("replicaset-decoy".to_string());
    namespace_collision_replica_set.metadata.owner_references = None;
    replica_sets.push(namespace_collision_replica_set);

    let stateful_set: StatefulSet = parse(json!({
        "apiVersion":"apps/v1","kind":"StatefulSet",
        "metadata":{"name":"ledger","namespace":"tenant-ledger","uid":"statefulset-ledger","creationTimestamp":TIME,"labels":{"app.kubernetes.io/name":"ledger"}},
        "spec":{"serviceName":"ledger","replicas":1,"selector":{"matchLabels":{"app.kubernetes.io/name":"ledger"}},"template":{"metadata":{"labels":{"app.kubernetes.io/name":"ledger"}},"spec":{"containers":[{"name":"ledger","image":"busybox:1.36.1","volumeMounts":[{"name":"data","mountPath":"/data"}]}]}},"volumeClaimTemplates":[{"metadata":{"name":"data"},"spec":{"accessModes":["ReadWriteOnce"],"storageClassName":"local-path","resources":{"requests":{"storage":"64Mi"}}}}]},
        "status":{"replicas":1,"readyReplicas":1}
    }));
    let stateful_pod: Pod = parse(json!({
        "apiVersion":"v1","kind":"Pod",
        "metadata":{"name":"ledger-0","namespace":"tenant-ledger","uid":"pod-ledger-0","creationTimestamp":TIME,"labels":{"app.kubernetes.io/name":"ledger"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"StatefulSet","name":"ledger","uid":"statefulset-ledger","controller":true}]},
        "spec":{"containers":[{"name":"ledger","image":"busybox:1.36.1","volumeMounts":[{"name":"data","mountPath":"/data"}]}]},
        "status":{"phase":"Running","conditions":[{"type":"Ready","status":"True"}],"containerStatuses":[{"name":"ledger","ready":true,"restartCount":0,"image":"busybox:1.36.1","imageID":"containerd://image","containerID":"containerd://ledger","state":{"running":{"startedAt":TIME}}}]}
    }));
    let claim: PersistentVolumeClaim = parse(json!({
        "apiVersion":"v1","kind":"PersistentVolumeClaim",
        "metadata":{"name":"data-ledger-0","namespace":"tenant-ledger","uid":"pvc-ledger-0","creationTimestamp":TIME},
        "spec":{"accessModes":["ReadWriteOnce"],"storageClassName":"local-path","resources":{"requests":{"storage":"64Mi"}}},"status":{"phase":"Bound"}
    }));
    let mut namespace_collision_stateful_set = stateful_set.clone();
    namespace_collision_stateful_set.metadata.namespace = Some("tenant-shadow".to_string());
    namespace_collision_stateful_set.metadata.uid = Some("statefulset-ledger-shadow".to_string());
    let mut namespace_collision_pod = stateful_pod.clone();
    namespace_collision_pod.metadata.namespace = Some("tenant-shadow".to_string());
    namespace_collision_pod.metadata.uid = Some("pod-ledger-shadow".to_string());
    namespace_collision_pod
        .metadata
        .owner_references
        .as_mut()
        .and_then(|owners| owners.first_mut())
        .expect("stateful Pod owner fixture")
        .uid = "statefulset-ledger-shadow".to_string();
    let mut namespace_collision_claim = claim.clone();
    namespace_collision_claim.metadata.namespace = Some("tenant-shadow".to_string());
    namespace_collision_claim.metadata.uid = Some("pvc-ledger-shadow".to_string());
    let cron_job: CronJob = parse(json!({
        "apiVersion":"batch/v1","kind":"CronJob",
        "metadata":{"name":"billing-reconcile","namespace":"tenant-batch","uid":"cronjob-billing","creationTimestamp":TIME},
        "spec":{"schedule":"*/5 * * * *","jobTemplate":{"spec":{"template":{"spec":{"restartPolicy":"Never","containers":[{"name":"reconcile","image":"ghcr.io/example/reconcile:1.0.0"}]}}}}},
        "status":{"lastScheduleTime":TIME,"lastSuccessfulTime":TIME}
    }));
    let job: Job = parse(json!({
        "apiVersion":"batch/v1","kind":"Job",
        "metadata":{"name":"billing-reconcile-20261001","namespace":"tenant-batch","uid":"job-billing","creationTimestamp":TIME,"ownerReferences":[{"apiVersion":"batch/v1","kind":"CronJob","name":"billing-reconcile","uid":"cronjob-billing","controller":true}]},
        "spec":{"template":{"spec":{"restartPolicy":"Never","containers":[{"name":"reconcile","image":"ghcr.io/example/reconcile:1.0.0"}]}}},
        "status":{"succeeded":1,"completionTime":TIME}
    }));
    let job_pod: Pod = parse(json!({
        "apiVersion":"v1","kind":"Pod",
        "metadata":{"name":"billing-reconcile-20261001-pod","namespace":"tenant-batch","uid":"pod-billing","creationTimestamp":TIME,"ownerReferences":[{"apiVersion":"batch/v1","kind":"Job","name":"billing-reconcile-20261001","uid":"job-billing","controller":true}]},
        "spec":{"containers":[{"name":"reconcile","image":"ghcr.io/example/reconcile:1.0.0"}]},"status":{"phase":"Succeeded"}
    }));
    let node: Node = parse(json!({
        "apiVersion":"v1","kind":"Node",
        "metadata":{"name":"worker-cluster-1","uid":"node-cluster-1","creationTimestamp":TIME,"labels":{"kubernetes.io/hostname":"worker-cluster-1"}},
        "status":{"conditions":[{"type":"Ready","status":"True","lastHeartbeatTime":TIME,"lastTransitionTime":TIME,"reason":"KubeletReady","message":"ready"}]}
    }));
    let noise: ConfigMap = parse(json!({
        "apiVersion":"v1","kind":"ConfigMap",
        "metadata":{"name":"kube-root-ca.crt","namespace":"kube-system","uid":"root-ca-noise","creationTimestamp":TIME},
        "data":{"ca.crt":"fixture"}
    }));
    // These cases exercise a same-name namespace collision and a missing owner UID.
    let collision: Pod = parse(json!({
        "apiVersion":"v1","kind":"Pod","metadata":{"name":"checkout-0-pod","namespace":"namespace-collision","uid":"pod-collision","creationTimestamp":TIME,"labels":{"app.kubernetes.io/name":"checkout-0"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"checkout-0-rs","uid":"missing-replicaset","controller":true}]},
        "spec":{"containers":[{"name":"web","image":"ghcr.io/example/checkout:1.0.0","ports":[{"name":"http","containerPort":8080}]}]},"status":{"phase":"Pending"}
    }));
    pods.push(stateful_pod.clone());
    pods.push(stateful_pod);
    pods.push(namespace_collision_pod);
    pods.push(job_pod);
    pods.push(collision);

    OwnershipTopologyFixture {
        deployments,
        replica_sets,
        pods,
        services,
        stateful_sets: vec![stateful_set, namespace_collision_stateful_set],
        cron_jobs: vec![cron_job],
        jobs: vec![job],
        claims: vec![claim, namespace_collision_claim],
        nodes: vec![node],
        noise: vec![noise],
        custom_resources: summaries::dynamic_summary_fixture(500),
        index: super::planning::build_flux_index_inner(super::planning::flux_index_fixture()),
    }
}

pub fn build_ownership_topology(fixture: OwnershipTopologyFixture) -> ResourceTopology {
    let now = fixed_now();
    let OwnershipTopologyFixture {
        deployments,
        replica_sets,
        pods,
        services,
        stateful_sets,
        cron_jobs,
        jobs,
        claims,
        nodes,
        noise,
        custom_resources,
        index,
    } = fixture;
    let mut inputs = Vec::with_capacity(2_509);
    for deploy in deployments {
        let mut input = resources::input_from_metadata_at(
            CLUSTER,
            "Deployment",
            "apps/v1",
            &deploy.metadata,
            now,
        );
        input.summary = resources::deployment_resource_summary_at(CLUSTER, &deploy, now);
        inputs.push(input);
    }
    for rs in replica_sets {
        let mut input =
            resources::input_from_metadata_at(CLUSTER, "ReplicaSet", "apps/v1", &rs.metadata, now);
        input.summary = resources::replicaset_resource_summary_at(CLUSTER, &rs, now);
        inputs.push(input);
    }
    for stateful_set in stateful_sets {
        let mut input = resources::input_from_metadata_at(
            CLUSTER,
            "StatefulSet",
            "apps/v1",
            &stateful_set.metadata,
            now,
        );
        input.summary = resources::statefulset_resource_summary_at(CLUSTER, &stateful_set, now);
        inputs.push(input);
    }
    for cron_job in cron_jobs {
        let mut input = resources::input_from_metadata_at(
            CLUSTER,
            "CronJob",
            "batch/v1",
            &cron_job.metadata,
            now,
        );
        input.summary = resources::cronjob_resource_summary_at(CLUSTER, &cron_job, now);
        inputs.push(input);
    }
    for job in jobs {
        let mut input =
            resources::input_from_metadata_at(CLUSTER, "Job", "batch/v1", &job.metadata, now);
        input.summary = resources::job_resource_summary_at(CLUSTER, &job, now);
        inputs.push(input);
    }
    for node in nodes {
        let mut input =
            resources::input_from_metadata_at(CLUSTER, "Node", "v1", &node.metadata, now);
        input.summary = resources::node_resource_summary_at(CLUSTER, &node, now);
        inputs.push(input);
    }
    for pod in pods {
        inputs.push(resources::pod_input_at(CLUSTER, pod, now));
    }
    for service in services {
        let mut input =
            resources::input_from_metadata_at(CLUSTER, "Service", "v1", &service.metadata, now);
        input.summary = resources::service_resource_summary_at(CLUSTER, &service, now);
        if let Some(spec) = &service.spec {
            input.port_hints = spec
                .ports
                .iter()
                .flatten()
                .map(|port| match port.name.as_deref() {
                    Some(name) if !name.is_empty() => format!("{name}:{}", port.port),
                    _ => port.port.to_string(),
                })
                .collect();
            input.port_hints.sort();
            input.port_hints.dedup();
        }
        inputs.push(input);
    }
    inputs.extend(resources::inputs_from_metadata_at(CLUSTER, claims, now));
    inputs.extend(resources::inputs_from_metadata_at(CLUSTER, noise, now));
    let summaries::DynamicSummaryFixture { kind, objects } = custom_resources;
    inputs.extend(
        objects
            .into_iter()
            .map(|object| resources::dynamic_topology_input_at(CLUSTER, &kind, &object, now)),
    );
    let inputs = resources::topology_inputs_with_flux_ownership(inputs, &index, &[]);
    resources::build_resource_topology(inputs)
}

pub fn topology_fixture() -> NetworkTopologyFixture {
    let summary = summaries::resource_summary_fixture(500);
    let mut pods = summary.pods;
    let mut services = Vec::with_capacity(500);
    let mut slices = Vec::with_capacity(250);
    for index in 0..500 {
        let namespace = format!("tenant-{}", index % 100);
        let name = format!("ledger-{index}-service");
        let service: Service = parse(json!({
            "apiVersion":"v1","kind":"Service",
            "metadata":{"name":name,"namespace":namespace,"uid":format!("service-{index}"),"creationTimestamp":TIME},
            "spec":{"type":"ClusterIP","selector":{"app.kubernetes.io/name":"ledger"},"ports":[{"name":"http","port":8080,"targetPort":8080}]}
        }));
        if index < 250 {
            let pod_name = format!("ledger-{index}");
            let endpoints = if index == 0 {
                json!([
                    {"addresses":["10.0.0.2"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":"missing-pod"}}
                ])
            } else {
                json!([
                    {"addresses":["10.0.0.1"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":pod_name}},
                    {"addresses":["10.0.0.1"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":pod_name}},
                    {"addresses":["10.0.0.1"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":pod_name}}
                ])
            };
            slices.push(parse(json!({
                "apiVersion":"discovery.k8s.io/v1","kind":"EndpointSlice",
                "metadata":{"name":format!("{name}-slice"),"namespace":namespace,"uid":format!("slice-{index}"),"creationTimestamp":TIME,"labels":{"kubernetes.io/service-name":name}},
                "addressType":"IPv4","ports":[{"name":"http","port":8080}],
                "endpoints":endpoints
            })));
        }
        services.push(service);
    }
    slices.push(parse(json!({
        "apiVersion":"discovery.k8s.io/v1","kind":"EndpointSlice",
        "metadata":{"name":"unlabelled-slice","namespace":"tenant-special","uid":"slice-unlabelled","creationTimestamp":TIME},
        "addressType":"IPv4","endpoints":[{"addresses":["10.0.0.9"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":"missing-pod"}}]
    })));
    services.push(parse(json!({
        "apiVersion":"v1","kind":"Service",
        "metadata":{"name":"external-database","namespace":"tenant-special","uid":"service-external","creationTimestamp":TIME},
        "spec":{"type":"ExternalName","externalName":"database.example.invalid","ports":[{"name":"postgres","port":5432,"targetPort":5432}]}
    })));
    let mut namespace_collision_pod = pods
        .iter()
        .find(|pod| pod.metadata.name.as_deref() == Some("ledger-1"))
        .expect("ledger-1 Pod fixture")
        .clone();
    namespace_collision_pod.metadata.namespace = Some("tenant-2".to_string());
    namespace_collision_pod.metadata.uid = Some("pod-ledger-1-tenant-2".to_string());
    pods.push(namespace_collision_pod);
    services.push(parse(json!({
        "apiVersion":"v1","kind":"Service",
        "metadata":{"name":"ledger-0-service","namespace":"tenant-1","uid":"service-ledger-0-tenant-1","creationTimestamp":TIME},
        "spec":{"type":"ClusterIP","selector":{"app.kubernetes.io/name":"ledger"},"ports":[{"name":"http","port":8080,"targetPort":8080}]}
    })));
    slices.push(parse(json!({
        "apiVersion":"discovery.k8s.io/v1","kind":"EndpointSlice",
        "metadata":{"name":"ledger-0-service-collision-slice","namespace":"tenant-1","uid":"slice-ledger-0-tenant-1","creationTimestamp":TIME,"labels":{"kubernetes.io/service-name":"ledger-0-service"}},
        "addressType":"IPv4","ports":[{"name":"http","port":8080}],
        "endpoints":[{"addresses":["10.0.1.1"],"targetRef":{"apiVersion":"v1","kind":"Pod","name":"ledger-1"}}]
    })));
    let ingress: Ingress = parse(json!({
        "apiVersion":"networking.k8s.io/v1","kind":"Ingress",
        "metadata":{"name":"ledger-public","namespace":"tenant-0","uid":"ingress-ledger","creationTimestamp":TIME},
        "spec":{"rules":[{"host":"ledger.example.com","http":{"paths":[{"path":"/","pathType":"Prefix","backend":{"service":{"name":"ledger-0-service","port":{"number":8080}}}},{"path":"/missing","pathType":"Prefix","backend":{"service":{"name":"missing-service","port":{"number":80}}}}]}}]}
    }));
    NetworkTopologyFixture {
        pods,
        services,
        ingresses: vec![ingress],
        slices,
    }
}

pub fn build_network_topology(fixture: NetworkTopologyFixture) -> ResourceTopology {
    let NetworkTopologyFixture {
        pods,
        services,
        ingresses,
        slices,
    } = fixture;
    resources::network_topology_from_objects_at(
        CLUSTER,
        pods,
        services,
        ingresses,
        slices,
        Vec::new(),
        fixed_now(),
    )
}
