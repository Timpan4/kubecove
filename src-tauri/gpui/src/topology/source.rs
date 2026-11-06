//! Immutable topology projection and reusable filtering/layout work buffers.

use super::{layout, loose_key};
use crate::{collate::Encoded, model::Visible};
use kubecove_lib::models::ResourceTopology;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

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
    filter: RefCell<Filter>,
}

#[derive(Default)]
struct Filter {
    kept: Vec<bool>,
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
        Self {
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
            filter: RefCell::default(),
        }
    }

    pub fn matches(&self, rows: &Visible) -> bool {
        Rc::ptr_eq(&self.rows, rows.source().identity())
    }
    pub(super) fn rank(&self, index: usize) -> usize {
        self.ranks[index]
    }

    pub(super) fn filter(&self, rows: &Visible) -> (Vec<usize>, Vec<usize>) {
        let mut scratch = self.filter.borrow_mut();
        scratch.kept.resize(self.canonical.len(), false);
        scratch.kept.fill(false);
        scratch.pending.clear();
        for (i, matches) in self.matches.iter().enumerate() {
            let canonical = self.canonical[i];
            if !scratch.kept[canonical] && matches.iter().any(|&row| rows.contains_index(row)) {
                scratch.kept[canonical] = true;
                scratch.pending.push(canonical);
            }
        }
        while let Some(node) = scratch.pending.pop() {
            for &parent in &self.parents[node] {
                if !std::mem::replace(&mut scratch.kept[parent], true) {
                    scratch.pending.push(parent);
                }
            }
        }
        (
            (0..self.canonical.len())
                .filter(|&i| scratch.kept[self.canonical[i]])
                .collect(),
            self.endpoints
                .iter()
                .enumerate()
                .filter(|(_, ends)| {
                    ends.is_some_and(|(source, target)| {
                        scratch.kept[source] && scratch.kept[target]
                    })
                })
                .map(|(i, _)| i)
                .collect(),
        )
    }
}
