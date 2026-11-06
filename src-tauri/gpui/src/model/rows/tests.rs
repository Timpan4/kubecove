use super::*;
use serde_json::json;

// Failure modes: filtering changes a kind lane's first member; equal names lose
// source order; removing the last GitOps owner changes grouping; lowercasing or
// joined search fields changes substring matches; a new source keeps old indices.
#[test]
fn indexed_rows_preserve_filter_and_display_order() {
    let rows: Vec<ResourceSummary> = [
        ("a-10", "Zed", None, None),
        ("a-2", "Yak", None, Some("chart")),
        ("z-2", "Zed", None, None),
        ("a-02", "Yak", Some("app"), None),
        ("a-2", "Pod", None, None),
        ("Étage", "Deployment", Some("app"), None),
        ("İstanbul", "Pod", None, None),
    ]
    .into_iter()
    .map(|(name, kind, argo, helm)| {
        serde_json::from_value(json!({
            "name":name,"kind":kind,"cluster":"test","namespace":"payments","age":"1d",
            "argoApp":argo,"helmRelease":helm
        }))
        .unwrap()
    })
    .collect();
    let source = Rc::new(PreparedRows::new(rows.clone()));
    let mut visible = Visible::default();
    for query in [
        "",
        "a-2",
        "z-2",
        "app",
        "helm",
        "pod",
        "PAYMENTS",
        "partial",
        "é",
        "i",
        " evidencepayments ",
    ] {
        let query = query.trim().to_lowercase();
        for kinds in [
            vec!["Zed", "Yak", "Pod", "Deployment"],
            vec!["Zed", "Yak"],
            vec!["Pod"],
            vec![],
        ] {
            let kinds: Vec<String> = kinds.into_iter().map(String::from).collect();
            let expected: Vec<_> = rows
                .iter()
                .filter(|row| kinds.contains(&row.kind))
                .filter(|row| {
                    query.is_empty() || crate::resource_search::haystack(row).contains(&query)
                })
                .cloned()
                .collect();
            let (order, grouped) = super::super::display_order(&expected);
            visible.refilter(source.clone(), &kinds, &query);
            assert_eq!(
                visible
                    .iter()
                    .map(|row| (&row.name, &row.kind))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|row| (&row.name, &row.kind))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                visible
                    .ordered()
                    .map(|row| (&row.name, &row.kind))
                    .collect::<Vec<_>>(),
                order
                    .iter()
                    .map(|&i| (&expected[i].name, &expected[i].kind))
                    .collect::<Vec<_>>()
            );
            assert_eq!(visible.grouped_by_gitops, grouped);
        }
    }
    let replacement = Rc::new(PreparedRows::new(vec![rows[4].clone()]));
    visible.refilter(replacement, &["Pod".into()], "");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible.ordered().next().unwrap().name, "a-2");
}

// A different preferred owner must change display order even when membership
// and every prepared row key stay unchanged.
#[test]
fn changing_preferred_owner_reorders_the_same_rows() {
    let rows = ["a", "b"].map(|name| {
        serde_json::from_value(json!({
            "name":name,"kind":"Pod","cluster":"test","namespace":"payments",
            "age":"1d","argoApp":name
        }))
        .unwrap()
    });
    let source = Rc::new(PreparedRows::new(rows.into()));
    let mut visible = Visible::default();
    let kinds = vec!["Pod".into()];
    let lanes = crate::preferences::KindLanes::default();
    for preferred in [None, Some("b"), Some("a"), None] {
        visible.refilter_with(
            source.clone(),
            KindFilter {
                kinds: &kinds,
                selected: |_: &str, _: Option<&str>| true,
            },
            "",
            |_, _| true,
            |row| preferred == Some(row.name.as_str()),
            &lanes,
        );
        let names: Vec<_> = visible.ordered().map(|row| row.name.as_str()).collect();
        assert_eq!(
            names,
            if preferred == Some("b") {
                vec!["b", "a"]
            } else {
                vec!["a", "b"]
            }
        );
    }
}

// Kind names can be shared by API groups. A new selection must be evaluated
// even when the source and query do not change; absent API versions stay wildcard matches.
#[test]
fn kind_selection_distinguishes_api_versions_and_rechecks_selection() {
    let rows = [
        ("alpha", Some("alpha/v1")),
        ("beta", Some("beta/v1")),
        ("legacy", None),
    ]
    .map(|(name, api)| {
        serde_json::from_value(json!({
            "name":name, "kind":"Widget", "apiVersion":api, "cluster":"test",
            "namespace":"payments", "age":"1d"
        }))
        .unwrap()
    });
    let source = Rc::new(PreparedRows::new(rows.into()));
    let kinds = vec!["Widget".into()];
    let lanes = crate::preferences::KindLanes::default();
    let mut visible = Visible::default();
    for selected in [Some("alpha/v1"), Some("beta/v1"), None, Some("alpha/v1")] {
        visible.refilter_with(
            source.clone(),
            KindFilter {
                kinds: &kinds,
                selected: |kind: &str, api: Option<&str>| {
                    kind == "Widget"
                        && api.is_none_or(|api| selected.is_none_or(|selected| api == selected))
                },
            },
            "",
            |_, _| true,
            |_| false,
            &lanes,
        );
        let names: Vec<_> = visible.ordered().map(|row| row.name.as_str()).collect();
        assert_eq!(
            names,
            match selected {
                Some("alpha/v1") => vec!["alpha", "legacy"],
                Some("beta/v1") => vec!["beta", "legacy"],
                _ => vec!["alpha", "beta", "legacy"],
            }
        );
    }
}

// Preferred rows must promote their entire owner group and kind lane. Other
// members stay inside that lane, with preferred members before ordinary members.
#[test]
fn preferred_members_keep_owner_groups_and_kind_lanes_contiguous() {
    let rows = [
        ("a", "Pod", "app-x"),
        ("b", "Service", "app-x"),
        ("c", "Pod", "app-x"),
        ("d", "Pod", "app-y"),
        ("e", "Service", "app-x"),
    ]
    .map(|(name, kind, app)| {
        serde_json::from_value(json!({
            "name":name, "kind":kind, "cluster":"test", "namespace":"payments",
            "age":"1d", "argoApp":app
        }))
        .unwrap()
    });
    let source = Rc::new(PreparedRows::new(rows.into()));
    let kinds = vec!["Pod".into(), "Service".into()];
    let mut visible = Visible::default();
    visible.refilter_with(
        source,
        KindFilter {
            kinds: &kinds,
            selected: |_: &str, _: Option<&str>| true,
        },
        "",
        |_, _| true,
        |row| matches!(row.name.as_str(), "c" | "d" | "e"),
        &crate::preferences::KindLanes::default(),
    );
    assert_eq!(
        visible
            .ordered()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["c", "a", "e", "b", "d"]
    );
}
