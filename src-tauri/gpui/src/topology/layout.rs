//! Ownership-map layout, ported from `topology-graph.ts`, `topology-layout.ts`
//! and `topology-standalone-groups.ts`. Nodes are addressed by their index in
//! `Graph::nodes`.

use crate::collate::{Key, collate};
mod order;
use kubecove_lib::models::{TopologyEdge, TopologyNode};
pub(super) use order::{Scratch, order_keys};
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::{borrow::Cow, ops::Deref};

/// `SVELTE_TOPOLOGY_LAYOUT` in `topology-implementation.ts`.
pub const NODE_WIDTH: f32 = 408.;
pub const NODE_HEIGHT: f32 = 98.;
const COLUMN_GAP: f32 = 112.;
const ROW_GAP: f32 = 30.;
pub const STANDALONE_NODE_WIDTH: f32 = 320.;
pub const CANVAS_PADDING: f32 = 96.;
const SUBTREE_GAP: f32 = 20.;
const ROOT_GROUP_GAP: f32 = 16.;
const STANDALONE_ROOT_GAP: f32 = 10.;

const GROUP_COLUMNS: usize = 3;
const GROUP_HEADER_HEIGHT: f32 = 38.;
const GROUP_COLLAPSED_HEIGHT: f32 = 42.;
const GROUP_PADDING: f32 = 14.;
const GROUP_COLUMN_GAP: f32 = 14.;
const GROUP_ROW_GAP: f32 = 10.;
const GROUP_GAP: f32 = 24.;

/// `buildTopologyGraph`: nodes and edges deduplicated by id (first position,
/// last value), edges limited to known endpoints.
pub struct Graph<'a> {
    pub nodes: Vec<&'a TopologyNode>,
    pub edges: Vec<&'a TopologyEdge>,
    data: Cow<'a, GraphData>,
}

/// Selection-independent adjacency and sort ranks, retained with the layout.
#[derive(Clone)]
pub struct GraphData {
    node_ranks: Vec<usize>,
    pub source_nodes: Vec<usize>,
    pub source_edges: Vec<usize>,
    pub endpoints: Vec<(usize, usize)>,
    pub index: NodeIndex,
    pub parents: Adjacency,
    pub children: Adjacency,
    /// Edge indices per node.
    pub incoming: Adjacency,
    pub outgoing: Adjacency,
}

impl Deref for Graph<'_> {
    type Target = GraphData;
    fn deref(&self) -> &GraphData {
        &self.data
    }
}

#[derive(Clone)]
pub struct NodeIndex {
    source: std::sync::Arc<HashMap<String, usize>>,
    positions: Vec<usize>,
}

impl NodeIndex {
    pub(super) fn get(&self, id: &str) -> Option<&usize> {
        let position = &self.positions[*self.source.get(id)?];
        (*position != usize::MAX).then_some(position)
    }
}

impl std::ops::Index<&str> for NodeIndex {
    type Output = usize;
    fn index(&self, id: &str) -> &usize {
        self.get(id).expect("known graph endpoint")
    }
}

/// Compressed adjacency preserves raw edge order without allocating a vector
/// for every node. Node and edge adjacency share the same row boundaries.
#[derive(Clone)]
pub struct Adjacency {
    offsets: std::sync::Arc<[usize]>,
    entries: Vec<usize>,
}

impl Adjacency {
    fn pair(count: usize, edges: &[(usize, usize)], incoming: bool) -> (Self, Self) {
        let mut offsets = vec![0; count + 1];
        for &(source, target) in edges {
            offsets[if incoming { target } else { source } + 1] += 1;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }
        let mut next = offsets[..count].to_vec();
        let mut nodes = vec![0; edges.len()];
        let mut indices = vec![0; edges.len()];
        for (edge, &(source, target)) in edges.iter().enumerate() {
            let (node, other) = if incoming {
                (target, source)
            } else {
                (source, target)
            };
            let at = next[node];
            nodes[at] = other;
            indices[at] = edge;
            next[node] += 1;
        }
        let offsets: std::sync::Arc<[usize]> = offsets.into();
        (
            Self {
                offsets: offsets.clone(),
                entries: nodes,
            },
            Self {
                offsets,
                entries: indices,
            },
        )
    }
}

