use super::{
    topology_collection::{collect_network_topology_inputs, collect_topology_inputs},
    topology_network::build_network_flow_topology,
};
use crate::commands::{
    diagnostic_field, discovery::crd_resource_kinds, helpers::extract_owner_ref_summary,
    BackendCancellationRegistry, ClusterAccess, ClusterLiveStore, FiniteRead,
};
use crate::models::AppErrorKind;
use crate::models::{
    AppError, DiscoveredResourceKind, OwnerReferenceSummary, ResourceSummary, ResourceTopology,
    TopologyEdge, TopologyNode, TopologyRelation,
};
use crate::runtime::State;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TopologyInputResource {
    pub uid: String,
    pub owner: Option<OwnerReferenceSummary>,
    pub labels: BTreeMap<String, String>,
    pub port_hints: Vec<String>,
    pub summary: ResourceSummary,
}

pub(super) const DEPLOYMENT_REVISION_LABEL: &str = "kubecove.dev/deployment-revision";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopologyMode {
    Ownership,
    NetworkFlow,
}

impl TopologyMode {
    fn parse(mode: Option<String>) -> Result<Self, AppError> {
        match mode.as_deref().unwrap_or("ownership") {
            "ownership" => Ok(Self::Ownership),
            "networkFlow" => Ok(Self::NetworkFlow),
            value => Err(AppError::new(
                format!("unsupported topology mode: {value}"),
                AppErrorKind::Validation,
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ownership => "ownership",
            Self::NetworkFlow => "networkFlow",
        }
    }
}

pub(crate) fn topology_node_id(
    cluster: &str,
    api_version: &str,
    kind: &str,
    namespace: Option<&str>,
    name: &str,
) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        cluster,
        api_version,
        kind,
        namespace.unwrap_or(""),
        name
    )
}

fn topology_edge_id(source: &str, target: &str, relation: &TopologyRelation) -> String {
    format!("{source}=>{target}:{relation:?}")
}

fn selectable_kind(kind: &str) -> bool {
    kind != "EndpointSlice"
}

/// Topology adds the object uid, controller owner and labels to the shared summary.
pub(crate) fn input_with_summary(
    metadata: &ObjectMeta,
    summary: ResourceSummary,
) -> TopologyInputResource {
    TopologyInputResource {
        uid: metadata.uid.clone().unwrap_or_default(),
        owner: extract_owner_ref_summary(metadata),
        labels: metadata.labels.clone().unwrap_or_default(),
        port_hints: Vec::new(),
        summary,
    }
}

pub(super) fn input_node_id(input: &TopologyInputResource) -> String {
    topology_node_id(
        &input.summary.cluster,
        input.summary.api_version.as_deref().unwrap_or("v1"),
        &input.summary.kind,
        input.summary.namespace.as_deref(),
        &input.summary.name,
    )
}

pub(super) fn node_from_input(input: &TopologyInputResource) -> TopologyNode {
    TopologyNode {
        id: input_node_id(input),
        kind: input.summary.kind.clone(),
        name: input.summary.name.clone(),
        namespace: input.summary.namespace.clone(),
        status: input.summary.status.clone(),
        health: input.summary.health,
        port_hints: input.port_hints.clone(),
        selectable: selectable_kind(&input.summary.kind),
        deployment_revision: input
            .labels
            .get(DEPLOYMENT_REVISION_LABEL)
            .and_then(|revision| revision.parse().ok()),
        summary: input.summary.clone(),
    }
}

