use crate::commands::{helpers::list_params, kubeconfig::KubeconfigSource};
use crate::models::{
    AppError, ResourceMetricSummary, ResourceMetricsAvailability,
    ResourceMetricsAvailabilityStatus, ResourceMetricsSummary,
};
use futures_util::{stream, StreamExt};
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ApiResource, DynamicObject},
    discovery::{verbs, Discovery},
    Client, Error,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsListStatus {
    Forbidden,
    Unavailable,
}

#[derive(Debug, Default)]
struct MetricsObjectList {
    objects: Vec<DynamicObject>,
    failures: Vec<MetricsListStatus>,
}

const MAX_NAMESPACE_CONCURRENCY: usize = 16;

fn metrics_api_resource(kind: &str, plural: &str) -> ApiResource {
    ApiResource {
        group: "metrics.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "metrics.k8s.io/v1beta1".to_string(),
        kind: kind.to_string(),
        plural: plural.to_string(),
    }
}

async fn client_for_context(
    cluster_context: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<Client, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    source.client_for_context(cluster_context).await
}

async fn has_metrics_api(client: Client) -> Result<bool, AppError> {
    let discovery = Discovery::new(client)
        .run_aggregated()
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    let available = discovery.groups().any(|group| {
        group.name() == "metrics.k8s.io"
            && group
                .recommended_resources()
                .iter()
                .any(|(resource, capabilities)| {
                    resource.api_version == "metrics.k8s.io/v1beta1"
                        && matches!(resource.kind.as_str(), "PodMetrics" | "NodeMetrics")
                        && capabilities.supports_operation(verbs::LIST)
                })
    });
    Ok(available)
}

fn classify_metrics_error(error: &Error) -> MetricsListStatus {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("forbidden") || message.contains("403") {
        MetricsListStatus::Forbidden
    } else {
        MetricsListStatus::Unavailable
    }
}

fn parse_cpu_millicores(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(raw) = trimmed.strip_suffix('n') {
        return raw.parse::<f64>().ok().map(|n| n / 1_000_000.0);
    }
    if let Some(raw) = trimmed.strip_suffix('u') {
        return raw.parse::<f64>().ok().map(|u| u / 1_000.0);
    }
    if let Some(raw) = trimmed.strip_suffix('m') {
        return raw.parse::<f64>().ok();
    }
    trimmed.parse::<f64>().ok().map(|cores| cores * 1_000.0)
}

fn parse_memory_bytes(value: &str) -> Option<i64> {
    const UNITS: &[(&str, i64)] = &[
        ("Ki", 1024),
        ("Mi", 1024 * 1024),
        ("Gi", 1024 * 1024 * 1024),
        ("Ti", 1024_i64.pow(4)),
        ("K", 1000),
        ("M", 1000 * 1000),
        ("G", 1000 * 1000 * 1000),
        ("T", 1000_i64.pow(4)),
    ];
    let trimmed = value.trim();
    for (suffix, multiplier) in UNITS {
        if let Some(raw) = trimmed.strip_suffix(suffix) {
            return raw
                .parse::<f64>()
                .ok()
                .map(|number| (number * *multiplier as f64).round() as i64);
        }
    }
    trimmed.parse::<i64>().ok()
}

