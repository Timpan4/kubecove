//! Layout path keys, compared without writing them out.
//!
//! A node's key is its parent's key, then `>depth>`, then its own local key. Written out,
//! every key repeats its ancestors' text, so a chain of owners costs memory with the square
//! of its length, and a cluster sets no limit on that length. A key is therefore kept as a
//! link to the key it extends, and two keys are compared from the first path node they do
//! not share. Both the walk that picks parents and the comparison use their own stacks and
//! tables, never one stack frame per link.

use super::Graph;
use crate::collate::{Encoded, tie_chars};
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

const NONE: usize = usize::MAX;

#[derive(Default)]
pub(crate) struct Scratch {
    roots: Vec<usize>,
    root_order: Vec<usize>,
    prefixes: Vec<Encoded>,
    /// `>depth>` for each depth.
    separators: Vec<Encoded>,
    parents: Vec<Vec<usize>>,
    /// The key each node's key extends.
    links: Vec<Reference>,
    /// The number of nodes on each node's path, itself included.
    lengths: Vec<usize>,
    /// `ancestors[node * levels + level]` is the node `2^level` links up the path, or `NONE`.
    ancestors: Vec<usize>,
    levels: usize,
    memo: Vec<bool>,
    visiting: Vec<bool>,
    pending: Vec<Frame>,
    sorted: Vec<usize>,
}

impl Scratch {
    pub(crate) fn preserves_root_order(&self) -> bool {
        self.prefixes[..=self.roots.len()]
            .windows(2)
            .all(|pair| pair[0].text < pair[1].text)
    }
}

#[derive(Clone, Copy)]
enum Reference {
    /// The node's local key alone: a node met again while its own key is being resolved.
    Local(usize),
    /// The node's whole key.
    Path(usize),
    Root(usize),
}

/// A node whose parents are being resolved, and the first key among those seen so far.
struct Frame {
    node: usize,
    next: usize,
    best: Option<Reference>,
}

/// The resolved keys, for comparing.
#[derive(Clone, Copy)]
struct Paths<'a> {
    depth: &'a [i32],
    raw: &'a [usize],
    locals: &'a [Encoded],
    prefixes: &'a [Encoded],
    separators: &'a [Encoded],
    links: &'a [Reference],
    lengths: &'a [usize],
    ancestors: &'a [usize],
    levels: usize,
}

/// The parts of one key in order, from a given path node on.
#[derive(Clone)]
struct Pieces<'a> {
    paths: Paths<'a>,
    start: Option<&'a Encoded>,
    /// The last node of the path, and how many of its nodes are still to come.
    end: usize,
    left: usize,
    local: Option<&'a Encoded>,
}

impl<'a> Iterator for Pieces<'a> {
    type Item = &'a Encoded;

    fn next(&mut self) -> Option<&'a Encoded> {
        if let Some(piece) = self.local.take().or_else(|| self.start.take()) {
            return Some(piece);
        }
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let paths = self.paths;
        let node = paths.ancestor(self.end, self.left);
        self.local = Some(&paths.locals[paths.raw[node]]);
        Some(&paths.separators[separator(paths.depth[node])])
    }
}

fn separator(depth: i32) -> usize {
    usize::try_from(depth).expect("layout depths are not negative")
}

/// Compares two sequences of byte runs as if each were joined into one run.
fn joined_cmp<'p>(
    mut a: impl Iterator<Item = &'p [u8]>,
    mut b: impl Iterator<Item = &'p [u8]>,
) -> Ordering {
    let (mut left, mut right): (&[u8], &[u8]) = (&[], &[]);
    loop {
        while left.is_empty() {
            match a.next() {
                Some(run) => left = run,
                None => break,
            }
        }
        while right.is_empty() {
            match b.next() {
                Some(run) => right = run,
                None => break,
            }
        }
        match (left.is_empty(), right.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }
        let common = left.len().min(right.len());
        match left[..common].cmp(&right[..common]) {
            Ordering::Equal => {
                left = &left[common..];
                right = &right[common..];
            }
            decided => return decided,
        }
    }
}

