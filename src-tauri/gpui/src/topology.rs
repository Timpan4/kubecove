//! Ownership-map view model, ported from `topology-view.ts`,
//! `topology-implementation.ts` and the table filter in
//! `resourceBrowserModel.ts`. Rendering lives in `inspector/map.rs`.

mod layout;
mod source;
pub use source::Source;
pub mod node;

use kubecove_lib::models::{ResourceSummary, ResourceTopology, TopologyNode};
use layout::{CANVAS_PADDING, Graph, NODE_HEIGHT, NODE_WIDTH, STANDALONE_NODE_WIDTH};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;

/// xyflow's `minZoom` and `maxZoom` for the ownership map. The zoom controls disable at these limits.
pub const MIN_ZOOM: f32 = 0.12;
pub const MAX_ZOOM: f32 = 1.4;
const WIDTH_FIT_PADDING: f32 = 0.08;

/// Map entries refer to the topology by index, so a built view does not
/// borrow it and can outlive the frame it was built for.
pub enum FlowItem {
    /// An index into `ResourceTopology::nodes`.
    Resource(usize),
    /// Ownerless resources of one kind; `members` index the filtered graph.
    Group {
        kind: String,
        members: Vec<usize>,
        expanded: bool,
    },
}

/// A placed map node. Coordinates are absolute flow units.
pub struct FlowNode {
    pub item: FlowItem,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub selected: bool,
    pub connected: bool,
    pub dimmed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EdgeState {
    /// No resource is selected.
    Idle,
    /// On the selected resource's path.
    Active,
    Inactive,
}

/// An edge between two entries of `View::nodes`.
pub struct FlowEdge {
    pub source: usize,
    pub target: usize,
    pub state: EdgeState,
}

pub struct View {
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    /// Topology nodes after the table filter, before focusing on a selection.
    pub topology_nodes: usize,
    /// The graph shows only the selected resource's root subtree.
    pub focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Viewport {
    pub fn clamp_zoom(zoom: f32) -> f32 {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    }
}

fn loose_key(row: &ResourceSummary) -> (&str, &str, Option<&str>, &str) {
    (&row.cluster, &row.kind, row.namespace.as_deref(), &row.name)
}

/// `filterTopologyByTableRows`: matching resources plus every ancestor that
/// explains their ownership. Historical ReplicaSets stay; the native table
/// has no "Hide historical ReplicaSets" filter yet.
#[cfg(test)]
fn filter_by_rows(
    topology: &ResourceTopology,
    rows: &crate::model::Visible,
) -> (Vec<usize>, Vec<usize>) {
    let keys: HashSet<_> = rows.iter().map(loose_key).collect();
    let known: HashSet<&str> = topology.nodes.iter().map(|node| node.id.as_str()).collect();
    let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &topology.edges {
        incoming
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }
    let mut kept: HashSet<&str> = topology
        .nodes
        .iter()
        .filter(|node| keys.contains(&loose_key(&node.summary)))
        .map(|node| node.id.as_str())
        .collect();
    let mut pending: Vec<&str> = kept.iter().copied().collect();
    while let Some(id) = pending.pop() {
        for &parent in incoming.get(id).into_iter().flatten() {
            if known.contains(parent) && kept.insert(parent) {
                pending.push(parent);
            }
        }
    }
    (
        topology
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| kept.contains(node.id.as_str()))
            .map(|(index, _)| index)
            .collect(),
        topology
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                kept.contains(edge.source.as_str()) && kept.contains(edge.target.as_str())
            })
            .map(|(index, _)| index)
            .collect(),
    )
}

/// `syncedTopologyNodeId`: an exact identity match wins over a match that
/// ignores apiVersion.
fn selected_id<'a>(nodes: &[&'a TopologyNode], resource: &ResourceSummary) -> Option<&'a str> {
    let loose = |node: &&&TopologyNode| loose_key(&node.summary) == loose_key(resource);
    nodes
        .iter()
        .find(|node| loose(node) && node.summary.api_version == resource.api_version)
        .or_else(|| nodes.iter().find(loose))
        .map(|node| node.id.as_str())
}

