use super::{
    builtin_summary::{builtin_summary, metadata_summary, BuiltinSummary},
    revisions::DEPLOYMENT_REVISION_ANNOTATION,
    topology::{input_with_summary, TopologyInputResource, DEPLOYMENT_REVISION_LABEL},
    topology_dynamic::{list_crd_definition_inputs, list_dynamic_topology_inputs},
    topology_network::{
        NetworkEndpointSlice, NetworkIngressBackend, NetworkService, NetworkTopologyInputs,
    },
};
use crate::commands::helpers::{
    enrich_resource_summaries_with_flux_inventory, list_params, read_flux_ownership_index,
    FluxOwnershipIndex,
};
use crate::commands::MAX_LIST_CONCURRENCY;
use crate::models::{AppError, DiscoveredResourceKind};
use chrono::{DateTime, Utc};
use futures_util::{stream, StreamExt};
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet},
    batch::v1::{CronJob, Job},
    core::v1::{ConfigMap, PersistentVolumeClaim, Pod, Secret, Service},
    discovery::v1::EndpointSlice,
    networking::v1::Ingress,
    storage::v1::StorageClass,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};
use kube::{core::PartialObjectMeta, Api, Client, Error as KubeError};

fn attach_deployment_revision(input: &mut TopologyInputResource, metadata: &ObjectMeta) {
    let Some(revision) = metadata
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(DEPLOYMENT_REVISION_ANNOTATION))
    else {
        return;
    };
    input
        .labels
        .insert(DEPLOYMENT_REVISION_LABEL.to_string(), revision.clone());
}

pub(super) struct TopologyInputCollection {
    pub resources: Vec<TopologyInputResource>,
    pub warnings: Vec<String>,
}

pub fn builtin_input<K: BuiltinSummary>(
    cluster_context: &str,
    object: &K,
    now: DateTime<Utc>,
) -> TopologyInputResource {
    input_with_summary(
        object.metadata(),
        builtin_summary(cluster_context, object, now),
    )
}

pub fn builtin_inputs<K: BuiltinSummary>(
    cluster_context: &str,
    objects: &[K],
    now: DateTime<Utc>,
) -> Vec<TopologyInputResource> {
    objects
        .iter()
        .map(|object| builtin_input(cluster_context, object, now))
        .collect()
}

pub(crate) fn metadata_inputs<K: BuiltinSummary>(
    cluster_context: &str,
    items: &[PartialObjectMeta<K>],
    now: DateTime<Utc>,
) -> Vec<TopologyInputResource> {
    items
        .iter()
        .map(|item| {
            input_with_summary(
                &item.metadata,
                metadata_summary::<K>(cluster_context, &item.metadata, now),
            )
        })
        .collect()
}

pub(super) fn is_optional_topology_list_error(error: &KubeError) -> bool {
    matches!(error, KubeError::Api(api_error) if matches!(api_error.code, 403 | 404))
}

pub(super) fn push_topology_list_warning<T>(
    warnings: &mut Vec<String>,
    namespace: Option<&str>,
    error: &KubeError,
) where
    T: k8s_openapi::Resource,
{
    warnings.push(format!(
        "Skipped {}{} in topology: {}",
        <T as k8s_openapi::Resource>::KIND,
        namespace.map_or_else(
            || " across namespaces".to_string(),
            |namespace| format!(" in namespace {namespace}"),
        ),
        error
    ));
}

