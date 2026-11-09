//! Immutable topology projection and reusable filtering/layout work buffers.

use super::{layout, loose_key};
use crate::{
    collate::Encoded,
    model::{RowsId, Visible},
};
use kubecove_lib::models::ResourceTopology;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;

pub struct Source {
    rows: RowsId,
    pub(super) index: Arc<HashMap<String, usize>>,
    pub(super) edge_positions: Vec<usize>,
    pub(super) edge_count: usize,
    pub(super) canonical: Vec<usize>,
    matches: Vec<Vec<usize>>,
    parents: Vec<Vec<usize>>,
    pub(super) endpoints: Vec<Option<(usize, usize)>>,
    /// Per canonical node: a historical ReplicaSet, which the toggle hides.
    historical: Vec<bool>,
    ranks: Vec<usize>,
    pub(super) locals: Vec<Encoded>,
    pub(super) layout: RefCell<layout::Scratch>,
    stable_layout: Option<StableLayout>,
    filter: RefCell<Filter>,
}

struct StableLayout {
    depth: Vec<i32>,
    order: Vec<usize>,
}

#[derive(Default)]
struct Filter {
    kept: Vec<bool>,
    matched: Vec<bool>,
    pending: Vec<usize>,
}

impl Source {
    pub fn new(topology: &ResourceTopology, rows: &Visible) -> Self {
        let index: HashMap<_, _> = topology
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.clone(), i))
            .collect();
        let canonical: Vec<_> = topology
            .nodes
            .iter()
            .map(|node| index[node.id.as_str()])
            .collect();
        // A refresh rebuilds this for every row of every workspace context, while
        // the graph has one context's nodes. Index the nodes and stream the rows
        // once, so a row costs a lookup and allocates nothing. The key starts with
        // the cluster, so a row from a cluster without nodes skips the hash.
        let mut node_keys: HashMap<_, Vec<usize>> = HashMap::with_capacity(topology.nodes.len());
        let mut clusters: Vec<&str> = Vec::new();
        for (i, node) in topology.nodes.iter().enumerate() {
            node_keys.entry(loose_key(&node.summary)).or_default().push(i);
            if !clusters.contains(&node.summary.cluster.as_str()) {
                clusters.push(&node.summary.cluster);
            }
        }
        // Rows carry their context as an index, so the check reads a flag, not the row.
        let prepared = rows.source();
        let with_nodes: Vec<bool> = prepared
            .context_names()
            .iter()
            .map(|context| clusters.contains(&context.as_str()))
            .collect();
        let mut matches: Vec<Vec<usize>> = vec![Vec::new(); topology.nodes.len()];
        for (i, (row, &context)) in rows
            .all_rows()
            .iter()
            .zip(prepared.row_contexts())
            .enumerate()
        {
            if !with_nodes[context as usize] {
                continue;
            }
            if let Some(nodes) = node_keys.get(&loose_key(row)) {
                for &node in nodes {
                    matches[node].push(i);
                }
            }
        }
        let mut parents = vec![Vec::new(); topology.nodes.len()];
        let edge_index: HashMap<_, _> = topology
            .edges
            .iter()
            .enumerate()
            .map(|(i, edge)| (edge.id.as_str(), i))
            .collect();
        let endpoints: Vec<_> = topology
            .edges
            .iter()
            .map(|edge| {
                let (&source, &target) = (
                    index.get(edge.source.as_str())?,
                    index.get(edge.target.as_str())?,
                );
                parents[target].push(source);
                Some((source, target))
            })
            .collect();
        let historical = historical_replicasets(topology, &parents, &endpoints);
        let mut source = Self {
            rows: rows.source().identity(),
            historical,
            ranks: layout::node_ranks(&topology.nodes),
            locals: topology
                .nodes
                .iter()
                .map(|node| Encoded::new(layout::local_key(node)))
                .collect(),
            index: Arc::new(index),
            canonical,
            matches,
            parents,
            endpoints,
            edge_positions: topology
                .edges
                .iter()
                .map(|edge| edge_index[edge.id.as_str()])
                .collect(),
            edge_count: topology.edges.len(),
            layout: RefCell::default(),
            stable_layout: None,
            filter: RefCell::default(),
        };
        // Raw duplicate edges can resolve to a different parent after filtering.
        if edge_index.len() == topology.edges.len() {
            source.stable_layout = StableLayout::new(topology, &source);
        }
        source
    }

    pub fn matches(&self, rows: &Visible) -> bool {
        self.rows == rows.source().identity()
    }
    pub(super) fn rank(&self, index: usize) -> usize {
        self.ranks[index]
    }

    /// `ancestor_closed` says every kept node's parents were kept, which the
    /// precomputed layout needs.
    pub(super) fn depth_and_order(
        &self,
        graph: &layout::Graph,
        ancestor_closed: bool,
    ) -> (Vec<i32>, Vec<usize>) {
        if let Some(stable) = self.stable_layout.as_ref().filter(|_| ancestor_closed) {
            return graph
                .source_nodes
                .iter()
                .map(|&i| (stable.depth[i], stable.order[i]))
                .unzip();
        }
        let depth_span = crate::frame_profile::scope("topology", "depth");
        let depth = layout::depths(graph);
        drop(depth_span);
        let _order_span = crate::frame_profile::scope("topology", "order");
        let order = layout::order_keys(
            graph,
            &depth,
            &graph.source_nodes,
            &self.locals,
            &mut self.layout.borrow_mut(),
        );
        (depth, order)
    }

    /// Nodes matching `rows` and their ancestors. With `links`, a per-node mask
    /// of kinds outside a narrowed kind selection, it keeps the matching nodes
    /// and the masked nodes one edge from them instead, as
    /// `filterTopologyByTableRows` does on the web. With `hide_historical`, the
    /// historical ReplicaSets and their edges are removed first, as
    /// `filterHistoricalReplicaSets` does on the web.
    pub(super) fn filter(
        &self,
        rows: &Visible,
        links: Option<&[bool]>,
        hide_historical: bool,
    ) -> (Vec<usize>, Vec<usize>) {
        let mut scratch = self.filter.borrow_mut();
        let Filter {
            kept,
            matched,
            pending,
        } = &mut *scratch;
        kept.resize(self.canonical.len(), false);
        kept.fill(false);
        matched.resize(self.canonical.len(), false);
        matched.fill(false);
        pending.clear();
        let hidden = |node: usize| hide_historical && self.historical[node];
        for (i, matches) in self.matches.iter().enumerate() {
            let canonical = self.canonical[i];
            if !kept[canonical]
                && !hidden(canonical)
                && matches.iter().any(|&row| rows.contains_index(row))
            {
                kept[canonical] = true;
                matched[canonical] = true;
                pending.push(canonical);
            }
        }
        if let Some(links) = links {
            for &(source, target) in self.endpoints.iter().flatten() {
                if hidden(source) || hidden(target) {
                    continue;
                }
                kept[target] |= matched[source] && links[target];
                kept[source] |= matched[target] && links[source];
            }
        } else {
            while let Some(node) = pending.pop() {
                for &parent in &self.parents[node] {
                    if hidden(parent) {
                        continue;
                    }
                    if !std::mem::replace(&mut kept[parent], true) {
                        pending.push(parent);
                    }
                }
            }
        }
        (
            (0..self.canonical.len())
                .filter(|&i| kept[self.canonical[i]])
                .collect(),
            self.endpoints
                .iter()
                .enumerate()
                .filter(|(_, ends)| {
                    ends.is_some_and(|(source, target)| kept[source] && kept[target])
                })
                .map(|(i, _)| i)
                .collect(),
        )
    }
}

