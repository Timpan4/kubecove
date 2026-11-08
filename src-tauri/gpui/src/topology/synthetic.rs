//! Synthetic topology for the developer topology harness, ported from
//! `src/features/resources/topology-spike/synthetic-topology.ts`.

use kubecove_lib::models::{
    HealthAssessment, ResourceHealth, ResourceSummary, ResourceTopology, TopologyEdge,
    TopologyNode, TopologyRelation,
};

/// `TOPOLOGY_SPIKE_SELECTION_COUNT`: how many nodes a benchmark run selects in turn.
pub const SELECTION_COUNT: usize = 100;

const CLUSTER: &str = "spike";

/// `NODE_TEMPLATES`: one application is an ingress-to-pod chain, and each
/// template names its relation to the template before it.
const TEMPLATES: [(&str, &str, Option<TopologyRelation>); 5] = [
    ("Ingress", "networking.k8s.io/v1", None),
    ("Service", "v1", Some(TopologyRelation::RoutesTo)),
    ("Deployment", "apps/v1", Some(TopologyRelation::Targets)),
    ("ReplicaSet", "apps/v1", Some(TopologyRelation::Creates)),
    ("Pod", "v1", Some(TopologyRelation::Owns)),
];

const HEALTH_SEQUENCE: [ResourceHealth; 5] = [
    ResourceHealth::Healthy,
    ResourceHealth::Healthy,
    ResourceHealth::Attention,
    ResourceHealth::Restarted,
    ResourceHealth::Degraded,
];

fn health_name(health: ResourceHealth) -> &'static str {
    match health {
        ResourceHealth::Healthy => "healthy",
        ResourceHealth::Attention => "attention",
        ResourceHealth::Degraded => "degraded",
        ResourceHealth::Restarted => "restarted",
        ResourceHealth::Unknown => "unknown",
    }
}

fn node(app: usize, template: usize) -> TopologyNode {
    let (kind, api_version, _) = &TEMPLATES[template];
    let namespace = format!("namespace-{}", app % 120);
    let name = format!("{}-api-{app}", kind.to_lowercase());
    let health = HEALTH_SEQUENCE[(app + template) % HEALTH_SEQUENCE.len()];
    let healthy = health == ResourceHealth::Healthy;
    let status = if healthy {
        "Running".to_owned()
    } else {
        health_name(health).to_owned()
    };
    let summary = ResourceSummary {
        kind: (*kind).to_owned(),
        cluster: CLUSTER.to_owned(),
        name: name.clone(),
        namespace: Some(namespace.clone()),
        age: format!("{}m", app % 59 + 1),
        api_version: Some((*api_version).to_owned()),
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        health,
        health_assessment: HealthAssessment::default(),
        created_at: None,
        status: Some(status.clone()),
        ready: Some(if healthy { "True" } else { "False" }.to_owned()),
        restarts: None,
        owner_ref: matches!(*kind, "Pod" | "ReplicaSet").then(|| format!("deployment-api-{app}")),
        argo_app: Some(format!("app-{}", app % 80)),
        helm_release: app
            .is_multiple_of(7)
            .then(|| format!("release-{}", app % 30)),
        git_ops_ownership_partial: false,
        git_ops_owner: None,
    };
    TopologyNode {
        // `resourceTopologyNodeId`.
        id: format!("{CLUSTER}:{api_version}:{kind}:{namespace}:{name}"),
        kind: (*kind).to_owned(),
        name,
        namespace: Some(namespace),
        status: Some(status),
        health,
        port_hints: if *kind == "Service" {
            vec!["80:8080".to_owned(), "443:8443".to_owned()]
        } else {
            Vec::new()
        },
        selectable: true,
        deployment_revision: None,
        summary,
    }
}

/// `buildSyntheticTopology`: `node_count` nodes in ingress-to-pod chains. The
/// last chain is cut short when `node_count` is not a multiple of five.
pub fn build(node_count: usize) -> ResourceTopology {
    let mut nodes = Vec::with_capacity(node_count);
    let mut edges = Vec::with_capacity(node_count);
    let mut app = 0;
    while nodes.len() < node_count {
        let first = nodes.len();
        let length = TEMPLATES.len().min(node_count - first);
        nodes.extend((0..length).map(|template| node(app, template)));
        for template in 1..length {
            let (source, target): (&TopologyNode, &TopologyNode) =
                (&nodes[first + template - 1], &nodes[first + template]);
            edges.push(TopologyEdge {
                id: format!("{}->{}", source.id, target.id),
                source: source.id.clone(),
                target: target.id.clone(),
                relation: TEMPLATES[template]
                    .2
                    .clone()
                    .unwrap_or(TopologyRelation::Owns),
            });
        }
        app += 1;
    }
    ResourceTopology {
        nodes,
        edges,
        warnings: Vec::new(),
    }
}

/// `selectionIdsFor`: up to `SELECTION_COUNT` evenly spaced pods, or nodes of
/// any kind when there are no pods. `kinds` lists each laid-out node's kind.
pub fn selection<'a>(kinds: impl Iterator<Item = &'a str> + Clone) -> Vec<usize> {
    let pods: Vec<usize> = kinds
        .clone()
        .enumerate()
        .filter(|(_, kind)| *kind == "Pod")
        .map(|(index, _)| index)
        .collect();
    let candidates = if pods.is_empty() {
        (0..kinds.count()).collect()
    } else {
        pods
    };
    let step = (candidates.len() / SELECTION_COUNT).max(1);
    candidates
        .into_iter()
        .step_by(step)
        .take(SELECTION_COUNT)
        .collect()
}