async fn list_namespaced_with_warnings<T>(
    client: Client,
    namespaces: &[String],
) -> Result<(Vec<T>, Vec<String>), AppError>
where
    T: Clone
        + std::fmt::Debug
        + serde::de::DeserializeOwned
        + k8s_openapi::Metadata<Ty = ObjectMeta>
        + kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Send
        + Sync
        + 'static,
{
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    if namespaces.is_empty() {
        let api: Api<T> = Api::all(client);
        match api.list(&list_params()).await {
            Ok(rows) => out.extend(rows.items),
            Err(error) if is_optional_topology_list_error(&error) => {
                push_topology_list_warning::<T>(&mut warnings, None, &error);
            }
            Err(error) => return Err(AppError::from(error)),
        }
        return Ok((out, warnings));
    }
    let outcomes = stream::iter(namespaces.to_vec())
        .map(|namespace| {
            let api: Api<T> = Api::namespaced(client.clone(), &namespace);
            async move {
                (
                    namespace,
                    api.list(&list_params()).await.map(|rows| rows.items),
                )
            }
        })
        .buffered(MAX_LIST_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    for (namespace, outcome) in outcomes {
        match outcome {
            Ok(rows) => out.extend(rows),
            Err(error) if is_optional_topology_list_error(&error) => {
                push_topology_list_warning::<T>(&mut warnings, Some(&namespace), &error);
            }
            Err(error) => return Err(AppError::from(error)),
        }
    }
    Ok((out, warnings))
}

/// Lists only object metadata so Secret/ConfigMap payloads never reach the backend.
async fn list_namespaced_metadata_with_warnings<T>(
    client: Client,
    namespaces: &[String],
) -> Result<(Vec<PartialObjectMeta<T>>, Vec<String>), AppError>
where
    T: Clone
        + std::fmt::Debug
        + serde::de::DeserializeOwned
        + kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + k8s_openapi::Resource
        + Send
        + Sync
        + 'static,
{
    let scopes: Vec<Option<String>> = if namespaces.is_empty() {
        vec![None]
    } else {
        namespaces.iter().cloned().map(Some).collect()
    };
    let outcomes = stream::iter(scopes)
        .map(|namespace| {
            let api: Api<T> = match &namespace {
                Some(namespace) => Api::namespaced(client.clone(), namespace),
                None => Api::all(client.clone()),
            };
            async move {
                let rows = api.list_metadata(&list_params()).await;
                (namespace, rows.map(|rows| rows.items))
            }
        })
        .buffered(MAX_LIST_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for (namespace, outcome) in outcomes {
        match outcome {
            Ok(rows) => out.extend(rows),
            Err(error) if is_optional_topology_list_error(&error) => {
                push_topology_list_warning::<T>(&mut warnings, namespace.as_deref(), &error);
            }
            Err(error) => return Err(AppError::from(error)),
        }
    }
    Ok((out, warnings))
}

async fn list_cluster_with_warnings<T>(client: Client) -> Result<(Vec<T>, Vec<String>), AppError>
where
    T: Clone
        + std::fmt::Debug
        + serde::de::DeserializeOwned
        + k8s_openapi::Metadata<Ty = ObjectMeta>
        + kube::Resource<DynamicType = (), Scope = ClusterResourceScope>
        + Send
        + Sync
        + 'static,
{
    let api: Api<T> = Api::all(client);
    match api.list(&list_params()).await {
        Ok(rows) => Ok((rows.items, Vec::new())),
        Err(error) if is_optional_topology_list_error(&error) => {
            let mut warnings = Vec::new();
            push_topology_list_warning::<T>(&mut warnings, None, &error);
            Ok((Vec::new(), warnings))
        }
        Err(error) => Err(AppError::from(error)),
    }
}

pub(super) fn cluster_scoped_input_visible_in_scope(
    input: &TopologyInputResource,
    namespaces: &[String],
) -> bool {
    namespaces.is_empty()
        || input.summary.git_ops_owner.is_some()
        || input.summary.helm_release.is_some()
        || input.owner.is_some()
}

pub(super) async fn collect_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
    custom_resource_kinds: Vec<DiscoveredResourceKind>,
    custom_resource_kinds_are_present: bool,
) -> Result<TopologyInputCollection, AppError> {
    let (mut workload_inputs, mut support_inputs, mut custom_inputs) = futures_util::try_join!(
        collect_workload_topology_inputs(client.clone(), cluster_context, namespaces),
        collect_support_topology_inputs(client.clone(), cluster_context, namespaces),
        collect_custom_topology_inputs(
            client.clone(),
            cluster_context,
            namespaces,
            custom_resource_kinds,
            custom_resource_kinds_are_present,
        ),
    )?;

    workload_inputs
        .resources
        .append(&mut support_inputs.resources);
    workload_inputs
        .resources
        .append(&mut custom_inputs.resources);
    workload_inputs
        .warnings
        .append(&mut support_inputs.warnings);
    workload_inputs.warnings.append(&mut custom_inputs.warnings);
    let index = read_flux_ownership_index(client, namespaces).await?;
    workload_inputs.resources =
        topology_inputs_with_flux_ownership(workload_inputs.resources, &index, namespaces);
    Ok(workload_inputs)
}

pub fn topology_inputs_with_flux_ownership(
    mut resources: Vec<TopologyInputResource>,
    index: &FluxOwnershipIndex,
    namespaces: &[String],
) -> Vec<TopologyInputResource> {
    for input in &mut resources {
        enrich_resource_summaries_with_flux_inventory(
            std::slice::from_mut(&mut input.summary),
            index,
        );
    }
    resources.retain(|input| {
        input.summary.kind != "StorageClass"
            || cluster_scoped_input_visible_in_scope(input, namespaces)
    });
    resources
}

async fn collect_workload_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<TopologyInputCollection, AppError> {
    let (
        (deployments, mut deployment_warnings),
        (daemonsets, mut daemonset_warnings),
        (replicasets, mut replicaset_warnings),
        (statefulsets, mut statefulset_warnings),
        (cronjobs, mut cronjob_warnings),
        (jobs, mut job_warnings),
        (pods, mut pod_warnings),
    ) = futures_util::try_join!(
        list_namespaced_with_warnings::<Deployment>(client.clone(), namespaces),
        list_namespaced_with_warnings::<DaemonSet>(client.clone(), namespaces),
        list_namespaced_with_warnings::<ReplicaSet>(client.clone(), namespaces),
        list_namespaced_with_warnings::<StatefulSet>(client.clone(), namespaces),
        list_namespaced_with_warnings::<CronJob>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Job>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Pod>(client, namespaces),
    )?;

    let mut inputs = Vec::new();
    let mut warnings = Vec::new();
    warnings.append(&mut deployment_warnings);
    warnings.append(&mut daemonset_warnings);
    warnings.append(&mut replicaset_warnings);
    warnings.append(&mut statefulset_warnings);
    warnings.append(&mut cronjob_warnings);
    warnings.append(&mut job_warnings);
    warnings.append(&mut pod_warnings);

    let now = Utc::now();
    for deploy in &deployments {
        let mut input = builtin_input(cluster_context, deploy, now);
        attach_deployment_revision(&mut input, &deploy.metadata);
        inputs.push(input);
    }
    inputs.extend(builtin_inputs(cluster_context, &daemonsets, now));
    for rs in &replicasets {
        let mut input = builtin_input(cluster_context, rs, now);
        attach_deployment_revision(&mut input, &rs.metadata);
        inputs.push(input);
    }
    inputs.extend(builtin_inputs(cluster_context, &statefulsets, now));
    inputs.extend(builtin_inputs(cluster_context, &cronjobs, now));
    inputs.extend(builtin_inputs(cluster_context, &jobs, now));
    for pod in pods {
        inputs.push(pod_input(cluster_context, pod, now));
    }

    Ok(TopologyInputCollection {
        resources: inputs,
        warnings,
    })
}

async fn collect_support_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<TopologyInputCollection, AppError> {
    let (
        (pvcs, mut pvc_warnings),
        (services, mut service_warnings),
        (ingresses, mut ingress_warnings),
        (configmaps, mut configmap_warnings),
        (secrets, mut secret_warnings),
        (storageclasses, mut storageclass_warnings),
    ) = futures_util::try_join!(
        list_namespaced_with_warnings::<PersistentVolumeClaim>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Service>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Ingress>(client.clone(), namespaces),
        list_namespaced_metadata_with_warnings::<ConfigMap>(client.clone(), namespaces),
        list_namespaced_metadata_with_warnings::<Secret>(client.clone(), namespaces),
        list_cluster_with_warnings::<StorageClass>(client),
    )?;

    let mut inputs = Vec::new();
    let mut warnings = Vec::new();
    warnings.append(&mut pvc_warnings);
    warnings.append(&mut service_warnings);
    warnings.append(&mut ingress_warnings);
    warnings.append(&mut configmap_warnings);
    warnings.append(&mut secret_warnings);
    warnings.append(&mut storageclass_warnings);

    let now = Utc::now();
    inputs.extend(builtin_inputs(cluster_context, &pvcs, now));
    inputs.extend(builtin_inputs(cluster_context, &services, now));
    inputs.extend(builtin_inputs(cluster_context, &ingresses, now));
    inputs.extend(metadata_inputs(cluster_context, &configmaps, now));
    inputs.extend(metadata_inputs(cluster_context, &secrets, now));
    inputs.extend(builtin_inputs(cluster_context, &storageclasses, now));

    Ok(TopologyInputCollection {
        resources: inputs,
        warnings,
    })
}

async fn collect_custom_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
    custom_resource_kinds: Vec<DiscoveredResourceKind>,
    custom_resource_kinds_are_present: bool,
) -> Result<TopologyInputCollection, AppError> {
    let dynamic_client = client.clone();
    let definition_client = client;
    let dynamic = async {
        let mut warnings = Vec::new();
        let rows = list_dynamic_topology_inputs(
            dynamic_client,
            cluster_context,
            namespaces,
            &mut warnings,
            custom_resource_kinds,
            custom_resource_kinds_are_present,
        )
        .await?;
        Ok::<_, AppError>((rows, warnings))
    };
    let definitions = async {
        let mut warnings = Vec::new();
        let rows = list_crd_definition_inputs(
            definition_client,
            cluster_context,
            namespaces,
            &mut warnings,
        )
        .await?;
        Ok::<_, AppError>((rows, warnings))
    };
    let ((mut resources, mut warnings), (mut crds, mut crd_warnings)) =
        futures_util::try_join!(dynamic, definitions)?;
    resources.append(&mut crds);
    warnings.append(&mut crd_warnings);
    Ok(TopologyInputCollection {
        resources,
        warnings,
    })
}

