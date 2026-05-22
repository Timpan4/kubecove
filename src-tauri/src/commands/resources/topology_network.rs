use super::topology::{
    node_from_input, push_edge, topology_kind_rank, topology_node_id, TopologyInputResource,
};
use crate::models::{ResourceTopology, TopologyRelation};
use std::collections::{BTreeMap, HashSet};

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
}

pub(crate) fn build_network_flow_topology(inputs: NetworkTopologyInputs) -> ResourceTopology {
    let NetworkTopologyInputs {
        resources,
        services,
        ingress_backends,
        endpoint_slices,
    } = inputs;
    let mut resources_by_id = BTreeMap::new();
    for input in resources {
        let node = node_from_input(&input);
        resources_by_id.insert(node.id, input);
    }

    let mut nodes_by_id = BTreeMap::new();
    let mut edges = Vec::new();
    let mut edge_keys = HashSet::new();
    let mut warnings = Vec::new();
    let cluster = resources_by_id
        .values()
        .next()
        .map(|input| input.summary.cluster.clone())
        .unwrap_or_default();
    let resource_id = |kind: &str, api_version: &str, namespace: &str, name: &str| {
        topology_node_id(&cluster, api_version, kind, Some(namespace), name)
    };

    for input in resources_by_id.values() {
        if matches!(input.summary.kind.as_str(), "Ingress" | "Service") {
            let node = node_from_input(input);
            nodes_by_id.insert(node.id.clone(), node);
        }
    }

    let pods: Vec<&TopologyInputResource> = resources_by_id
        .values()
        .filter(|input| input.summary.kind == "Pod")
        .collect();

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
        let service_slices: Vec<_> = endpoint_slices
            .iter()
            .filter(|slice| {
                slice.namespace == service.namespace
                    && slice.service_name.as_deref() == Some(service.name.as_str())
            })
            .collect();

        let mut linked_pods = HashSet::new();
        let mut has_pod_targets = false;
        for slice in service_slices.iter() {
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
                    let node = node_from_input(pod_input);
                    nodes_by_id.insert(node.id.clone(), node);
                    linked_pods.insert(pod_id.clone());
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

        if service_slices.is_empty() {
            for pod in pods.iter().filter(|pod| {
                pod.summary.namespace.as_deref() == Some(service.namespace.as_str())
                    && selector_matches(&service.selector, &pod.labels)
            }) {
                let pod_node = node_from_input(pod);
                linked_pods.insert(pod_node.id.clone());
                nodes_by_id.insert(pod_node.id.clone(), pod_node.clone());
                push_edge(
                    &mut edges,
                    &mut edge_keys,
                    service_id.clone(),
                    pod_node.id,
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
        } else if (service_slices.is_empty() || has_pod_targets) && linked_pods.is_empty() {
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
    !selector.is_empty()
        && selector
            .iter()
            .all(|(key, value)| labels.get(key) == Some(value))
}
