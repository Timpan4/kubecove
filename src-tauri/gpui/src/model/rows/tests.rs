use super::*;
use serde_json::json;

// Failure modes: filtering changes a kind lane's first member; equal names lose
// source order; removing the last GitOps owner changes grouping; lowercasing or
// joined search fields changes substring matches; a match runs from one row's
// search text into the next row's in the shared buffer; a new source keeps old indices.
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
        "paymentsa-2",
        "paymentsz-2",
        "paymentsa",
        "charta-02",
        "appa-2",
        "paymentsé",
        "paymentsi",
        "s",
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

// The packed-key sort must order every column as the web's comparison sort
// does. Failure modes: a descending sort reverses the name tie-break; ties
// use numeric name collation (`pod-10` before `pod-9`, `a1` before `a01`);
// values that differ only in case do not tie; a missing number (-1) or a
// fractional CPU value sorts out of place; canonical and other Age values
// mixed in one sort compare by different rules; equal names in two namespaces
// lose row order; rows outside the filter shift the ranks of visible rows;
// owner groups, kind lanes and preferred rows follow name order instead of
// the active sort.
#[test]
fn packed_sort_matches_the_web_comparison_sort() {
    use crate::model::{TableView, grouped_page};
    use crate::table_sort::{Column, Sort, Usage, reference};
    use kubecove_lib::models::HealthAssessmentState as State;
    const NAMES: [&str; 12] = [
        "pod-9", "pod-10", "a01", "a1", "A1", "Étage", "same", "same", "zeta", "b-2", "b-02",
        "pod-9b",
    ];
    const STATES: [State; 5] = [
        State::Healthy,
        State::Degraded,
        State::NeedsAttention,
        State::Unknown,
        State::NotEvaluated,
    ];
    const READY: [Option<&str>; 6] = [
        Some("1/1"),
        Some("0/1"),
        Some("10/10"),
        None,
        Some("Ready"),
        Some("ready"),
    ];
    let build = |mixed_ages: bool| -> Vec<ResourceSummary> {
        (0..40)
            .map(|index: usize| {
                let kind = ["Pod", "Deployment", "Service", "Widget"][index % 4];
                let namespace = [Some("a"), Some("b"), Some("B"), None][index / 12 % 4];
                let argo = [None, Some("shop"), Some("web")][index % 3];
                let mut row: ResourceSummary = serde_json::from_value(json!({
                    "name": NAMES[index % NAMES.len()],
                    "kind": kind,
                    "cluster": "test",
                    "namespace": namespace,
                    "age": format!("{}d", index % 7),
                    "argoApp": argo,
                    "helmRelease": index.is_multiple_of(5).then_some("chart"),
                }))
                .unwrap();
                row.health_assessment.state = STATES[index % STATES.len()];
                row.ready = READY[index % READY.len()].map(str::to_owned);
                row.restarts = (!index.is_multiple_of(4)).then_some((index % 3) as i32);
                row.created_at = (!mixed_ages || !index.is_multiple_of(6)).then(|| {
                    format!(
                        "2026-{:02}-0{}T1{}:00:0{}Z",
                        index % 12 + 1,
                        index % 3,
                        index % 2,
                        index % 4
                    )
                });
                row
            })
            .collect()
    };
    let usage = |row: &ResourceSummary| Usage {
        cpu_millicores: (!row.name.len().is_multiple_of(3)).then_some(row.name.len() as f64 / 4.),
        memory_bytes: row.namespace.as_ref().map(|ns| (ns.len() as i64) << 40),
    };
    let identity =
        |row: &ResourceSummary| (row.kind.clone(), row.namespace.clone(), row.name.clone());
    let lanes = crate::preferences::KindLanes::default();
    let collapsed = std::collections::HashSet::new();
    let columns = [
        Column::Name,
        Column::Status,
        Column::Namespace,
        Column::Kind,
        Column::Ready,
        Column::Restarts,
        Column::Cpu,
        Column::Memory,
        Column::Age,
    ];
    for mixed_ages in [false, true] {
        let rows = build(mixed_ages);
        let source = Rc::new(PreparedRows::new(rows));
        for kinds in [
            vec!["Pod", "Deployment", "Service", "Widget"],
            vec!["Deployment", "Widget"],
        ] {
            let kinds: Vec<String> = kinds.into_iter().map(String::from).collect();
            let preferences: [fn(&ResourceSummary) -> bool; 2] = [
                |_| false,
                |row| row.argo_app.as_deref() == Some("web") && row.name.len() % 2 == 1,
            ];
            for preferred in preferences {
                let mut visible = Visible::default();
                visible.refilter_with(
                    source.clone(),
                    KindFilter {
                        kinds: &kinds,
                        selected: |_: &str, _: Option<&str>| true,
                    },
                    "",
                    |_, _| true,
                    preferred,
                    &lanes,
                );
                for column in columns {
                    for desc in [false, true] {
                        let sort = Sort { column, desc };
                        let actual: Vec<_> = visible
                            .sorted_order(sort, usage)
                            .into_iter()
                            .map(|index| identity(&visible.all_rows()[index]))
                            .collect();
                        let refs: Vec<&ResourceSummary> = visible.iter().collect();
                        let view = TableView {
                            page: 0,
                            sort,
                            collapsed: &collapsed,
                            selected: None,
                        };
                        let expected: Vec<_> = grouped_page(
                            reference::order(&refs, sort, usage)
                                .into_iter()
                                .map(|index| refs[index])
                                .collect(),
                            &view,
                            preferred,
                            &lanes,
                        )
                        .rows
                        .iter()
                        .map(identity)
                        .collect();
                        assert_eq!(
                            actual, expected,
                            "{sort:?}, mixed ages {mixed_ages}, kinds {kinds:?}"
                        );
                    }
                }
            }
        }
    }
}