impl<'a> Paths<'a> {
    fn ancestor(&self, mut node: usize, mut up: usize) -> usize {
        let mut level = 0;
        while up > 0 {
            if up & 1 == 1 {
                node = self.ancestors[node * self.levels + level];
            }
            up >>= 1;
            level += 1;
        }
        node
    }

    /// How many leading path nodes two keys share. Shared nodes contribute equal parts.
    fn shared(&self, a: Reference, b: Reference) -> usize {
        let (Reference::Path(a), Reference::Path(b)) = (a, b) else {
            return 0;
        };
        let length = self.lengths[a].min(self.lengths[b]);
        let mut a = self.ancestor(a, self.lengths[a] - length);
        let mut b = self.ancestor(b, self.lengths[b] - length);
        if a == b {
            return length;
        }
        // Climb both paths while the nodes above differ. Past a path's first node both
        // entries are `NONE`, so two paths with nothing in common stop at their first nodes.
        let levels = (usize::BITS - (length - 1).leading_zeros()) as usize;
        for level in (0..levels).rev() {
            let above_a = self.ancestors[a * self.levels + level];
            let above_b = self.ancestors[b * self.levels + level];
            if above_a != above_b {
                a = above_a;
                b = above_b;
            }
        }
        self.lengths[a] - 1
    }

    /// The parts of `key` after its first `skip` path nodes. With none skipped, the key's
    /// start comes first: a root's prefix or the local key of a node met again.
    fn pieces(&self, key: Reference, skip: usize) -> Pieces<'a> {
        let start = |link: Reference| match link {
            Reference::Local(node) => &self.locals[self.raw[node]],
            Reference::Root(root) => &self.prefixes[root],
            Reference::Path(_) => unreachable!("a path starts at a root or a local key"),
        };
        let (start, end, left) = match key {
            Reference::Path(node) => {
                let length = self.lengths[node];
                let first = self.ancestor(node, length - 1);
                (
                    (skip == 0).then(|| start(self.links[first])),
                    node,
                    length - skip,
                )
            }
            other => (Some(start(other)), NONE, 0),
        };
        Pieces {
            paths: *self,
            start,
            end,
            left,
            local: None,
        }
    }

    /// The order of two keys' text by byte, which picks a node's parent key.
    fn text_cmp(&self, a: Reference, b: Reference) -> Ordering {
        let skip = self.shared(a, b);
        joined_cmp(
            self.pieces(a, skip).map(|piece| piece.text.as_bytes()),
            self.pieces(b, skip).map(|piece| piece.text.as_bytes()),
        )
    }

    /// The collation order of two nodes' keys: their units, then the tie-break of their text.
    fn cmp(&self, a: usize, b: usize) -> Ordering {
        let (a, b) = (Reference::Path(a), Reference::Path(b));
        let skip = self.shared(a, b);
        joined_cmp(
            self.pieces(a, skip).map(Encoded::primary),
            self.pieces(b, skip).map(Encoded::primary),
        )
        .then_with(|| {
            tie_chars(
                self.pieces(a, skip).flat_map(|piece| piece.text.chars()),
                self.pieces(b, skip).flat_map(|piece| piece.text.chars()),
            )
        })
    }
}

struct Resolver<'a> {
    depth: &'a [i32],
    raw: &'a [usize],
    locals: &'a [Encoded],
    scratch: &'a mut Scratch,
}