impl std::ops::Index<usize> for Adjacency {
    type Output = [usize];
    fn index(&self, node: usize) -> &[usize] {
        &self.entries[self.offsets[node]..self.offsets[node + 1]]
    }
}

impl<'a> Graph<'a> {
    pub fn ranked(
        nodes: impl Iterator<Item = (usize, &'a TopologyNode)>,
        edges: impl Iterator<Item = (usize, &'a TopologyEdge)>,
        source: &super::Source,
    ) -> Self {
        let mut index = NodeIndex {
            source: source.index.clone(),
            positions: vec![usize::MAX; source.locals.len()],
        };
        let mut unique = Vec::with_capacity(nodes.size_hint().0);
        let mut source_nodes = Vec::with_capacity(nodes.size_hint().0);
        let mut node_ranks = Vec::with_capacity(nodes.size_hint().0);
        for (raw, node) in nodes {
            let canonical = source.canonical[raw];
            let position = &mut index.positions[canonical];
            if *position == usize::MAX {
                *position = unique.len();
                unique.push(node);
                source_nodes.push(raw);
                node_ranks.push(source.rank(canonical));
            } else {
                unique[*position] = node;
                source_nodes[*position] = raw;
            }
        }
        let nodes = unique;
        let mut seen = vec![usize::MAX; source.edge_count];
        let mut unique = Vec::with_capacity(edges.size_hint().0);
        for (raw, edge) in edges {
            let position = &mut seen[source.edge_positions[raw]];
            if *position == usize::MAX {
                *position = unique.len();
                unique.push((raw, edge));
            } else {
                unique[*position] = (raw, edge);
            }
        }
        unique.retain(|(raw, _)| {
            source.endpoints[*raw].is_some_and(|(source, target)| {
                index.positions[source] != usize::MAX && index.positions[target] != usize::MAX
            })
        });
        let source_edges = unique.iter().map(|(raw, _)| *raw).collect();
        let endpoints: Vec<_> = unique
            .iter()
            .map(|(raw, _)| {
                let (source, target) = source.endpoints[*raw].expect("known endpoints");
                (index.positions[source], index.positions[target])
            })
            .collect();
        let edges: Vec<_> = unique.into_iter().map(|(_, edge)| edge).collect();
        let (parents, incoming) = Adjacency::pair(nodes.len(), &endpoints, true);
        let (children, outgoing) = Adjacency::pair(nodes.len(), &endpoints, false);
        let data = GraphData {
            node_ranks,
            source_nodes,
            source_edges,
            endpoints,
            index,
            parents,
            children,
            incoming,
            outgoing,
        };
        Graph {
            nodes,
            edges,
            data: Cow::Owned(data),
        }
    }

    pub fn prepared(
        nodes: Vec<&'a TopologyNode>,
        edges: Vec<&'a TopologyEdge>,
        data: &'a GraphData,
    ) -> Self {
        Self {
            nodes,
            edges,
            data: Cow::Borrowed(data),
        }
    }

    pub fn into_data(self) -> GraphData {
        self.data.into_owned()
    }
}

pub fn kind_rank(kind: &str) -> i32 {
    match kind {
        "Deployment" | "DaemonSet" | "StatefulSet" | "CronJob" | "Service" | "Ingress"
        | "ConfigMap" | "Secret" => 0,
        "ReplicaSet" | "Job" => 1,
        "EndpointSlice" | "Pod" => 2,
        "PersistentVolumeClaim" => 3,
        _ => 9,
    }
}

fn created_at_ms(node: &TopologyNode) -> Option<i64> {
    let created = node.summary.created_at.as_deref()?;
    chrono::DateTime::parse_from_rfc3339(created)
        .ok()
        .map(|time| time.timestamp_millis())
}

pub(super) fn node_ranks(nodes: &[TopologyNode]) -> Vec<usize> {
    let keys: Vec<_> = nodes.iter().map(NodeKey::new).collect();
    let mut sorted: Vec<_> = (0..nodes.len()).collect();
    sorted.sort_unstable_by(|&a, &b| keys[a].cmp(&keys[b]));
    let mut ranks = vec![0; nodes.len()];
    let mut rank = 0;
    for (at, &node) in sorted.iter().enumerate() {
        if at > 0 && keys[sorted[at - 1]] != keys[node] {
            rank += 1;
        }
        ranks[node] = rank;
    }
    ranks
}

pub(super) fn local_key(node: &TopologyNode) -> String {
    let created = match created_at_ms(node) {
        Some(ms) => format!("0:{:016}", 9_007_199_254_740_991_i64 - ms),
        None => "1:".to_owned(),
    };
    [
        format!("{:02}", kind_rank(&node.kind)),
        node.namespace.clone().unwrap_or_default(),
        node.kind.clone(),
        created,
        node.name.clone(),
        node.id.clone(),
    ]
    .join("|")
}

/// `sortTopologyNodes`, with timestamps and collation decoded once per node.
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct NodeKey<'a> {
    rank: i32,
    namespace: Key<'a>,
    kind: Key<'a>,
    // Newest first; resources without a timestamp sort last.
    created: Reverse<Option<i64>>,
    name: Key<'a>,
}

impl<'a> NodeKey<'a> {
    fn new(node: &'a TopologyNode) -> Self {
        Self {
            rank: kind_rank(&node.kind),
            namespace: Key::new(node.namespace.as_deref().unwrap_or(""), false),
            kind: Key::new(&node.kind, false),
            created: Reverse(created_at_ms(node)),
            name: Key::new(&node.name, true),
        }
    }
}

/// `topologyColumnDepth` followed by `compactDirectEdgeDepths`.
pub fn depths(graph: &Graph) -> Vec<i32> {
    /// A node whose parents are being resolved, and the deepest parent edge so far.
    struct Frame {
        node: usize,
        next: usize,
        edge_depth: i32,
    }
    let rank = |node: usize| kind_rank(&graph.nodes[node].kind);
    let count = graph.nodes.len();
    let mut depth: Vec<Option<i32>> = vec![None; count];
    let mut visiting = vec![false; count];
    // The web recurses once per owner link. A cluster sets no limit on a chain's length, so
    // the nodes being resolved are kept here instead of on the stack.
    let mut pending: Vec<Frame> = Vec::new();
    for start in 0..count {
        if depth[start].is_some() {
            continue;
        }
        visiting[start] = true;
        pending.push(Frame {
            node: start,
            next: 0,
            edge_depth: 0,
        });
        while let Some(frame) = pending.last_mut() {
            let node = frame.node;
            if let Some(&parent) = graph.parents[node].get(frame.next) {
                frame.next += 1;
                let parent_depth = if let Some(cached) = depth[parent] {
                    cached
                } else if visiting[parent] {
                    // A cycle: the node met again counts as its kind's column.
                    rank(parent)
                } else {
                    visiting[parent] = true;
                    pending.push(Frame {
                        node: parent,
                        next: 0,
                        edge_depth: 0,
                    });
                    continue;
                };
                frame.edge_depth = frame.edge_depth.max(parent_depth + 1);
                continue;
            }
            let resolved = if graph.parents[node].is_empty() {
                0
            } else {
                frame.edge_depth.max(rank(node))
            };
            pending.pop();
            depth[node] = Some(resolved);
            visiting[node] = false;
            if let Some(child) = pending.last_mut() {
                child.edge_depth = child.edge_depth.max(resolved + 1);
            }
        }
    }
    let column: Vec<i32> = depth.into_iter().map(Option::unwrap_or_default).collect();
    let mut compacted = column.clone();
    let mut by_depth: Vec<usize> = (0..count).collect();
    by_depth.sort_by_key(|&node| column[node]);
    for node in by_depth {
        if graph.parents[node].is_empty() {
            continue;
        }
        let direct = graph.parents[node]
            .iter()
            .map(|&parent| compacted[parent] + 1)
            .fold(0, i32::max);
        if direct < compacted[node] {
            compacted[node] = direct;
        }
    }
    compacted
}

/// `compareNodesByLayoutOrder`.
pub fn compare(graph: &Graph, depth: &[i32], order: &[usize], a: usize, b: usize) -> Ordering {
    depth[a]
        .cmp(&depth[b])
        .then_with(|| order[a].cmp(&order[b]))
        .then_with(|| graph.node_ranks[a].cmp(&graph.node_ranks[b]))
}

/// `buildTopologyPositions`: each node's top-left corner.
pub fn positions(graph: &Graph, depth: &[i32], order: &[usize]) -> Vec<Option<(f32, f32)>> {
    let count = graph.nodes.len();
    let cmp = |a: &usize, b: &usize| compare(graph, depth, order, *a, *b);
    let mut primary_children: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut is_primary_child = vec![false; count];
    for (node, primary) in is_primary_child.iter_mut().enumerate() {
        if let Some(&parent) = graph.parents[node].iter().min_by(|a, b| cmp(a, b)) {
            primary_children[parent].push(node);
            *primary = true;
        }
    }
    for children in &mut primary_children {
        children.sort_by(cmp);
    }
    let mut roots: Vec<usize> = (0..count).filter(|&node| !is_primary_child[node]).collect();
    roots.sort_by(cmp);
    let mut sorted: Vec<usize> = (0..count).collect();
    sorted.sort_by(cmp);
    let ordered_roots: Vec<usize> = if roots.is_empty() {
        sorted.clone()
    } else {
        let (owned, standalone): (Vec<usize>, Vec<usize>) = roots
            .into_iter()
            .partition(|&root| !primary_children[root].is_empty());
        owned.into_iter().chain(standalone).collect()
    };

    /// A node whose children are being placed.
    struct Frame {
        node: usize,
        top: f32,
        /// The next child to place.
        at: usize,
        /// Where the next child starts.
        next: f32,
        first: Option<f32>,
        last: Option<f32>,
    }
    struct Placer<'p> {
        primary_children: &'p [Vec<usize>],
        x: Vec<f32>,
        positions: Vec<Option<(f32, f32)>>,
        visited: Vec<bool>,
        visiting: Vec<bool>,
        /// The web recurses once per owner link; see `depths`.
        pending: Vec<Frame>,
    }
    const STEP: f32 = NODE_HEIGHT + ROW_GAP;
    impl Placer<'_> {
        /// Places `node` and its subtree from `top` down, and returns where the next
        /// subtree starts.
        fn place(&mut self, node: usize, top: f32) -> f32 {
            let mut placed = self.begin(node, top);
            loop {
                if let Some(end) = placed.take() {
                    let Some(frame) = self.pending.last_mut() else {
                        return end;
                    };
                    let child = self.primary_children[frame.node][frame.at - 1];
                    let before = std::mem::replace(&mut frame.next, end);
                    let child_y = self.positions[child].map_or(before, |(_, y)| y);
                    frame.first.get_or_insert(child_y);
                    frame.last = Some(child_y);
                    if !self.primary_children[child].is_empty() {
                        frame.next += SUBTREE_GAP;
                    }
                }
                let frame = self.pending.last_mut().expect("a node is being placed");
                if let Some(&child) = self.primary_children[frame.node].get(frame.at) {
                    frame.at += 1;
                    let top = frame.next;
                    placed = self.begin(child, top);
                    continue;
                }
                let frame = self.pending.pop().expect("a node is being placed");
                let y = match (frame.first, frame.last) {
                    (Some(first), Some(last)) => (first + last) / 2.,
                    _ => frame.top,
                };
                self.positions[frame.node] = Some((self.x[frame.node], y));
                self.visited[frame.node] = true;
                self.visiting[frame.node] = false;
                placed = Some(frame.next.max(y + STEP));
            }
        }

