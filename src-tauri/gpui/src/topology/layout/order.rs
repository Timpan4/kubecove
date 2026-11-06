//! Layout path keys assembled from prepared segments in reusable buffers.

use super::Graph;
use crate::collate::Encoded;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct Scratch {
    roots: Vec<usize>,
    root_order: Vec<usize>,
    prefixes: Vec<Encoded>,
    parents: Vec<Vec<usize>>,
    keys: Vec<Encoded>,
    memo: Vec<bool>,
    visiting: Vec<bool>,
    sorted: Vec<usize>,
}

#[derive(Clone, Copy)]
enum Reference {
    Local(usize),
    Path(usize),
    Root(usize),
}

struct Resolver<'a> {
    depth: &'a [i32],
    raw: &'a [usize],
    locals: &'a [Encoded],
    scratch: &'a mut Scratch,
}

impl Resolver<'_> {
    fn key(&self, key: Reference) -> &Encoded {
        match key {
            Reference::Local(node) => &self.locals[self.raw[node]],
            Reference::Path(node) => &self.scratch.keys[self.raw[node]],
            Reference::Root(root) => &self.scratch.prefixes[root],
        }
    }

    fn resolve(&mut self, node: usize) -> Reference {
        if self.scratch.memo[node] {
            return Reference::Path(node);
        }
        if self.scratch.visiting[node] {
            return Reference::Local(node);
        }
        self.scratch.visiting[node] = true;
        let mut best: Option<Reference> = None;
        for at in 0..self.scratch.parents[node].len() {
            let parent = self.scratch.parents[node][at];
            let candidate = self.resolve(parent);
            if best.is_none_or(|current| self.key(candidate).text < self.key(current).text) {
                best = Some(candidate);
            }
        }
        let parent = best.unwrap_or(Reference::Root(self.scratch.root_order[node]));
        let mut key = std::mem::take(&mut self.scratch.keys[self.raw[node]]);
        key.join(
            self.key(parent),
            self.depth[node],
            &self.locals[self.raw[node]],
        );
        self.scratch.keys[self.raw[node]] = key;
        self.scratch.memo[node] = true;
        self.scratch.visiting[node] = false;
        Reference::Path(node)
    }
}

pub(crate) fn order_keys(
    graph: &Graph,
    depth: &[i32],
    raw: &[usize],
    locals: &[Encoded],
    scratch: &mut Scratch,
) -> Vec<usize> {
    let count = graph.nodes.len();
    scratch.roots.clear();
    scratch
        .roots
        .extend((0..count).filter(|&node| graph.parents[node].is_empty()));
    scratch.roots.sort_by_key(|&node| graph.node_ranks[node]);
    scratch.root_order.resize(count, scratch.roots.len());
    scratch.root_order.fill(scratch.roots.len());
    for (at, &node) in scratch.roots.iter().enumerate() {
        scratch.root_order[node] = at;
    }
    for root in scratch.prefixes.len()..=scratch.roots.len() {
        scratch.prefixes.push(Encoded::new(format!("{root:04}")));
    }
    if scratch.parents.len() < count {
        scratch.parents.resize_with(count, Vec::new);
    }
    if scratch.keys.len() < locals.len() {
        scratch.keys.resize_with(locals.len(), Encoded::default);
    }
    scratch.memo.resize(count, false);
    scratch.memo.fill(false);
    scratch.visiting.resize(count, false);
    scratch.visiting.fill(false);
    for node in 0..count {
        scratch.parents[node].clear();
        scratch.parents[node].extend_from_slice(&graph.parents[node]);
        scratch.parents[node].sort_by_key(|&parent| graph.node_ranks[parent]);
    }
    let mut resolver = Resolver {
        depth,
        raw,
        locals,
        scratch,
    };
    for node in 0..count {
        resolver.resolve(node);
    }
    let scratch = resolver.scratch;
    scratch.sorted.clear();
    scratch.sorted.extend(0..count);
    scratch
        .sorted
        .sort_unstable_by(|&a, &b| scratch.keys[raw[a]].cmp(&scratch.keys[raw[b]]));
    let mut ranks = vec![0; count];
    let mut rank = 0;
    for (at, &node) in scratch.sorted.iter().enumerate() {
        if at > 0 && scratch.keys[raw[scratch.sorted[at - 1]]] != scratch.keys[raw[node]] {
            rank += 1;
        }
        ranks[node] = rank;
    }
    ranks
}
