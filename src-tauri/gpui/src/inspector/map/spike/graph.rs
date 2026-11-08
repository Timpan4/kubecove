//! `createTopologySpikeGraph`: the synthetic topology laid out by the map's
//! Network Flow layout, kept as positioned harness nodes and edges.

use crate::model::Visible;
use crate::topology::{self, FlowItem, synthetic};
use gpui_kit::SharedString;
use kubecove_lib::models::ResourceHealth;
use std::collections::HashSet;
use std::time::Instant;

pub(super) struct Node {
    pub x: f32,
    pub y: f32,
    pub health: ResourceHealth,
    /// `kindAbbreviation`, the compact node's label.
    pub abbreviation: SharedString,
    /// Kind, name and namespace, the full node's three lines.
    pub lines: [SharedString; 3],
}

pub(super) struct Graph {
    pub nodes: Vec<Node>,
    /// Source and target indices into `nodes`.
    pub edges: Vec<(usize, usize)>,
    /// The nodes a benchmark run selects in turn.
    pub selection: Vec<usize>,
    pub layout_ms: f64,
    /// The nodes' bounding box in flow units: x, y, width and height.
    pub bounds: Option<(f32, f32, f32, f32)>,
}

fn abbreviation(kind: &str) -> String {
    match kind {
        "Ingress" => "ING".to_owned(),
        "Service" => "SVC".to_owned(),
        "Deployment" => "DEP".to_owned(),
        "ReplicaSet" => "RS".to_owned(),
        "Pod" => "POD".to_owned(),
        kind => kind.chars().take(3).collect::<String>().to_uppercase(),
    }
}

impl Graph {
    /// `size` is a node's width and height in flow units, which bound the graph.
    pub fn build(node_count: usize, size: (f32, f32)) -> Self {
        let topology = synthetic::build(node_count);
        let rows: Visible = topology
            .nodes
            .iter()
            .map(|node| node.summary.clone())
            .collect();
        // The web times `buildFlowTopologyView`; the native layout also matches rows.
        let started = Instant::now();
        let source = topology::Source::new(&topology, &rows);
        let prepared = topology::prepare(&topology, &rows, &source, None);
        let view = topology::view(&topology, &prepared, None, &HashSet::new(), true);
        let layout_ms = started.elapsed().as_secs_f64() * 1000.;

        let mut index = vec![None; view.nodes.len()];
        let mut nodes = Vec::with_capacity(view.nodes.len());
        let mut kinds = Vec::with_capacity(view.nodes.len());
        for (position, flow) in view.nodes.iter().enumerate() {
            // The harness keeps `ownershipResource` nodes; Network Flow makes no groups.
            let FlowItem::Resource(source) = flow.item else {
                continue;
            };
            let node = &topology.nodes[source];
            index[position] = Some(nodes.len());
            kinds.push(node.kind.clone());
            nodes.push(Node {
                x: flow.x,
                y: flow.y,
                health: node.health,
                abbreviation: abbreviation(&node.kind).into(),
                lines: [
                    node.kind.clone().into(),
                    node.name.clone().into(),
                    node.namespace
                        .clone()
                        .unwrap_or_else(|| "cluster".to_owned())
                        .into(),
                ],
            });
        }
        let edges = view
            .edges
            .iter()
            .filter_map(|edge| Some((index[edge.source]?, index[edge.target]?)))
            .collect();
        let selection = synthetic::selection(kinds.iter().map(String::as_str));
        let bounds = nodes.iter().fold(None, |bounds, node| {
            let (left, top, right, bottom) =
                bounds.unwrap_or((node.x, node.y, node.x + size.0, node.y + size.1));
            Some((
                left.min(node.x),
                top.min(node.y),
                right.max(node.x + size.0),
                bottom.max(node.y + size.1),
            ))
        });
        Self {
            nodes,
            edges,
            selection,
            layout_ms,
            bounds: bounds
                .map(|(left, top, right, bottom)| (left, top, right - left, bottom - top)),
        }
    }
}