        /// Starts placing `node`. A node already placed, or met again inside its own
        /// subtree, is done at once and returns where the next subtree starts.
        fn begin(&mut self, node: usize, top: f32) -> Option<f32> {
            if self.visited[node] {
                return Some(top);
            }
            if self.visiting[node] {
                self.positions[node] = Some((self.x[node], top));
                self.visited[node] = true;
                return Some(top + STEP);
            }
            self.visiting[node] = true;
            self.pending.push(Frame {
                node,
                top,
                at: 0,
                next: top,
                first: None,
                last: None,
            });
            None
        }
    }
    let mut placer = Placer {
        primary_children: &primary_children,
        x: depth
            .iter()
            .map(|&column| CANVAS_PADDING + column as f32 * (NODE_WIDTH + COLUMN_GAP))
            .collect(),
        positions: vec![None; count],
        visited: vec![false; count],
        // The web passes a fresh `visiting` set per root; a finished root clears its marks.
        visiting: vec![false; count],
        pending: Vec::new(),
    };
    let mut cursor = CANVAS_PADDING;
    for root in ordered_roots {
        let before = cursor;
        cursor = placer.place(root, cursor);
        let gap = if primary_children[root].is_empty() {
            STANDALONE_ROOT_GAP
        } else {
            ROOT_GROUP_GAP
        };
        if cursor > before {
            cursor += gap;
        }
    }
    for node in sorted {
        if !placer.visited[node] {
            cursor = placer.place(node, cursor) + ROOT_GROUP_GAP;
        }
    }
    placer.positions
}