impl Resolver<'_> {
    fn paths(&self) -> Paths<'_> {
        Paths {
            depth: self.depth,
            raw: self.raw,
            locals: self.locals,
            prefixes: &self.scratch.prefixes,
            separators: &self.scratch.separators,
            links: &self.scratch.links,
            lengths: &self.scratch.lengths,
            ancestors: &self.scratch.ancestors,
            levels: self.scratch.levels,
        }
    }

    /// Resolves `start` and every ancestor it reaches, parents before children.
    fn resolve(&mut self, start: usize) {
        if self.scratch.memo[start] {
            return;
        }
        self.enter(start);
        while let Some(frame) = self.scratch.pending.last_mut() {
            let node = frame.node;
            let Some(&parent) = self.scratch.parents[node].get(frame.next) else {
                let best = frame.best;
                self.scratch.pending.pop();
                self.finish(node, best);
                self.offer(Reference::Path(node));
                continue;
            };
            frame.next += 1;
            if self.scratch.memo[parent] {
                self.offer(Reference::Path(parent));
            } else if self.scratch.visiting[parent] {
                self.offer(Reference::Local(parent));
            } else {
                self.enter(parent);
            }
        }
    }

    fn enter(&mut self, node: usize) {
        self.scratch.visiting[node] = true;
        self.scratch.pending.push(Frame {
            node,
            next: 0,
            best: None,
        });
    }

    /// Makes `candidate` the parent key of the node being resolved when its text sorts
    /// before the ones seen so far.
    fn offer(&mut self, candidate: Reference) {
        let Some(best) = self.scratch.pending.last().map(|frame| frame.best) else {
            return;
        };
        if best.is_none_or(|current| self.paths().text_cmp(candidate, current) == Ordering::Less) {
            let frame = self.scratch.pending.last_mut().expect("a node is pending");
            frame.best = Some(candidate);
        }
    }

    fn finish(&mut self, node: usize, best: Option<Reference>) {
        let scratch = &mut *self.scratch;
        let link = best.unwrap_or(Reference::Root(scratch.root_order[node]));
        let (length, above) = match link {
            Reference::Path(parent) => (scratch.lengths[parent] + 1, parent),
            Reference::Local(_) | Reference::Root(_) => (1, NONE),
        };
        scratch.links[node] = link;
        scratch.lengths[node] = length;
        let row = node * scratch.levels;
        scratch.ancestors[row] = above;
        for level in 1..scratch.levels {
            let below = scratch.ancestors[row + level - 1];
            scratch.ancestors[row + level] = if below == NONE {
                NONE
            } else {
                scratch.ancestors[below * scratch.levels + level - 1]
            };
        }
        scratch.memo[node] = true;
        scratch.visiting[node] = false;
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
    let deepest = depth.iter().copied().max().map_or(0, separator);
    for column in scratch.separators.len()..=deepest {
        scratch
            .separators
            .push(Encoded::new(format!(">{column:02}>")));
    }
    if scratch.parents.len() < count {
        scratch.parents.resize_with(count, Vec::new);
    }
    // A path holds each node once at most, so `levels` binary steps reach any ancestor.
    scratch.levels = ((usize::BITS - count.leading_zeros()) as usize).max(1);
    scratch.links.resize(count, Reference::Root(0));
    scratch.lengths.resize(count, 0);
    scratch.ancestors.resize(count * scratch.levels, NONE);
    scratch.memo.resize(count, false);
    scratch.memo.fill(false);
    scratch.visiting.resize(count, false);
    scratch.visiting.fill(false);
    scratch.pending.clear();
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
    let mut sorted = std::mem::take(&mut resolver.scratch.sorted);
    sorted.clear();
    sorted.extend(0..count);
    let paths = resolver.paths();
    sorted.sort_unstable_by(|&a, &b| paths.cmp(a, b));
    let mut ranks = vec![0; count];
    let mut rank = 0;
    for (at, &node) in sorted.iter().enumerate() {
        if at > 0 && paths.cmp(sorted[at - 1], node) != Ordering::Equal {
            rank += 1;
        }
        ranks[node] = rank;
    }
    resolver.scratch.sorted = sorted;
    ranks
}