/// `filterHistoricalReplicaSets`: a ReplicaSet with no Pod children whose
/// revision is below its Deployment parent's revision. Indexed like `endpoints`.
fn historical_replicasets(
    topology: &ResourceTopology,
    parents: &[Vec<usize>],
    endpoints: &[Option<(usize, usize)>],
) -> Vec<bool> {
    let nodes = &topology.nodes;
    let mut owns_pods = vec![false; nodes.len()];
    for &(source, target) in endpoints.iter().flatten() {
        if nodes[target].kind == "Pod" {
            owns_pods[source] = true;
        }
    }
    (0..nodes.len())
        .map(|index| {
            let node = &nodes[index];
            let Some(revision) = node.deployment_revision else {
                return false;
            };
            if node.kind != "ReplicaSet" || owns_pods[index] {
                return false;
            }
            parents[index]
                .iter()
                .map(|&parent| &nodes[parent])
                .find(|parent| parent.kind == "Deployment")
                .and_then(|deployment| deployment.deployment_revision)
                .is_some_and(|deployment| revision < deployment)
        })
        .collect()
}

impl StableLayout {
    fn new(topology: &ResourceTopology, source: &Source) -> Option<Self> {
        let graph = layout::Graph::ranked(
            topology.nodes.iter().enumerate(),
            topology.edges.iter().enumerate(),
            source,
        );
        let mut incoming: Vec<_> = (0..graph.nodes.len())
            .map(|i| graph.parents[i].len())
            .collect();
        let mut pending: Vec<_> = (0..graph.nodes.len())
            .filter(|&i| incoming[i] == 0)
            .collect();
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            for &child in &graph.children[node] {
                incoming[child] -= 1;
                if incoming[child] == 0 {
                    pending.push(child);
                }
            }
        }
        if visited != graph.nodes.len() {
            return None;
        }
        // Ancestor-closed filtering leaves DAG depths and relative path order
        // unchanged. Placement still depends on the remaining children.
        let depth = layout::depths(&graph);
        let mut scratch = source.layout.borrow_mut();
        let order = layout::order_keys(
            &graph,
            &depth,
            &graph.source_nodes,
            &source.locals,
            &mut scratch,
        );
        if !scratch.preserves_root_order() {
            return None;
        }
        let mut stable = Self {
            depth: vec![0; topology.nodes.len()],
            order: vec![0; topology.nodes.len()],
        };
        for (i, &raw) in graph.source_nodes.iter().enumerate() {
            stable.depth[raw] = depth[i];
            stable.order[raw] = order[i];
        }
        Some(stable)
    }
}
