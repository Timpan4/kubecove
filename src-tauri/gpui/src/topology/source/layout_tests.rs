use super::*;
use crate::model::KindFilter;
use kubecove_lib::models::{TopologyEdge, TopologyNode};
use serde_json::json;
use std::collections::HashSet;

fn node(name: &str, kind: &str) -> TopologyNode {
    serde_json::from_value(json!({
        "id":name,"name":name,"kind":kind,"health":"healthy","selectable":true,
        "summary":{"name":name,"kind":kind,"cluster":"test","namespace":"ns","age":"1d"}
    }))
    .unwrap()
}

fn edge(id: &str, source: &str, target: &str) -> TopologyEdge {
    serde_json::from_value(json!({"id":id,"source":source,"target":target,"relation":"owns"}))
        .unwrap()
}

fn check_layout(topology: &ResourceTopology, source: &Source, rows: &Visible) {
    let (nodes, edges) = source.filter(rows, None, false);
    let graph = layout::Graph::ranked(
        nodes.iter().map(|&i| (i, &topology.nodes[i])),
        edges.iter().map(|&i| (i, &topology.edges[i])),
        source,
    );
    let expected_depth = layout::depths(&graph);
    let expected_order = layout::order_keys(
        &graph,
        &expected_depth,
        &graph.source_nodes,
        &source.locals,
        &mut layout::Scratch::default(),
    );
    let (depth, order) = source.depth_and_order(&graph, true);
    assert_eq!(depth, expected_depth);
    for a in 0..graph.nodes.len() {
        for b in 0..graph.nodes.len() {
            assert_eq!(
                layout::compare(&graph, &depth, &order, a, b),
                layout::compare(&graph, &expected_depth, &expected_order, a, b),
            );
        }
    }
    assert_eq!(
        layout::positions(&graph, &depth, &order),
        layout::positions(&graph, &expected_depth, &expected_order)
    );
}

// Removing descendants and roots must preserve relative layout order, including
// multiple parents, Unicode and duplicate node IDs. Cycles and duplicate edge
// IDs can change parent resolution after filtering and must use the full algorithm.
#[test]
fn filtered_layout_matches_recomputation_for_every_subset() {
    let mut topology = ResourceTopology {
        nodes: vec![
            node("z-root", "Deployment"),
            node("a-root", "Service"),
            node("Étage", "ReplicaSet"),
            node("pod-2", "Pod"),
            node("pod-02", "Pod"),
            node("other", "Service"),
        ],
        edges: vec![
            edge("za", "z-root", "Étage"),
            edge("aa", "a-root", "Étage"),
            edge("p2", "Étage", "pod-2"),
            edge("p02", "other", "pod-02"),
            edge("unknown", "absent", "pod-2"),
        ],
        warnings: vec![],
    };
    let mut duplicate = node("z-last", "Deployment");
    duplicate.id = "z-root".into();
    topology.nodes.push(duplicate);
    for variant in 0..3 {
        let mut topology = topology.clone();
        match variant {
            1 => topology.edges.push(edge("za", "a-root", "pod-02")),
            2 => topology.edges.push(edge("cycle", "pod-2", "z-root")),
            _ => {}
        }
        let mut rows: Visible = topology.nodes.iter().map(|n| n.summary.clone()).collect();
        let source = Source::new(&topology, &rows);
        let data = rows.source().clone();
        let kinds: Vec<_> = topology
            .nodes
            .iter()
            .map(|n| n.summary.kind.clone())
            .collect();
        for subset in 0..1usize << topology.nodes.len() {
            let wanted: HashSet<_> = topology
                .nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| subset & (1 << i) != 0)
                .map(|(_, n)| n.summary.name.as_str())
                .collect();
            rows.refilter_with(
                data.clone(),
                KindFilter {
                    kinds: &kinds,
                    selected: |_: &str, _: Option<&str>| true,
                },
                "",
                |_, row, _| wanted.contains(row.name.as_str()),
                |_| false,
                &crate::preferences::KindLanes::default(),
            );
            check_layout(&topology, &source, &rows);
        }
    }
}

// Owner chains have no length limit in a cluster. One that is far longer than the stack
// allows recursion per link must still lay out, in memory that grows with its length.
#[test]
fn a_long_owner_chain_lays_out() {
    const LINKS: usize = 50_000;
    let name = |i: usize| format!("link-{i}");
    let topology = ResourceTopology {
        nodes: (0..=LINKS).map(|i| node(&name(i), "Pod")).collect(),
        edges: (0..LINKS)
            .map(|i| edge(&i.to_string(), &name(i), &name(i + 1)))
            .collect(),
        warnings: vec![],
    };
    let rows: Visible = topology.nodes.iter().map(|n| n.summary.clone()).collect();
    let source = Source::new(&topology, &rows);
    let graph = layout::Graph::ranked(
        topology.nodes.iter().enumerate(),
        topology.edges.iter().enumerate(),
        &source,
    );
    let (depth, order) = source.depth_and_order(&graph, false);
    let links: Vec<usize> = (0..=LINKS).collect();
    assert_eq!(
        depth,
        links.iter().map(|&i| i as i32).collect::<Vec<_>>(),
        "each link is one column right of its owner"
    );
    assert_eq!(order, links, "each link sorts after its owner");
    let positions = layout::positions(&graph, &depth, &order);
    let first = positions[0].expect("the chain's root is placed");
    for pair in positions.windows(2) {
        let (owner, owned) = (pair[0].unwrap(), pair[1].unwrap());
        assert!(owner.0 < owned.0, "columns run left to right");
        assert_eq!(owned.1, first.1, "a chain stays on one row");
    }
}

// Root prefixes use a minimum width of four in order.rs. Crossing that width
// reverses string order for 9999 and 10000; removing roots then reverses it again.
#[test]
fn root_prefix_width_change_preserves_filtered_positions() {
    let topology = ResourceTopology {
        nodes: (0..=10_000)
            .map(|i| node(&format!("root-{i}"), "Pod"))
            .collect(),
        edges: vec![],
        warnings: vec![],
    };
    let mut rows: Visible = topology.nodes.iter().map(|n| n.summary.clone()).collect();
    let source = Source::new(&topology, &rows);
    let data = rows.source().clone();
    rows.refilter_with(
        data,
        KindFilter {
            kinds: &["Pod".into()],
            selected: |_: &str, _: Option<&str>| true,
        },
        "",
        |_, row, _| matches!(row.name.as_str(), "root-9999" | "root-10000"),
        |_| false,
        &crate::preferences::KindLanes::default(),
    );
    check_layout(&topology, &source, &rows);
}