/// `selectedTopologyPath`: ancestors and descendants of `selected`.
fn selected_path(graph: &Graph, selected: usize) -> (Vec<bool>, Vec<bool>) {
    let mut nodes = vec![false; graph.nodes.len()];
    let mut edges = vec![false; graph.edges.len()];
    nodes[selected] = true;
    let mut pending = vec![selected];
    while let Some(node) = pending.pop() {
        for &edge in &graph.incoming[node] {
            let source = graph.endpoints[edge].0;
            if !std::mem::replace(&mut edges[edge], true) && !nodes[source] {
                nodes[source] = true;
                pending.push(source);
            }
        }
    }
    let mut pending = vec![selected];
    while let Some(node) = pending.pop() {
        for &edge in &graph.outgoing[node] {
            let target = graph.endpoints[edge].1;
            if !std::mem::replace(&mut edges[edge], true) && !nodes[target] {
                nodes[target] = true;
                pending.push(target);
            }
        }
    }
    (nodes, edges)
}

/// `selectedTopologyRootSubtree`: everything under the selection's roots.
fn root_subtree(graph: &Graph, selected: usize) -> (Vec<bool>, Vec<bool>) {
    let mut seen = vec![false; graph.nodes.len()];
    let mut roots = Vec::new();
    let mut pending = vec![selected];
    while let Some(node) = pending.pop() {
        if std::mem::replace(&mut seen[node], true) {
            continue;
        }
        if graph.incoming[node].is_empty() {
            roots.push(node);
        }
        for &edge in &graph.incoming[node] {
            pending.push(graph.endpoints[edge].0);
        }
    }
    let mut nodes = vec![false; graph.nodes.len()];
    let mut edges = vec![false; graph.edges.len()];
    for &root in &roots {
        nodes[root] = true;
    }
    let mut pending = roots;
    while let Some(node) = pending.pop() {
        for &edge in &graph.outgoing[node] {
            let target = graph.endpoints[edge].1;
            if !std::mem::replace(&mut edges[edge], true) && !nodes[target] {
                nodes[target] = true;
                pending.push(target);
            }
        }
    }
    (nodes, edges)
}

/// Selection-independent layout. Indices refer to the source topology;
/// changing selection never changes filtering, depths or tree placement.
pub struct Prepared {
    source_nodes: Vec<usize>,
    source_edges: Vec<usize>,
    depth: Vec<i32>,
    order: Vec<usize>,
    positions: Vec<Option<(f32, f32)>>,
    graph: layout::GraphData,
}

pub fn prepare(
    topology: &ResourceTopology,
    rows: &crate::model::Visible,
    source: &Source,
) -> Prepared {
    // The graph keeps the last node of a repeated id, so the index does too.
    let filter_span = crate::frame_profile::scope("topology", "filter");
    let (source_nodes, source_edges) = source.filter(rows);
    drop(filter_span);
    let graph_span = crate::frame_profile::scope("topology", "graph");
    let graph = Graph::ranked(
        source_nodes
            .iter()
            .map(|&index| (index, &topology.nodes[index])),
        source_edges
            .iter()
            .map(|&index| (index, &topology.edges[index])),
        source,
    );
    drop(graph_span);
    let layout_span = crate::frame_profile::scope("topology", "depth_and_order");
    let (depth, order) = source.depth_and_order(&graph);
    drop(layout_span);
    let positions_span = crate::frame_profile::scope("topology", "positions");
    let positions = layout::positions(&graph, &depth, &order);
    drop(positions_span);
    Prepared {
        source_nodes,
        source_edges,
        depth,
        order,
        positions,
        graph: graph.into_data(),
    }
}

