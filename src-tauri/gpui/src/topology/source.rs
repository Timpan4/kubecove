//! Immutable topology projection and reusable filtering/layout work buffers.

use super::{layout, loose_key};
use crate::{collate::Encoded, model::Visible};
use kubecove_lib::models::ResourceTopology;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;

pub struct Source {
    rows: Rc<()>,
    pub(super) index: Arc<HashMap<String, usize>>,
    pub(super) edge_positions: Vec<usize>,
    pub(super) edge_count: usize,
    pub(super) canonical: Vec<usize>,
    matches: Vec<Vec<usize>>,
    parents: Vec<Vec<usize>>,
    pub(super) endpoints: Vec<Option<(usize, usize)>>,
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
        let mut row_index: HashMap<_, Vec<usize>> = HashMap::new();
        for (i, row) in rows.all_rows().iter().enumerate() {
            row_index.entry(loose_key(row)).or_default().push(i);
        }
        let matches = topology
            .nodes
            .iter()
            .map(|node| {
                row_index
                    .get(&loose_key(&node.summary))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        let mut parents = vec![Vec::new(); topology.nodes.len()];
        let edge_index: HashMap<_, _> = topology
            .edges
            .iter()
            .enumerate()
            .map(|(i, edge)| (edge.id.as_str(), i))
            .collect();
        let endpoints = topology
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
        let mut source = Self {
            rows: rows.source().identity().clone(),
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
        Rc::ptr_eq(&self.rows, rows.source().identity())
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
    /// `filterTopologyByTableRows` does on the web.
    pub(super) fn filter(
        &self,
        rows: &Visible,
        links: Option<&[bool]>,
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
        for (i, matches) in self.matches.iter().enumerate() {
            let canonical = self.canonical[i];
            if !kept[canonical] && matches.iter().any(|&row| rows.contains_index(row)) {
                kept[canonical] = true;
                matched[canonical] = true;
                pending.push(canonical);
            }
        }
        if let Some(links) = links {
            for &(source, target) in self.endpoints.iter().flatten() {
                kept[target] |= matched[source] && links[target];
                kept[source] |= matched[target] && links[source];
            }
        } else {
            while let Some(node) = pending.pop() {
                for &parent in &self.parents[node] {
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
