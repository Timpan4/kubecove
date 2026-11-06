use super::*;

// Parent choice, cycles, root numbering, equal collation keys and Unicode
// must preserve the complete string algorithm's ranks when scratch is reused.
#[test]
fn encoded_order_matches_complete_layout_keys() {
    use serde_json::json;
    let nodes: Vec<_> = ["Z", "a", "b", "c", "é", "😀"]
        .into_iter()
        .map(|name| {
            serde_json::from_value(
                json!({"id":name,"name":name,"kind":"Pod","health":"healthy","selectable":true,
            "summary":{"name":name,"kind":"Pod","cluster":"test","namespace":"ns","age":"1d"}}),
            )
            .unwrap()
        })
        .collect();
    let edges = [("Z", "b"), ("a", "b"), ("b", "c"), ("c", "a")]
        .into_iter()
        .enumerate()
        .map(|(i, (source, target))| {
            serde_json::from_value(
                json!({"id":i.to_string(),"source":source,"target":target,"relation":"owns"}),
            )
            .unwrap()
        })
        .collect();
    let topology = kubecove_lib::models::ResourceTopology {
        nodes,
        edges,
        warnings: vec![],
    };
    let rows = topology
        .nodes
        .iter()
        .map(|node| node.summary.clone())
        .collect();
    let source = crate::topology::Source::new(&topology, &rows);
    let graph = Graph::ranked(
        topology.nodes.iter().enumerate(),
        topology.edges.iter().enumerate(),
        &source,
    );
    let raw: Vec<_> = graph
        .nodes
        .iter()
        .map(|node| source.index[node.id.as_str()])
        .collect();
    let depth = super::super::depths(&graph);
    let expected = legacy_order_keys(&graph, &depth);
    let mut scratch = Scratch::default();
    assert_eq!(
        order_keys(&graph, &depth, &raw, &source.locals, &mut scratch),
        expected
    );
    assert_eq!(
        order_keys(&graph, &depth, &raw, &source.locals, &mut scratch),
        expected
    );
}

use super::super::{created_at_ms, kind_rank};
use crate::collate::Key;

/// `topologyLayoutOrder`: a key that keeps each subtree under its root.
fn legacy_order_keys(graph: &Graph, depth: &[i32]) -> Vec<usize> {
    let count = graph.nodes.len();
    let mut roots: Vec<usize> = (0..count)
        .filter(|&node| graph.parents[node].is_empty())
        .collect();
    roots.sort_by_key(|&node| graph.node_ranks[node]);
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
            let mut parents = self.graph.parents[node].to_vec();
            parents.sort_by_key(|&node| self.graph.node_ranks[node]);
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
    let keys: Vec<String> = (0..count).map(|node| resolver.resolve(node)).collect();
    let mut sorted: Vec<usize> = (0..count).collect();
    sorted.sort_by_cached_key(|&node| Key::new(&keys[node], false));
    let mut ranks = vec![0; count];
    let mut rank = 0;
    for (at, &node) in sorted.iter().enumerate() {
        if at > 0 && keys[sorted[at - 1]] != keys[node] {
            rank += 1;
        }
        ranks[node] = rank;
    }
    ranks
}