fn usage_value(data: &Value, key: &str) -> Option<String> {
    data.get("usage")
        .and_then(|usage| usage.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn container_usage_totals(data: &Value) -> (Option<f64>, Option<i64>) {
    let containers = data
        .get("containers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut cpu = 0.0;
    let mut has_cpu = false;
    let mut memory = 0_i64;
    let mut has_memory = false;

    for container in containers {
        if let Some(value) =
            usage_value(container, "cpu").and_then(|value| parse_cpu_millicores(&value))
        {
            cpu += value;
            has_cpu = true;
        }
        if let Some(value) =
            usage_value(container, "memory").and_then(|value| parse_memory_bytes(&value))
        {
            memory += value;
            has_memory = true;
        }
    }

    (has_cpu.then_some(cpu), has_memory.then_some(memory))
}

fn metric_from_object(
    cluster_context: &str,
    kind: &str,
    object: &DynamicObject,
) -> ResourceMetricSummary {
    let (cpu_millicores, memory_bytes) = if kind == "Pod" {
        container_usage_totals(&object.data)
    } else {
        (
            usage_value(&object.data, "cpu").and_then(|value| parse_cpu_millicores(&value)),
            usage_value(&object.data, "memory").and_then(|value| parse_memory_bytes(&value)),
        )
    };

    ResourceMetricSummary {
        kind: kind.to_string(),
        cluster: cluster_context.to_string(),
        name: object.metadata.name.clone().unwrap_or_default(),
        namespace: object.metadata.namespace.clone(),
        cpu_millicores,
        memory_bytes,
        sampled_at: object
            .data
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string),
        source_pods: Vec::new(),
    }
}

async fn list_pod_metric_objects(client: Client, namespaces: &[String]) -> MetricsObjectList {
    let resource = metrics_api_resource("PodMetrics", "pods");
    let mut result = MetricsObjectList::default();
    if namespaces.is_empty() {
        let api: Api<DynamicObject> = Api::all_with(client, &resource);
        match api.list(&list_params()).await {
            Ok(list) => result.objects = list.items,
            Err(error) => result.failures.push(classify_metrics_error(&error)),
        }
        return result;
    }
    let outcomes = stream::iter(namespaces.to_vec())
        .map(|namespace| {
            let api: Api<DynamicObject> =
                Api::namespaced_with(client.clone(), &namespace, &resource);
            async move { api.list(&list_params()).await }
        })
        .buffered(MAX_NAMESPACE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for outcome in outcomes {
        match outcome {
            Ok(list) => result.objects.extend(list.items),
            Err(error) => result.failures.push(classify_metrics_error(&error)),
        }
    }
    result
}

async fn list_node_metric_objects(client: Client) -> Result<Vec<DynamicObject>, MetricsListStatus> {
    let resource = metrics_api_resource("NodeMetrics", "nodes");
    let api: Api<DynamicObject> = Api::all_with(client, &resource);
    api.list(&list_params())
        .await
        .map(|list| list.items)
        .map_err(|error| classify_metrics_error(&error))
}

async fn list_pods(client: Client, namespaces: &[String]) -> Result<Vec<Pod>, AppError> {
    let mut out = Vec::new();
    if namespaces.is_empty() {
        let api: Api<Pod> = Api::all(client);
        return api
            .list(&list_params())
            .await
            .map(|list| list.items)
            .map_err(|e| AppError::kube(e.to_string()));
    }
    let outcomes = stream::iter(namespaces.to_vec())
        .map(|namespace| {
            let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);
            async move { api.list(&list_params()).await }
        })
        .buffered(MAX_NAMESPACE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for outcome in outcomes {
        out.extend(
            outcome
                .map_err(|e| AppError::kube(e.to_string()))?
                .items,
        );
    }
    Ok(out)
}

async fn list_replicasets(
    client: Client,
    namespaces: &[String],
) -> Result<Vec<ReplicaSet>, AppError> {
    let mut out = Vec::new();
    if namespaces.is_empty() {
        let api: Api<ReplicaSet> = Api::all(client);
        return api
            .list(&list_params())
            .await
            .map(|list| list.items)
            .map_err(|e| AppError::kube(e.to_string()));
    }
    let outcomes = stream::iter(namespaces.to_vec())
        .map(|namespace| {
            let api: Api<ReplicaSet> = Api::namespaced(client.clone(), &namespace);
            async move { api.list(&list_params()).await }
        })
        .buffered(MAX_NAMESPACE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for outcome in outcomes {
        out.extend(
            outcome
                .map_err(|e| AppError::kube(e.to_string()))?
                .items,
        );
    }
    Ok(out)
}

type WorkloadKey = (String, String, String);

fn controller_owner(
    owners: &[k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference],
) -> Option<&k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference> {
    owners
        .iter()
        .find(|owner| owner.controller == Some(true))
        .or_else(|| owners.first())
}

fn add_workload_metric(
    by_workload: &mut BTreeMap<WorkloadKey, ResourceMetricSummary>,
    cluster_context: &str,
    key: WorkloadKey,
    pod_name: &str,
    metric: &ResourceMetricSummary,
) {
    let (kind, namespace, name) = key.clone();
    let entry = by_workload
        .entry(key)
        .or_insert_with(|| ResourceMetricSummary {
            kind,
            cluster: cluster_context.to_string(),
            name,
            namespace: Some(namespace),
            cpu_millicores: None,
            memory_bytes: None,
            sampled_at: metric.sampled_at.clone(),
            source_pods: Vec::new(),
        });
    if let Some(cpu) = metric.cpu_millicores {
        entry.cpu_millicores = Some(entry.cpu_millicores.unwrap_or(0.0) + cpu);
    }
    if let Some(memory) = metric.memory_bytes {
        entry.memory_bytes = Some(entry.memory_bytes.unwrap_or(0) + memory);
    }
    if metric.sampled_at > entry.sampled_at {
        entry.sampled_at.clone_from(&metric.sampled_at);
    }
    entry.source_pods.push(pod_name.to_string());
}

fn aggregate_workload_metrics(
    cluster_context: &str,
    pods: &[Pod],
    replicasets: &[ReplicaSet],
    pod_metrics: &[ResourceMetricSummary],
) -> Vec<ResourceMetricSummary> {
    let metric_by_pod: BTreeMap<_, _> = pod_metrics
        .iter()
        .filter_map(|metric| Some(((metric.namespace.clone()?, metric.name.clone()), metric)))
        .collect();
    let replicaset_owner_by_key: BTreeMap<WorkloadKey, WorkloadKey> = replicasets
        .iter()
        .filter_map(|replicaset| {
            let namespace = replicaset.metadata.namespace.clone()?;
            let name = replicaset.metadata.name.clone()?;
            let owner = controller_owner(replicaset.metadata.owner_references.as_deref()?)?;
            Some((
                ("ReplicaSet".to_string(), namespace.clone(), name),
                (owner.kind.clone(), namespace, owner.name.clone()),
            ))
        })
        .collect();
    let mut by_workload: BTreeMap<WorkloadKey, ResourceMetricSummary> = BTreeMap::new();

    for pod in pods {
        let namespace = pod.metadata.namespace.clone().unwrap_or_default();
        let name = pod.metadata.name.clone().unwrap_or_default();
        let Some(metric) = metric_by_pod.get(&(namespace.clone(), name.clone())) else {
            continue;
        };
        let Some(owner) = pod.metadata.owner_references.as_ref().and_then(|owners| {
            owners
                .iter()
                .find(|owner| owner.controller == Some(true))
                .or_else(|| owners.first())
        }) else {
            continue;
        };
        let key = (owner.kind.clone(), namespace.clone(), owner.name.clone());
        add_workload_metric(
            &mut by_workload,
            cluster_context,
            key.clone(),
            &name,
            metric,
        );

        if let Some(parent_key) = replicaset_owner_by_key.get(&key) {
            add_workload_metric(
                &mut by_workload,
                cluster_context,
                parent_key.clone(),
                &name,
                metric,
            );
        }
    }

    by_workload.into_values().collect()
}

fn availability(
    status: ResourceMetricsAvailabilityStatus,
    message: impl Into<String>,
) -> ResourceMetricsAvailability {
    ResourceMetricsAvailability {
        status,
        message: Some(message.into()),
    }
}

pub async fn resource_metrics_from(
    cluster_context: String,
    namespaces: Vec<String>,
    kubeconfig_env_var: Option<String>,
) -> Result<ResourceMetricsSummary, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    match has_metrics_api(client.clone()).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok(ResourceMetricsSummary {
                cluster: cluster_context,
                availability: availability(
                    ResourceMetricsAvailabilityStatus::Unavailable,
                    "metrics API unavailable",
                ),
                pods: Vec::new(),
                nodes: Vec::new(),
                workloads: Vec::new(),
                warnings: Vec::new(),
            });
        }
        Err(err) => {
            let forbidden = err.message.to_ascii_lowercase().contains("forbidden")
                || err.message.contains("403");
            return Ok(ResourceMetricsSummary {
                cluster: cluster_context,
                availability: availability(
                    if forbidden {
                        ResourceMetricsAvailabilityStatus::Forbidden
                    } else {
                        ResourceMetricsAvailabilityStatus::Unavailable
                    },
                    if forbidden {
                        "metrics API forbidden"
                    } else {
                        "metrics API unavailable"
                    },
                ),
                pods: Vec::new(),
                nodes: Vec::new(),
                workloads: Vec::new(),
                warnings: vec![format!("Metrics discovery unavailable: {}", err.message)],
            });
        }
    }

    let pod_result = list_pod_metric_objects(client.clone(), &namespaces).await;
    let node_result = list_node_metric_objects(client.clone()).await;
    let mut warnings = Vec::new();
    let mut had_unavailable_error = false;

    if pod_result.failures.contains(&MetricsListStatus::Forbidden) {
        warnings.push("Pod metrics forbidden".to_string());
    }
    if pod_result
        .failures
        .contains(&MetricsListStatus::Unavailable)
    {
        had_unavailable_error = true;
        warnings.push("Pod metrics unavailable".to_string());
    }
    let pods: Vec<ResourceMetricSummary> = pod_result
        .objects
        .iter()
        .map(|object| metric_from_object(&cluster_context, "Pod", object))
        .collect();
    let nodes = match node_result {
        Ok(objects) => objects
            .iter()
            .map(|object| metric_from_object(&cluster_context, "Node", object))
            .collect(),
        Err(status) => {
            if status == MetricsListStatus::Forbidden {
                warnings.push("Node metrics forbidden".to_string());
            } else {
                had_unavailable_error = true;
                warnings.push("Node metrics unavailable".to_string());
            }
            Vec::new()
        }
    };
    let workloads = match list_pods(client.clone(), &namespaces).await {
        Ok(pod_objects) => {
            let replicasets = match list_replicasets(client, &namespaces).await {
                Ok(replicasets) => replicasets,
                Err(err) => {
                    warnings.push(format!(
                        "Workload owner rollup unavailable: {}",
                        err.message
                    ));
                    Vec::new()
                }
            };
            aggregate_workload_metrics(&cluster_context, &pod_objects, &replicasets, &pods)
        }
        Err(err) => {
            warnings.push(format!("Workload metrics unavailable: {}", err.message));
            Vec::new()
        }
    };
    let status = if pods.is_empty() && nodes.is_empty() {
        if warnings.iter().any(|warning| warning.contains("forbidden")) {
            ResourceMetricsAvailabilityStatus::Forbidden
        } else if had_unavailable_error {
            ResourceMetricsAvailabilityStatus::Unavailable
        } else {
            ResourceMetricsAvailabilityStatus::NoSamples
        }
    } else {
        ResourceMetricsAvailabilityStatus::Available
    };
    let message = match status {
        ResourceMetricsAvailabilityStatus::Available => "metrics available",
        ResourceMetricsAvailabilityStatus::Unavailable => "metrics API unavailable",
        ResourceMetricsAvailabilityStatus::Forbidden => "forbidden",
        ResourceMetricsAvailabilityStatus::NoSamples => "no samples yet",
    };

    Ok(ResourceMetricsSummary {
        cluster: cluster_context,
        availability: availability(status, message),
        pods,
        nodes,
        workloads,
        warnings,
    })
}

#[tauri::command]
pub async fn list_resource_metrics(
    cluster_context: String,
    namespaces: Vec<String>,
    kubeconfig_env_var: Option<String>,
) -> Result<ResourceMetricsSummary, AppError> {
    let started = Instant::now();
    eprintln!(
        "[kubecove:backend] list_resource_metrics start context={} namespaces={}",
        cluster_context,
        namespaces.len()
    );
    let result =
        resource_metrics_from(cluster_context.clone(), namespaces, kubeconfig_env_var).await;
    match &result {
        Ok(summary) => eprintln!(
            "[kubecove:backend] list_resource_metrics done context={} status={:?} pods={} nodes={} workloads={} ms={}",
            cluster_context,
            summary.availability.status,
            summary.pods.len(),
            summary.nodes.len(),
            summary.workloads.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[kubecove:backend] list_resource_metrics error context={} kind={} message={} ms={}",
            cluster_context,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use serde_json::json;

    #[test]
    fn parses_metrics_quantities() {
        assert_eq!(parse_cpu_millicores("250m"), Some(250.0));
        assert_eq!(parse_cpu_millicores("1"), Some(1000.0));
        assert_eq!(parse_cpu_millicores("125000000n"), Some(125.0));
        assert_eq!(parse_memory_bytes("64Mi"), Some(67_108_864));
        assert_eq!(parse_memory_bytes("1536Ki"), Some(1_572_864));
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

        let workloads =
            aggregate_workload_metrics("kind-dev", &[pod], &[replicaset], &[pod_metric]);

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
}