/// Builds selection and standalone expansion from the prepared layout.
pub fn view(
    topology: &ResourceTopology,
    prepared: &Prepared,
    selected: Option<&ResourceSummary>,
    expanded_kinds: &HashSet<String>,
    network: bool,
) -> View {
    let nodes: Vec<_> = prepared
        .graph
        .source_nodes
        .iter()
        .map(|&index| &topology.nodes[index])
        .collect();
    let edges: Vec<_> = prepared
        .graph
        .source_edges
        .iter()
        .map(|&index| &topology.edges[index])
        .collect();
    let topology_nodes = prepared.source_nodes.len();
    // Selection observes the original filtered entries before graph deduplication.
    let selection_nodes: Vec<_> = prepared
        .source_nodes
        .iter()
        .map(|&index| &topology.nodes[index])
        .collect();
    let selected = selected.and_then(|resource| selected_id(&selection_nodes, resource));
    let expansion = selected.filter(|&id| {
        !prepared.source_edges.iter().any(|&index| {
            let edge = &topology.edges[index];
            edge.source == id || edge.target == id
        })
    });
    let graph = Graph::prepared(nodes, edges, &prepared.graph);
    let (depth, order, positions) = (&prepared.depth, &prepared.order, &prepared.positions);
    let cmp = |a: usize, b: usize| layout::compare(&graph, depth, order, a, b);
    let standalone = if network {
        layout::Standalone {
            groups: Vec::new(),
            is_standalone: vec![false; graph.nodes.len()],
            member_positions: std::collections::HashMap::new(),
        }
    } else {
        layout::standalone_groups(&graph, positions, cmp, expanded_kinds, expansion)
    };

    let mut flow: Vec<FlowNode> = standalone
        .groups
        .iter()
        .map(|group| FlowNode {
            item: FlowItem::Group {
                kind: group.kind.clone(),
                members: group.members.clone(),
                expanded: group.expanded,
            },
            x: group.x,
            y: group.y,
            width: group.width,
            height: group.height,
            selected: false,
            connected: false,
            dimmed: false,
        })
        .collect();
    let mut graph_nodes: Vec<Option<usize>> = vec![None; flow.len()];
    let mut sorted: Vec<usize> = (0..graph.nodes.len()).collect();
    sorted.sort_by(|&a, &b| cmp(a, b));
    let mut flow_index = vec![None; graph.nodes.len()];
    for node in sorted {
        let member = standalone.member_positions.get(&node);
        if standalone.is_standalone[node] && member.is_none() {
            continue;
        }
        let (x, y) = member
            .copied()
            .or(positions[node])
            .unwrap_or((CANVAS_PADDING, CANVAS_PADDING));
        flow_index[node] = Some(flow.len());
        graph_nodes.push(Some(node));
        flow.push(FlowNode {
            item: FlowItem::Resource(prepared.graph.source_nodes[node]),
            x,
            y,
            width: if member.is_some() {
                STANDALONE_NODE_WIDTH
            } else {
                NODE_WIDTH
            },
            height: NODE_HEIGHT,
            selected: false,
            connected: false,
            dimmed: false,
        });
    }
    let mut flow_edges: Vec<(usize, FlowEdge)> = graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(at, _)| {
            let source = flow_index[graph.endpoints[at].0]?;
            let target = flow_index[graph.endpoints[at].1]?;
            Some((
                at,
                FlowEdge {
                    source,
                    target,
                    state: EdgeState::Idle,
                },
            ))
        })
        .collect();

    let selected = selected.and_then(|id| graph.index.get(id).copied());
    let mut focused = false;
    if let Some(selected) = selected {
        // `filterFlowTopologyToSelectedRoot`, then `applyFlowTopologySelectionWithIndex`.
        let (subtree, subtree_edges) = root_subtree(&graph, selected);
        let keep: Vec<bool> = flow
            .iter()
            .zip(&graph_nodes)
            .map(|(node, graph_node)| match (&node.item, graph_node) {
                (FlowItem::Group { members, .. }, _) => {
                    members.iter().any(|&member| subtree[member])
                }
                (FlowItem::Resource(_), Some(graph_node)) => subtree[*graph_node],
                (FlowItem::Resource(_), None) => false,
            })
            .collect();
        let mut remap = vec![None; flow.len()];
        let mut kept = Vec::new();
        let mut kept_graph = Vec::new();
        for (at, (node, graph_node)) in flow.into_iter().zip(graph_nodes).enumerate() {
            if keep[at] {
                remap[at] = Some(kept.len());
                kept.push(node);
                kept_graph.push(graph_node);
            }
        }
        flow_edges = flow_edges
            .into_iter()
            .filter(|(at, _)| subtree_edges[*at])
            .filter_map(|(at, edge)| {
                Some((
                    at,
                    FlowEdge {
                        source: remap[edge.source]?,
                        target: remap[edge.target]?,
                        state: edge.state,
                    },
                ))
            })
            .collect();
        focused = kept.len() < topology_nodes;

        let (path, path_edges) = selected_path(&graph, selected);
        for (node, graph_node) in kept.iter_mut().zip(&kept_graph) {
            match (&node.item, graph_node) {
                (FlowItem::Group { members, .. }, _) => {
                    node.dimmed = !members.iter().any(|&member| path[member]);
                }
                (FlowItem::Resource(_), Some(graph_node)) => {
                    node.selected = *graph_node == selected;
                    node.connected = path[*graph_node] && !node.selected;
                    node.dimmed = !path[*graph_node];
                }
                (FlowItem::Resource(_), None) => {}
            }
        }
        for (at, edge) in &mut flow_edges {
            edge.state = if path_edges[*at] {
                EdgeState::Active
            } else {
                EdgeState::Inactive
            };
        }
        flow = kept;
    }
    View {
        nodes: flow,
        edges: flow_edges.into_iter().map(|(_, edge)| edge).collect(),
        topology_nodes,
        focused,
    }
}