pub fn build_resource_topology(inputs: Vec<TopologyInputResource>) -> ResourceTopology {
    let mut deduped_inputs = BTreeMap::new();
    for input in inputs
        .into_iter()
        .filter(|input| !is_ownership_noise(input))
    {
        deduped_inputs.insert(input_node_id(&input), input);
    }
    let inputs: Vec<TopologyInputResource> = deduped_inputs.into_values().collect();

    let mut uid_to_node = HashMap::new();
    let mut nodes = Vec::new();
    for input in &inputs {
        let node = node_from_input(input);
        if !input.uid.is_empty() {
            uid_to_node.insert(input.uid.clone(), node.id.clone());
        }
        nodes.push(node);
    }

    let mut edges = Vec::new();
    let mut edge_keys = HashSet::new();
    let mut warnings = Vec::new();

    for input in &inputs {
        let Some(owner) = &input.owner else {
            continue;
        };
        let Some(source) = uid_to_node.get(&owner.uid) else {
            warnings.push(format!(
                "{} {}/{} references missing owner {}/{}",
                input.summary.kind,
                input.summary.namespace.as_deref().unwrap_or("<cluster>"),
                input.summary.name,
                owner.kind,
                owner.name
            ));
            continue;
        };
        push_edge(
            &mut edges,
            &mut edge_keys,
            source.clone(),
            input_node_id(input),
            TopologyRelation::Owns,
        );
    }

    add_statefulset_pvc_edges(&inputs, &uid_to_node, &mut edges, &mut edge_keys);

    nodes.sort_by(|a, b| {
        topology_kind_rank(&a.kind)
            .cmp(&topology_kind_rank(&b.kind))
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.name.cmp(&b.name))
    });
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    warnings.sort();

    ResourceTopology {
        nodes,
        edges,
        warnings,
    }
}

fn is_ownership_noise(input: &TopologyInputResource) -> bool {
    input.summary.kind == "ConfigMap" && input.summary.name == "kube-root-ca.crt"
}

pub(super) fn push_edge(
    edges: &mut Vec<TopologyEdge>,
    edge_keys: &mut HashSet<String>,
    source: String,
    target: String,
    relation: TopologyRelation,
) {
    let id = topology_edge_id(&source, &target, &relation);
    if edge_keys.insert(id.clone()) {
        edges.push(TopologyEdge {
            id,
            source,
            target,
            relation,
        });
    }
}

pub(super) fn topology_kind_rank(kind: &str) -> usize {
    match kind {
        "Deployment" | "DaemonSet" | "StatefulSet" | "CronJob" | "Service" | "Ingress"
        | "ConfigMap" | "Secret" => 0,
        "ReplicaSet" | "Job" => 1,
        "EndpointSlice" | "Pod" => 2,
        "PersistentVolumeClaim" => 3,
        _ => 4,
    }
}

fn add_statefulset_pvc_edges(
    inputs: &[TopologyInputResource],
    uid_to_node: &HashMap<String, String>,
    edges: &mut Vec<TopologyEdge>,
    edge_keys: &mut HashSet<String>,
) {
    let statefulsets: BTreeMap<(&str, &str), &TopologyInputResource> = inputs
        .iter()
        .filter(|input| input.summary.kind == "StatefulSet")
        .filter_map(|input| {
            Some((
                (
                    input.summary.namespace.as_deref()?,
                    input.summary.name.as_str(),
                ),
                input,
            ))
        })
        .collect();

    let pods_by_statefulset: Vec<(&TopologyInputResource, &OwnerReferenceSummary)> = inputs
        .iter()
        .filter(|input| input.summary.kind == "Pod")
        .filter_map(|input| {
            let owner = input.owner.as_ref()?;
            (owner.kind == "StatefulSet").then_some((input, owner))
        })
        .collect();

    for pvc in inputs
        .iter()
        .filter(|input| input.summary.kind == "PersistentVolumeClaim")
    {
        let Some(namespace) = pvc.summary.namespace.as_deref() else {
            continue;
        };
        let Some((_, owner)) = pods_by_statefulset.iter().find(|(pod, owner)| {
            pod.summary.namespace.as_deref() == Some(namespace)
                && pvc
                    .summary
                    .name
                    .ends_with(&format!("-{}", pod.summary.name))
                && statefulsets.contains_key(&(namespace, owner.name.as_str()))
        }) else {
            continue;
        };
        let Some(source) = uid_to_node.get(&owner.uid) else {
            continue;
        };
        push_edge(
            edges,
            edge_keys,
            source.clone(),
            input_node_id(pvc),
            TopologyRelation::Creates,
        );
    }
}

