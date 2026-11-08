use super::*;
use kubecove_lib::models::TopologyNode;
use serde_json::json;

// Filtering must keep every ancestor and raw duplicate, even when duplicate
// edge IDs describe different parents. Scope changes must clear old marks.
#[test]
fn indexed_filter_matches_raw_ancestor_closure() {
    let nodes: Vec<TopologyNode> = ["a", "b", "c", "d", "a"]
        .into_iter()
        .map(|name| {
            serde_json::from_value(
                json!({"id":name,"name":name,"kind":"Pod","health":"healthy","selectable":true,
            "summary":{"name":name,"kind":"Pod","cluster":"test","namespace":"ns","age":"1d"}}),
            )
            .unwrap()
        })
        .collect();
    let edges = [
        ("ab", "a", "b"),
        ("bc", "b", "c"),
        ("ab", "d", "b"),
        ("ca", "c", "a"),
        ("x", "absent", "b"),
    ]
    .into_iter()
    .map(|(id, source, target)| {
        serde_json::from_value(json!({"id":id,"source":source,"target":target,"relation":"owns"}))
            .unwrap()
    })
    .collect();
    let topology = ResourceTopology {
        nodes,
        edges,
        warnings: vec![],
    };
    let mut rows: crate::model::Visible = topology
        .nodes
        .iter()
        .map(|node| node.summary.clone())
        .collect();
    let source = Source::new(&topology, &rows);
    let data = rows.source().clone();
    for query in ["", "b", "d", "a", "absent", "c", ""] {
        rows.refilter(data.clone(), &["Pod".into()], query);
        assert_eq!(
            source.filter(&rows, None),
            super::super::filter_by_rows(&topology, &rows)
        );
    }
}