/// `buildFlowTopologyFitPlan`: fits the selected path's width, or the whole
/// graph's, and centers it.
pub fn fit(nodes: &[FlowNode], width: f32, height: f32) -> Option<Viewport> {
    if width <= 0. || height <= 0. {
        return None;
    }
    let has_path = nodes.iter().any(|node| node.selected || node.connected);
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for node in nodes {
        if has_path && !node.selected && !node.connected {
            continue;
        }
        bounds = Some(match bounds {
            None => (node.x, node.y, node.x + node.width, node.y + node.height),
            Some((left, top, right, bottom)) => (
                left.min(node.x),
                top.min(node.y),
                right.max(node.x + node.width),
                bottom.max(node.y + node.height),
            ),
        });
    }
    let (left, top, right, bottom) = bounds?;
    let usable = (width * (1. - WIDTH_FIT_PADDING * 2.)).max(1.);
    let zoom = Viewport::clamp_zoom(usable / (right - left).max(1.));
    Some(Viewport {
        x: width / 2. - (left + right) / 2. * zoom,
        y: height / 2. - (top + bottom) / 2. * zoom,
        zoom,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    /// A quadratic curve through the control point to the end point.
    Quad {
        control: (f32, f32),
        to: (f32, f32),
    },
}

const EDGE_RADIUS: f32 = 10.;
const EDGE_OFFSET: f32 = 22.;
const EDGE_STEP: f32 = 0.42;

/// `getSmoothStepPath` from a right-side source handle to a left-side target
/// handle, with `EDGE_PATH_OPTIONS`.
pub fn edge_path(source: (f32, f32), target: (f32, f32)) -> Vec<Segment> {
    let source_gapped = (source.0 + EDGE_OFFSET, source.1);
    let target_gapped = (target.0 - EDGE_OFFSET, target.1);
    let middle = if source_gapped.0 < target_gapped.0 {
        let x = source_gapped.0 + (target_gapped.0 - source_gapped.0) * EDGE_STEP;
        [(x, source_gapped.1), (x, target_gapped.1)]
    } else {
        let y = (source_gapped.1 + target_gapped.1) / 2.;
        [(source_gapped.0, y), (target_gapped.0, y)]
    };
    let points = [
        source,
        source_gapped,
        middle[0],
        middle[1],
        target_gapped,
        target,
    ];
    let distance =
        |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let mut path = vec![Segment::Move(source.0, source.1)];
    for window in points.windows(3) {
        let [a, b, c] = [window[0], window[1], window[2]];
        let bend = (distance(a, b) / 2.)
            .min(distance(b, c) / 2.)
            .min(EDGE_RADIUS);
        let (x, y) = b;
        if (a.0 == x && x == c.0) || (a.1 == y && y == c.1) {
            path.push(Segment::Line(x, y));
        } else if a.1 == y {
            let x_dir = if a.0 < c.0 { -1. } else { 1. };
            let y_dir = if a.1 < c.1 { 1. } else { -1. };
            path.push(Segment::Line(x + bend * x_dir, y));
            path.push(Segment::Quad {
                control: (x, y),
                to: (x, y + bend * y_dir),
            });
        } else {
            let x_dir = if a.0 < c.0 { 1. } else { -1. };
            let y_dir = if a.1 < c.1 { -1. } else { 1. };
            path.push(Segment::Line(x, y + bend * y_dir));
            path.push(Segment::Quad {
                control: (x, y),
                to: (x + bend * x_dir, y),
            });
        }
    }
    path.push(Segment::Line(target.0, target.1));
    path
}