pub async fn resource_topology_from(
    access: &ClusterAccess,
    cluster_context: String,
    namespaces: Vec<String>,
    mode: Option<String>,
    custom_resource_kinds: Option<Vec<DiscoveredResourceKind>>,
    custom_resource_kinds_are_present: bool,
) -> Result<ResourceTopology, AppError> {
    for namespace in &namespaces {
        crate::commands::helpers::validate_namespace(Some(namespace))?;
    }
    let mode = TopologyMode::parse(mode)?;
    let client = access.read_client(&cluster_context).await?;
    match mode {
        TopologyMode::Ownership => {
            let custom_resource_kinds = match custom_resource_kinds {
                Some(kinds) => kinds,
                None => crd_resource_kinds(client.clone()).await?,
            };
            let collection = Box::pin(collect_topology_inputs(
                client,
                &cluster_context,
                &namespaces,
                custom_resource_kinds,
                custom_resource_kinds_are_present,
            ))
            .await?;
            let mut topology = build_resource_topology(collection.resources);
            topology.warnings.extend(collection.warnings);
            topology.warnings.sort();
            topology.warnings.dedup();
            Ok(topology)
        }
        TopologyMode::NetworkFlow => {
            let inputs =
                collect_network_topology_inputs(client, &cluster_context, &namespaces).await?;
            Ok(build_network_flow_topology(inputs))
        }
    }
}

pub async fn list_resource_topology(
    cluster_context: String,
    namespaces: Vec<String>,
    mode: Option<String>,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<Arc<ResourceTopology>, AppError> {
    let mode = TopologyMode::parse(mode)?;
    let namespace_count = namespaces.len();
    FiniteRead::new("list_resource_topology")
        .label("context", &cluster_context)
        .label("namespaces", namespaces.join(","))
        .label("mode", mode.as_str())
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            async {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                let source_key = access.source_key().to_string();
                let cached_custom_resource_kinds = match &mode {
                    TopologyMode::Ownership => live_store
                        .cached_present_custom_resource_kinds(
                            &source_key,
                            &cluster_context,
                            &namespaces,
                        )
                        .map(|kinds| (kinds, true)),
                    TopologyMode::NetworkFlow => None,
                };
                let topology_cache_mode = match (&mode, cached_custom_resource_kinds.is_some()) {
                    (TopologyMode::Ownership, false) => "ownership:native",
                    _ => mode.as_str(),
                };
                live_store
                    .topology(
                        source_key,
                        cluster_context.clone(),
                        namespaces.clone(),
                        topology_cache_mode.to_string(),
                        {
                            let cluster_context = cluster_context.clone();
                            let namespaces = namespaces.clone();
                            let mode = mode.as_str().to_string();
                            let cached_custom_resource_kinds = cached_custom_resource_kinds.clone();
                            move || {
                                let cached_custom_resource_kinds =
                                    cached_custom_resource_kinds.clone();
                                async move {
                                    let (custom_resource_kinds, custom_resource_kinds_are_present) =
                                        cached_custom_resource_kinds.unwrap_or_default();
                                    Box::pin(resource_topology_from(
                                        &access,
                                        cluster_context,
                                        namespaces,
                                        Some(mode),
                                        Some(Arc::unwrap_or_clone(custom_resource_kinds)),
                                        custom_resource_kinds_are_present,
                                    ))
                                    .await
                                }
                            }
                        },
                    )
                    .await
            },
            |topology| {
                vec![
                    diagnostic_field("mode", mode.as_str()),
                    diagnostic_field("namespaces", namespace_count),
                    diagnostic_field("nodes", topology.nodes.len()),
                    diagnostic_field("edges", topology.edges.len()),
                    diagnostic_field("warnings", topology.warnings.len()),
                ]
            },
        )
        .await
}
