//! Ownership-map layout, ported from `topology-graph.ts`, `topology-layout.ts`
//! and `topology-standalone-groups.ts`. Nodes are addressed by their index in
//! `Graph::nodes`.

use crate::collate::collate;
use kubecove_lib::models::{TopologyEdge, TopologyNode};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

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
    pub index: HashMap<&'a str, usize>,
    pub edges: Vec<&'a TopologyEdge>,
    pub parents: Vec<Vec<usize>>,
    pub children: Vec<Vec<usize>>,
    /// Edge indices per node.
    pub incoming: Vec<Vec<usize>>,
    pub outgoing: Vec<Vec<usize>>,
}

fn dedup<'a, T>(items: impl Iterator<Item = &'a T>, id: impl Fn(&T) -> &str) -> Vec<&'a T> {
    let mut position: HashMap<&str, usize> = HashMap::new();
    let mut unique: Vec<&'a T> = Vec::new();
    for item in items {
        match position.get(id(item)) {
            Some(&at) => unique[at] = item,
            None => {
                position.insert(id(item), unique.len());
                unique.push(item);
            }
        }
    }
    unique
}

impl<'a> Graph<'a> {
    pub fn new(
        nodes: impl Iterator<Item = &'a TopologyNode>,
        edges: impl Iterator<Item = &'a TopologyEdge>,
    ) -> Self {
        let nodes = dedup(nodes, |node| &node.id);
        let index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(at, node)| (node.id.as_str(), at))
            .collect();
        let edges: Vec<_> = dedup(edges, |edge| &edge.id)
            .into_iter()
            .filter(|edge| {
                index.contains_key(edge.source.as_str()) && index.contains_key(edge.target.as_str())
            })
            .collect();
        let count = nodes.len();
        let mut graph = Graph {
            nodes,
            index,
            edges,
            parents: vec![Vec::new(); count],
            children: vec![Vec::new(); count],
            incoming: vec![Vec::new(); count],
            outgoing: vec![Vec::new(); count],
        };
        for (at, edge) in graph.edges.iter().enumerate() {
            let source = graph.index[edge.source.as_str()];
            let target = graph.index[edge.target.as_str()];
            graph.parents[target].push(source);
            graph.children[source].push(target);
            graph.incoming[target].push(at);
            graph.outgoing[source].push(at);
        }
        graph
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

/// `sortTopologyNodes`.
fn sort_nodes(a: &TopologyNode, b: &TopologyNode) -> Ordering {
    kind_rank(&a.kind)
        .cmp(&kind_rank(&b.kind))
        .then_with(|| {
            collate(
                a.namespace.as_deref().unwrap_or(""),
                b.namespace.as_deref().unwrap_or(""),
                false,
            )
        })
        .then_with(|| collate(&a.kind, &b.kind, false))
        // Newest first; resources without a timestamp sort last.
        .then_with(|| match (created_at_ms(a), created_at_ms(b)) {
            (Some(a), Some(b)) => b.cmp(&a),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| collate(&a.name, &b.name, true))
}

/// `topologyColumnDepth` followed by `compactDirectEdgeDepths`.
pub fn depths(graph: &Graph) -> Vec<i32> {
    fn resolve(
        graph: &Graph,
        node: usize,
        depth: &mut [Option<i32>],
        visiting: &mut [bool],
    ) -> i32 {
        if let Some(cached) = depth[node] {
            return cached;
        }
        let rank = kind_rank(&graph.nodes[node].kind);
        if visiting[node] {
            return rank;
        }
        visiting[node] = true;
        let resolved = if graph.parents[node].is_empty() {
            0
        } else {
            let mut edge_depth = 0;
            for &parent in &graph.parents[node] {
                edge_depth = edge_depth.max(resolve(graph, parent, depth, visiting) + 1);
            }
            edge_depth.max(rank)
        };
        depth[node] = Some(resolved);
        visiting[node] = false;
        resolved
    }
    let count = graph.nodes.len();
    let mut depth = vec![None; count];
    let mut visiting = vec![false; count];
    for node in 0..count {
        resolve(graph, node, &mut depth, &mut visiting);
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

/// `topologyLayoutOrder`: a key that keeps each subtree under its root.
pub fn order_keys(graph: &Graph, depth: &[i32]) -> Vec<String> {
    let count = graph.nodes.len();
    let mut roots: Vec<usize> = (0..count)
        .filter(|&node| graph.parents[node].is_empty())
        .collect();
    roots.sort_by(|&a, &b| sort_nodes(graph.nodes[a], graph.nodes[b]));
    let mut root_order = vec![roots.len(); count];
    for (at, &root) in roots.iter().enumerate() {
        root_order[root] = at;
    }
    let local_key = |node: usize| {
        let node = graph.nodes[node];
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
    };
    struct Resolver<'g, 'a, F> {
        graph: &'g Graph<'a>,
        depth: &'g [i32],
        root_order: Vec<usize>,
        local_key: F,
        memo: Vec<Option<String>>,
        visiting: Vec<bool>,
    }
    impl<F: Fn(usize) -> String> Resolver<'_, '_, F> {
        fn resolve(&mut self, node: usize) -> String {
            if let Some(cached) = &self.memo[node] {
                return cached.clone();
            }
            if self.visiting[node] {
                return (self.local_key)(node);
            }
            self.visiting[node] = true;
            let mut parents = self.graph.parents[node].clone();
            parents.sort_by(|&a, &b| sort_nodes(self.graph.nodes[a], self.graph.nodes[b]));
            // JavaScript's default `sort()` orders the parent keys by code unit.
            let parent_key = parents
                .into_iter()
                .map(|parent| self.resolve(parent))
                .min()
                .unwrap_or_else(|| format!("{:04}", self.root_order[node]));
            let key = format!(
                "{parent_key}>{:02}>{}",
                self.depth[node],
                (self.local_key)(node)
            );
            self.memo[node] = Some(key.clone());
            self.visiting[node] = false;
            key
        }
    }
    let mut resolver = Resolver {
        graph,
        depth,
        root_order,
        local_key,
        memo: vec![None; count],
        visiting: vec![false; count],
    };
    (0..count).map(|node| resolver.resolve(node)).collect()
}

/// `compareNodesByLayoutOrder`.
pub fn compare(graph: &Graph, depth: &[i32], order: &[String], a: usize, b: usize) -> Ordering {
    depth[a]
        .cmp(&depth[b])
        .then_with(|| collate(&order[a], &order[b], false))
        .then_with(|| sort_nodes(graph.nodes[a], graph.nodes[b]))
}

/// `buildTopologyPositions`: each node's top-left corner.
pub fn positions(graph: &Graph, depth: &[i32], order: &[String]) -> Vec<Option<(f32, f32)>> {
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

    struct Placer<'p> {
        primary_children: &'p [Vec<usize>],
        x: Vec<f32>,
        positions: Vec<Option<(f32, f32)>>,
        visited: Vec<bool>,
        visiting: Vec<bool>,
    }
    const STEP: f32 = NODE_HEIGHT + ROW_GAP;
    impl Placer<'_> {
        fn place(&mut self, node: usize, top: f32) -> f32 {
            if self.visited[node] {
                return top;
            }
            if self.visiting[node] {
                self.positions[node] = Some((self.x[node], top));
                self.visited[node] = true;
                return top + STEP;
            }
            self.visiting[node] = true;
            let mut next = top;
            let (mut first, mut last) = (None, None);
            for &child in self.primary_children[node].iter() {
                let before = next;
                next = self.place(child, next);
                let child_y = self.positions[child].map_or(before, |(_, y)| y);
                first.get_or_insert(child_y);
                last = Some(child_y);
                if !self.primary_children[child].is_empty() {
                    next += SUBTREE_GAP;
                }
            }
            let y = match (first, last) {
                (Some(first), Some(last)) => (first + last) / 2.,
                _ => top,
            };
            self.positions[node] = Some((self.x[node], y));
            self.visited[node] = true;
            self.visiting[node] = false;
            next.max(y + STEP)
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