pub fn pod_input(cluster_context: &str, pod: Pod, now: DateTime<Utc>) -> TopologyInputResource {
    let mut input = builtin_input(cluster_context, &pod, now);
    if let Some(spec) = &pod.spec {
        input.port_hints = spec
            .containers
            .iter()
            .flat_map(|container| {
                container
                    .ports
                    .iter()
                    .flatten()
                    .map(|port| port_hint(port.name.as_deref(), port.container_port))
            })
            .collect();
        input.port_hints.sort();
        input.port_hints.dedup();
    }
    input
}

pub(super) async fn collect_network_topology_inputs(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<NetworkTopologyInputs, AppError> {
    let (
        (pods, mut pod_warnings),
        (services, mut service_warnings),
        (ingresses, mut ingress_warnings),
        (endpoint_slices, mut endpoint_slice_warnings),
    ) = futures_util::try_join!(
        list_namespaced_with_warnings::<Pod>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Service>(client.clone(), namespaces),
        list_namespaced_with_warnings::<Ingress>(client.clone(), namespaces),
        list_namespaced_with_warnings::<EndpointSlice>(client, namespaces),
    )?;

    let mut warnings = Vec::new();
    warnings.append(&mut pod_warnings);
    warnings.append(&mut service_warnings);
    warnings.append(&mut ingress_warnings);
    warnings.append(&mut endpoint_slice_warnings);
    Ok(network_topology_inputs_from_objects_at(
        cluster_context,
        pods,
        services,
        ingresses,
        endpoint_slices,
        warnings,
        Utc::now(),
    ))
}

pub(crate) fn network_topology_inputs_from_objects_at(
    cluster_context: &str,
    pods: Vec<Pod>,
    services: Vec<Service>,
    ingresses: Vec<Ingress>,
    endpoint_slices: Vec<EndpointSlice>,
    warnings: Vec<String>,
    now: DateTime<Utc>,
) -> NetworkTopologyInputs {
    let mut resources = Vec::new();
    let mut service_flows = Vec::new();
    let mut ingress_backends = Vec::new();
    let mut endpoint_slice_flows = Vec::new();

    for pod in pods {
        resources.push(pod_input(cluster_context, pod, now));
    }
    for service in services {
        let mut input = builtin_input(cluster_context, &service, now);
        let namespace = service.metadata.namespace.clone().unwrap_or_default();
        let name = service.metadata.name.clone().unwrap_or_default();
        let selector = service
            .spec
            .as_ref()
            .and_then(|spec| spec.selector.clone())
            .unwrap_or_default();
        let service_type = service
            .spec
            .as_ref()
            .and_then(|spec| spec.type_.clone())
            .unwrap_or_else(|| "ClusterIP".to_string());
        if let Some(spec) = &service.spec {
            input.port_hints = spec
                .ports
                .iter()
                .flatten()
                .map(|port| port_hint(port.name.as_deref(), port.port))
                .collect();
            input.port_hints.sort();
            input.port_hints.dedup();
        }
        service_flows.push(NetworkService {
            namespace,
            name,
            service_type,
            selector,
        });
        resources.push(input);
    }
    for ingress in ingresses {
        let namespace = ingress.metadata.namespace.clone().unwrap_or_default();
        let ingress_name = ingress.metadata.name.clone().unwrap_or_default();
        if let Some(spec) = &ingress.spec {
            if let Some(default_backend) = &spec.default_backend {
                if let Some(service_name) = backend_service_name(default_backend) {
                    ingress_backends.push(NetworkIngressBackend {
                        namespace: namespace.clone(),
                        ingress_name: ingress_name.clone(),
                        service_name,
                    });
                }
            }
            for rule in spec.rules.iter().flatten() {
                let Some(http) = &rule.http else {
                    continue;
                };
                for path in &http.paths {
                    if let Some(service_name) = backend_service_name(&path.backend) {
                        ingress_backends.push(NetworkIngressBackend {
                            namespace: namespace.clone(),
                            ingress_name: ingress_name.clone(),
                            service_name,
                        });
                    }
                }
            }
        }
        resources.push(builtin_input(cluster_context, &ingress, now));
    }
    for slice in endpoint_slices {
        let namespace = slice.metadata.namespace.clone().unwrap_or_default();
        let name = slice.metadata.name.clone().unwrap_or_default();
        let service_name = slice
            .metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get("kubernetes.io/service-name").cloned());
        let target_pods: Vec<String> = slice
            .endpoints
            .iter()
            .filter_map(|endpoint| {
                let target = endpoint.target_ref.as_ref()?;
                (target.kind.as_deref() == Some("Pod")).then(|| target.name.clone())?
            })
            .collect();
        let mut input = builtin_input(cluster_context, &slice, now);
        input.port_hints = slice
            .ports
            .iter()
            .flatten()
            .filter_map(|port| {
                port.port
                    .map(|number| port_hint(port.name.as_deref(), number))
            })
            .collect();
        input.port_hints.sort();
        input.port_hints.dedup();
        endpoint_slice_flows.push(NetworkEndpointSlice {
            namespace,
            name,
            service_name,
            target_pods,
        });
        resources.push(input);
    }

    NetworkTopologyInputs {
        resources,
        services: service_flows,
        ingress_backends,
        endpoint_slices: endpoint_slice_flows,
        warnings,
    }
}

#[cfg(feature = "bench-support")]
pub fn network_topology_from_objects_at(
    cluster_context: &str,
    pods: Vec<Pod>,
    services: Vec<Service>,
    ingresses: Vec<Ingress>,
    endpoint_slices: Vec<EndpointSlice>,
    warnings: Vec<String>,
    now: DateTime<Utc>,
) -> crate::models::ResourceTopology {
    super::topology_network::build_network_flow_topology(network_topology_inputs_from_objects_at(
        cluster_context,
        pods,
        services,
        ingresses,
        endpoint_slices,
        warnings,
        now,
    ))
}

fn backend_service_name(
    backend: &k8s_openapi::api::networking::v1::IngressBackend,
) -> Option<String> {
    backend.service.as_ref().map(|service| service.name.clone())
}

fn port_hint(name: Option<&str>, port: i32) -> String {
    match name {
        Some(name) if !name.is_empty() => format!("{name}:{port}"),
        _ => port.to_string(),
    }
}