/// A `standaloneKindGroup` node: ownerless resources of one kind.
pub struct Group {
    pub kind: String,
    pub members: Vec<usize>,
    pub expanded: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Standalone {
    pub groups: Vec<Group>,
    pub is_standalone: Vec<bool>,
    /// Absolute positions of members shown inside expanded groups.
    pub member_positions: HashMap<usize, (f32, f32)>,
}

/// `buildStandaloneGroups` for the ownership mode.
pub fn standalone_groups(
    graph: &Graph,
    positions: &[Option<(f32, f32)>],
    cmp: impl Fn(usize, usize) -> Ordering,
    expanded_kinds: &HashSet<String>,
    selected: Option<&str>,
) -> Standalone {
    let count = graph.nodes.len();
    let mut members: Vec<usize> = (0..count)
        .filter(|&node| graph.parents[node].is_empty() && graph.children[node].is_empty())
        .collect();
    members.sort_by(|&a, &b| cmp(a, b));
    let mut is_standalone = vec![false; count];
    for &node in &members {
        is_standalone[node] = true;
    }
    let owned_bottom = (0..count)
        .filter(|&node| !is_standalone[node])
        .filter_map(|node| positions[node])
        .map(|(_, y)| y + NODE_HEIGHT)
        .reduce(f32::max)
        .unwrap_or(CANVAS_PADDING - GROUP_GAP);
    let mut by_kind: Vec<(String, Vec<usize>)> = Vec::new();
    for node in members {
        let kind = &graph.nodes[node].kind;
        match by_kind.iter_mut().find(|(existing, _)| existing == kind) {
            Some((_, bucket)) => bucket.push(node),
            None => by_kind.push((kind.clone(), vec![node])),
        }
    }
    by_kind.sort_by(|(a, _), (b, _)| {
        kind_rank(a)
            .cmp(&kind_rank(b))
            .then_with(|| collate(a, b, false))
    });
    let mut groups = Vec::new();
    let mut member_positions = HashMap::new();
    let mut cursor = owned_bottom + GROUP_GAP;
    for (kind, nodes) in by_kind {
        let expanded = expanded_kinds.contains(&kind)
            || nodes
                .iter()
                .any(|&node| Some(graph.nodes[node].id.as_str()) == selected);
        let columns = nodes.len().clamp(1, GROUP_COLUMNS);
        let rows = if expanded {
            nodes.len().div_ceil(columns)
        } else {
            0
        };
        let (width, height) = if expanded {
            (
                GROUP_PADDING * 2.
                    + columns as f32 * STANDALONE_NODE_WIDTH
                    + (columns - 1) as f32 * GROUP_COLUMN_GAP,
                GROUP_HEADER_HEIGHT
                    + GROUP_PADDING * 2.
                    + rows as f32 * NODE_HEIGHT
                    + rows.saturating_sub(1) as f32 * GROUP_ROW_GAP,
            )
        } else {
            let digits = nodes.len().to_string().len() as f32;
            (
                (156. + digits * 8.).clamp(190., 360.),
                GROUP_COLLAPSED_HEIGHT,
            )
        };
        if expanded {
            for (at, &node) in nodes.iter().enumerate() {
                let (column, row) = ((at % columns) as f32, (at / columns) as f32);
                member_positions.insert(
                    node,
                    (
                        CANVAS_PADDING
                            + GROUP_PADDING
                            + column * (STANDALONE_NODE_WIDTH + GROUP_COLUMN_GAP),
                        cursor
                            + GROUP_HEADER_HEIGHT
                            + GROUP_PADDING
                            + row * (NODE_HEIGHT + GROUP_ROW_GAP),
                    ),
                );
            }
        }
        groups.push(Group {
            kind,
            members: nodes,
            expanded,
            x: CANVAS_PADDING,
            y: cursor,
            width,
            height,
        });
        cursor += height + GROUP_GAP;
    }
    Standalone {
        groups,
        is_standalone,
        member_positions,
    }
}
