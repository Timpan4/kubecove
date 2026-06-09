use super::{
    topology_collection::{collect_network_topology_inputs, collect_topology_inputs},
    topology_network::build_network_flow_topology,
};
use crate::commands::{
    helpers::{base_resource_summary, extract_owner_ref_summary, resource_age},
    kubeconfig::{kubeconfig_source_key, KubeconfigSource},
    ClusterLiveStore,
};
use crate::models::{
    AppError, OwnerReferenceSummary, ResourceSummary, ResourceTopology, TopologyEdge, TopologyNode,
    TopologyRelation,
};
use chrono::{TimeZone, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;
use tauri::State;

#[derive(Debug, Clone)]
pub(crate) struct TopologyInputResource {
    pub uid: String,
    pub owner: Option<OwnerReferenceSummary>,
    pub labels: BTreeMap<String, String>,
    pub port_hints: Vec<String>,
    pub summary: ResourceSummary,
}

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
                "validation",
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

fn age_from_metadata(metadata: &ObjectMeta) -> String {
    resource_age(metadata.creation_timestamp.clone().map(|t| {
        Utc.timestamp_opt(t.0.as_second(), 0)
            .single()
            .unwrap_or_else(Utc::now)
    }))
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

pub(crate) fn topology_root_kinds() -> &'static [&'static str] {
    &["Deployment", "DaemonSet", "StatefulSet", "CronJob"]
}

pub(crate) fn topology_standalone_kinds() -> &'static [&'static str] {
    &["Service", "Ingress", "ConfigMap", "Secret"]
}

fn node_health(summary: &ResourceSummary) -> String {
    let status = summary.status.as_deref().unwrap_or("").to_lowercase();
    let ready = summary.ready.as_deref().unwrap_or("").to_lowercase();
    if let Some((ready_count, desired_count)) = parse_ready_ratio(&ready) {
        if desired_count > 0 && ready_count < desired_count {
            return "attention".to_string();
        }
    }
    if matches!(
        status.as_str(),
        "failed" | "error" | "crashloopbackoff" | "imagepullbackoff"
    ) || ready == "false"
    {
        return "degraded".to_string();
    }
    if status == "pending" || status == "terminating" || status == "unknown" {
        return "attention".to_string();
    }
    if summary.restarts.unwrap_or(0) > 0 {
        return "restarted".to_string();
    }
    if matches!(
        status.as_str(),
        "running" | "succeeded" | "ready" | "complete"
    ) || ready == "true"
        || ready.contains('/')
    {
        return "healthy".to_string();
    }
    "unknown".to_string()
}

fn parse_ready_ratio(ready: &str) -> Option<(i32, i32)> {
    let (ready_count, desired_count) = ready.split_once('/')?;
    Some((
        ready_count.trim().parse().ok()?,
        desired_count.trim().parse().ok()?,
    ))
}

fn selectable_kind(kind: &str) -> bool {
    !matches!(kind, "ReplicaSet" | "EndpointSlice")
}

pub(super) fn input_from_metadata(
    cluster_context: &str,
    kind: &str,
    api_version: &str,
    metadata: &ObjectMeta,
) -> TopologyInputResource {
    let mut summary =
        base_resource_summary(kind, cluster_context, metadata, age_from_metadata(metadata));
    summary.api_version = Some(api_version.to_string());
    TopologyInputResource {
        uid: metadata.uid.clone().unwrap_or_default(),
        owner: extract_owner_ref_summary(metadata),
        labels: metadata.labels.clone().unwrap_or_default(),
        port_hints: Vec::new(),
        summary,
    }
}

fn owner_lookup_key(owner: &OwnerReferenceSummary) -> String {
    owner.uid.clone()
}

pub(super) fn node_from_input(input: &TopologyInputResource) -> TopologyNode {
    let api_version = input.summary.api_version.as_deref().unwrap_or("v1");
    TopologyNode {
        id: topology_node_id(
            &input.summary.cluster,
            api_version,
            &input.summary.kind,
            input.summary.namespace.as_deref(),
            &input.summary.name,
        ),
        kind: input.summary.kind.clone(),
        name: input.summary.name.clone(),
        namespace: input.summary.namespace.clone(),
        status: input.summary.status.clone(),
        health: node_health(&input.summary),
        port_hints: input.port_hints.clone(),
        selectable: selectable_kind(&input.summary.kind),
        summary: input.summary.clone(),
    }
}

pub(crate) fn build_resource_topology(inputs: Vec<TopologyInputResource>) -> ResourceTopology {
    let mut deduped_inputs = BTreeMap::new();
    for input in inputs {
        let node = node_from_input(&input);
        deduped_inputs.insert(node.id, input);
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
        let Some(source) = uid_to_node.get(&owner_lookup_key(owner)) else {
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
        let target = node_from_input(input).id;
        push_edge(
            &mut edges,
            &mut edge_keys,
            source.clone(),
            target,
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
    if topology_root_kinds().contains(&kind) {
        return 0;
    }
    if topology_standalone_kinds().contains(&kind) {
        return 0;
    }
    match kind {
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
        let target = node_from_input(pvc).id;
        push_edge(
            edges,
            edge_keys,
            source.clone(),
            target,
            TopologyRelation::Creates,
        );
    }
}

pub async fn resource_topology_from(
    cluster_context: String,
    namespaces: Vec<String>,
    mode: Option<String>,
    kubeconfig_env_var: Option<String>,
) -> Result<ResourceTopology, AppError> {
    let mode = TopologyMode::parse(mode)?;
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    let client = source.client_for_context(&cluster_context).await?;
    match mode {
        TopologyMode::Ownership => {
            let inputs = collect_topology_inputs(client, &cluster_context, &namespaces).await?;
            Ok(build_resource_topology(inputs))
        }
        TopologyMode::NetworkFlow => {
            let inputs =
                collect_network_topology_inputs(client, &cluster_context, &namespaces).await?;
            Ok(build_network_flow_topology(inputs))
        }
    }
}

#[tauri::command]
pub async fn list_resource_topology(
    cluster_context: String,
    namespaces: Vec<String>,
    mode: Option<String>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<ResourceTopology, AppError> {
    let mode = TopologyMode::parse(mode)?;
    let started = Instant::now();
    eprintln!(
        "[kubecove:backend] list_resource_topology start context={} namespaces={} mode={}",
        cluster_context,
        namespaces.join(","),
        mode.as_str()
    );
    let source_key = kubeconfig_source_key(kubeconfig_env_var.as_deref())?;
    let result = live_store
        .topology(
            source_key,
            cluster_context.clone(),
            namespaces.clone(),
            mode.as_str().to_string(),
            {
                let cluster_context = cluster_context.clone();
                let namespaces = namespaces.clone();
                let mode = mode.as_str().to_string();
                let kubeconfig_env_var = kubeconfig_env_var.clone();
                move || {
                    resource_topology_from(
                        cluster_context,
                        namespaces,
                        Some(mode),
                        kubeconfig_env_var,
                    )
                }
            },
        )
        .await;
    match &result {
        Ok(topology) => eprintln!(
            "[kubecove:backend] list_resource_topology done context={} namespaces={} mode={} nodes={} edges={} warnings={} ms={}",
            cluster_context,
            namespaces.join(","),
            mode.as_str(),
            topology.nodes.len(),
            topology.edges.len(),
            topology.warnings.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[kubecove:backend] list_resource_topology error context={} namespaces={} mode={} error_kind={} message={} ms={}",
            cluster_context,
            namespaces.join(","),
            mode.as_str(),
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}
