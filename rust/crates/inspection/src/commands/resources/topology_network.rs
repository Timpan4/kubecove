use super::topology::{
    TopologyInputResource, input_node_id, node_from_input, push_edge, topology_kind_rank,
    topology_node_id,
};
use crate::models::{ResourceTopology, TopologyRelation};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct NetworkService {
    pub namespace: String,
    pub name: String,
    pub service_type: String,
    pub selector: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkIngressBackend {
    pub namespace: String,
    pub ingress_name: String,
    pub service_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkEndpointSlice {
    pub namespace: String,
    pub name: String,
    pub service_name: Option<String>,
    pub target_pods: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkTopologyInputs {
    pub resources: Vec<TopologyInputResource>,
    pub services: Vec<NetworkService>,
    pub ingress_backends: Vec<NetworkIngressBackend>,
    pub endpoint_slices: Vec<NetworkEndpointSlice>,
    pub warnings: Vec<String>,
}

pub(crate) fn build_network_flow_topology(inputs: NetworkTopologyInputs) -> ResourceTopology {
    let NetworkTopologyInputs {
        resources,
        services,
        ingress_backends,
        endpoint_slices,
        warnings: collection_warnings,
    } = inputs;
    let mut resources_by_id = BTreeMap::new();
    for input in resources {
        resources_by_id.insert(input_node_id(&input), input);
    }

    let mut nodes_by_id = BTreeMap::new();
    let mut edges = Vec::new();
    let mut edge_keys = HashSet::new();
    let mut warnings = Vec::new();
    let cluster = resources_by_id
        .values()
        .next()
        .map_or_default(|input| input.summary.cluster.clone());
    let resource_id = |kind: &str, api_version: &str, namespace: &str, name: &str| {
        topology_node_id(&cluster, api_version, kind, Some(namespace), name)
    };

    for input in resources_by_id.values() {
        if matches!(input.summary.kind.as_str(), "Ingress" | "Service") {
            let node = node_from_input(input);
            nodes_by_id.insert(node.id.clone(), node);
        }
    }

    let mut pods_by_namespace: HashMap<&str, Vec<_>> = HashMap::new();
    for (id, input) in &resources_by_id {
        if input.summary.kind == "Pod"
            && let Some(namespace) = input.summary.namespace.as_deref()
        {
            pods_by_namespace
                .entry(namespace)
                .or_default()
                .push((id, input));
        }
    }

    let mut slices_by_service: HashMap<(&str, &str), Vec<_>> = HashMap::new();
    for slice in &endpoint_slices {
        if let Some(service_name) = slice.service_name.as_deref() {
            slices_by_service
                .entry((&slice.namespace, service_name))
                .or_default()
                .push(slice);
        }
    }

    for backend in &ingress_backends {
        let ingress_id = resource_id(
            "Ingress",
            "networking.k8s.io/v1",
            &backend.namespace,
            &backend.ingress_name,
        );
        let service_id = resource_id("Service", "v1", &backend.namespace, &backend.service_name);
        if resources_by_id.contains_key(&ingress_id) && resources_by_id.contains_key(&service_id) {
            push_edge(
                &mut edges,
                &mut edge_keys,
                ingress_id,
                service_id,
                TopologyRelation::RoutesTo,
            );
        } else if resources_by_id.contains_key(&ingress_id) {
            warnings.push(format!(
                "Ingress {}/{} routes to missing Service/{}",
                backend.namespace, backend.ingress_name, backend.service_name
            ));
        }
    }

    for service in &services {
        let service_id = resource_id("Service", "v1", &service.namespace, &service.name);
        // Network flow labels Service nodes with their type; the summary keeps the shared status.
        if let Some(node) = nodes_by_id.get_mut(&service_id) {
            node.status = Some(service.service_type.clone());
        }
        let service_slices = slices_by_service
            .get(&(service.namespace.as_str(), service.name.as_str()))
            .map_or(&[][..], Vec::as_slice);

        let mut has_linked_pod = false;
        let mut has_pod_targets = false;
        for slice in service_slices {
            let slice_id = resource_id(
                "EndpointSlice",
                "discovery.k8s.io/v1",
                &slice.namespace,
                &slice.name,
            );
            if let Some(slice_input) = resources_by_id.get(&slice_id) {
                let node = node_from_input(slice_input);
                nodes_by_id.insert(node.id.clone(), node);
                push_edge(
                    &mut edges,
                    &mut edge_keys,
                    service_id.clone(),
                    slice_id.clone(),
                    TopologyRelation::Targets,
                );
            }

            for pod_name in &slice.target_pods {
                has_pod_targets = true;
                let pod_id = resource_id("Pod", "v1", &slice.namespace, pod_name);
                if let Some(pod_input) = resources_by_id.get(&pod_id) {
                    nodes_by_id
                        .entry(pod_id.clone())
                        .or_insert_with(|| node_from_input(pod_input));
                    has_linked_pod = true;
                    push_edge(
                        &mut edges,
                        &mut edge_keys,
                        slice_id.clone(),
                        pod_id,
                        TopologyRelation::Targets,
                    );
                }
            }
        }

        if service_slices.is_empty()
            && !service.selector.is_empty()
            && let Some(pods) = pods_by_namespace.get(service.namespace.as_str())
        {
            for &(pod_id, pod) in pods {
                if !selector_matches(&service.selector, &pod.labels) {
                    continue;
                }
                nodes_by_id
                    .entry(pod_id.clone())
                    .or_insert_with(|| node_from_input(pod));
                has_linked_pod = true;
                push_edge(
                    &mut edges,
                    &mut edge_keys,
                    service_id.clone(),
                    pod_id.clone(),
                    TopologyRelation::Selects,
                );
            }
        }

        if service.service_type == "ExternalName" {
            continue;
        }

        if service_slices.is_empty() && service.selector.is_empty() {
            warnings.push(format!(
                "Service {}/{} has no selector or EndpointSlice targets",
                service.namespace, service.name
            ));
        } else if (service_slices.is_empty() || has_pod_targets) && !has_linked_pod {
            warnings.push(format!(
                "Service {}/{} has no matching Pod endpoints",
                service.namespace, service.name
            ));
        }
    }

    for slice in endpoint_slices
        .iter()
        .filter(|slice| slice.service_name.is_none())
    {
        warnings.push(format!(
            "EndpointSlice {}/{} has no service-name label",
            slice.namespace, slice.name
        ));
    }

    let mut nodes: Vec<_> = nodes_by_id.into_values().collect();
    nodes.sort_by(|a, b| {
        topology_kind_rank(&a.kind)
            .cmp(&topology_kind_rank(&b.kind))
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.name.cmp(&b.name))
    });
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    warnings.extend(collection_warnings);
    warnings.sort();
    warnings.dedup();

    ResourceTopology {
        nodes,
        edges,
        warnings,
    }
}

fn selector_matches(
    selector: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    selector
        .iter()
        .all(|(key, value)| labels.get(key) == Some(value))
}
