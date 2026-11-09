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
    let contexts = ["test".to_owned()];
    let source = Rc::new(PreparedRows::new(rows.clone(), &contexts));
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
            let (order, grouped) = super::super::display_order(&expected, &contexts);
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
    let replacement = Rc::new(PreparedRows::new(vec![rows[4].clone()], &contexts));
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
    let source = Rc::new(PreparedRows::new(rows.into(), &["test".to_owned()]));
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
    let source = Rc::new(PreparedRows::new(rows.into(), &["test".to_owned()]));
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
    let source = Rc::new(PreparedRows::new(rows.into(), &["test".to_owned()]));
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
// the active sort. With two contexts: equal names tie by context name or by
// row order instead of context rank; the Context column sorts by rank instead
// of by context name; owner and Helm groups of two contexts merge.
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
    let build = |mixed_ages: bool, contexts: &[String]| -> Vec<ResourceSummary> {
        (0..40)
            .map(|index: usize| {
                let kind = ["Pod", "Deployment", "Service", "Widget"][index % 4];
                let namespace = [Some("a"), Some("b"), Some("B"), None][index / 12 % 4];
                let argo = [None, Some("shop"), Some("web")][index % 3];
                let mut row: ResourceSummary = serde_json::from_value(json!({
                    "name": NAMES[index % NAMES.len()],
                    "kind": kind,
                    "cluster": contexts[index / 7 % contexts.len()],
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
    let identity = |row: &ResourceSummary| {
        (
            row.kind.clone(),
            row.namespace.clone(),
            row.name.clone(),
            row.cluster.clone(),
        )
    };
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
        Column::Context,
    ];
    // "east" collates before "west", so rank order differs from name order.
    let context_sets = [
        vec!["test".to_owned()],
        vec!["west".to_owned(), "east".to_owned()],
    ];
    for (contexts, mixed_ages) in context_sets
        .iter()
        .flat_map(|contexts| [false, true].map(|mixed| (contexts, mixed)))
    {
        let rows = build(mixed_ages, contexts.as_slice());
        let source = Rc::new(PreparedRows::new(rows, contexts));
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
                            reference::order(&refs, contexts, sort, usage)
                                .into_iter()
                                .map(|index| refs[index])
                                .collect(),
                            &view,
                            preferred,
                            &lanes,
                            contexts.len() > 1,
                        )
                        .rows
                        .iter()
                        .map(identity)
                        .collect();
                        assert_eq!(
                            actual, expected,
                            "{sort:?}, mixed ages {mixed_ages}, kinds {kinds:?}, contexts {contexts:?}"
                        );
                    }
                }
            }
        }
    }
}

fn resource(name: &str, cluster: &str) -> ResourceSummary {
    serde_json::from_value(json!({
        "name":name, "kind":"Pod", "cluster":cluster, "namespace":"payments", "age":"1d"
    }))
    .unwrap()
}

/// Two workspace contexts whose rank order differs from their name order.
fn workspace() -> [String; 2] {
    ["mock-dev".to_owned(), "admin@solid-k8s".to_owned()]
}

// Same-named rows of two contexts tie under every column but Context.
// Failure modes: the tie breaks by row order, so a row a watch appends sorts
// after a same-named row of a later context; it breaks by context name
// instead of workspace rank; a descending sort reverses the rank; the
// Context column sorts by rank instead of by context name.
#[test]
fn same_names_in_two_contexts_order_by_context_rank() {
    use crate::table_sort::{Column, Sort};
    use kubecove_lib::models::WatchResourceKey;
    let mut rows = PreparedRows::new(
        vec![resource("payments-api", "admin@solid-k8s")],
        &workspace(),
    );
    let key: WatchResourceKey =
        serde_json::from_value(json!({"resourceKind":{"kind":"Pod"}, "namespace":null})).unwrap();
    rows.patch(
        "mock-dev",
        &key,
        vec![resource("payments-api", "mock-dev")],
        None,
    );
    fn clusters(rows: &[ResourceSummary]) -> Vec<String> {
        rows.iter().map(|row| row.cluster.clone()).collect()
    }
    assert_eq!(
        clusters(&rows),
        ["admin@solid-k8s", "mock-dev"],
        "the watch appends its row"
    );
    let mut visible = Visible::default();
    visible.refilter(Rc::new(rows), &["Pod".into()], "");
    let by_rank = ["mock-dev", "admin@solid-k8s"];
    assert_eq!(
        visible
            .ordered()
            .map(|row| row.cluster.as_str())
            .collect::<Vec<_>>(),
        by_rank
    );
    let sorted = |column: Column, desc: bool| -> Vec<String> {
        visible
            .sorted_order(Sort { column, desc }, |_| Default::default())
            .into_iter()
            .map(|index| visible.all_rows()[index].cluster.clone())
            .collect()
    };
    for column in [
        Column::Name,
        Column::Status,
        Column::Namespace,
        Column::Kind,
        Column::Ready,
        Column::Restarts,
        Column::Cpu,
        Column::Memory,
        Column::Age,
    ] {
        for desc in [false, true] {
            assert_eq!(sorted(column, desc), by_rank, "{column:?}, desc {desc}");
        }
    }
    assert_eq!(
        sorted(Column::Context, false),
        ["admin@solid-k8s", "mock-dev"]
    );
    assert_eq!(sorted(Column::Context, true), by_rank);
}

// A Pod inherits the GitOps owner of its ReplicaSet. Failure mode: a Pod in
// one context inherits the owner of a same-named ReplicaSet in another.
#[test]
fn owners_are_inherited_only_within_one_context() {
    let replica_set: ResourceSummary = serde_json::from_value(json!({
        "name":"payments-api-rs", "kind":"ReplicaSet", "cluster":"mock-dev",
        "namespace":"payments", "age":"1d",
        "gitOpsOwner":{"provider":"argo", "kind":"Application", "name":"payments",
            "confidence":"label", "provenance":"tracking-id", "partial":false}
    }))
    .unwrap();
    let pod = |cluster: &str| {
        let mut pod = resource("payments-api", cluster);
        pod.owner_ref = Some("payments-api-rs".into());
        pod
    };
    let rows = PreparedRows::with_inherited_owners(
        vec![replica_set, pod("mock-dev"), pod("admin@solid-k8s")],
        &workspace(),
    );
    let owner = |cluster: &str| -> Option<String> {
        let pod = rows
            .iter()
            .find(|row| row.kind == "Pod" && row.cluster == cluster)
            .unwrap();
        pod.git_ops_owner.as_ref().map(|owner| owner.name.clone())
    };
    assert_eq!(owner("mock-dev").as_deref(), Some("payments"));
    assert_eq!(owner("admin@solid-k8s"), None);
}

// Owner and Helm groups belong to one context. Failure modes: one Argo CD
// application name or one Helm release in two contexts makes one group or
// one owner filter key; a one-context view adds the context to its labels.
#[test]
fn owner_and_helm_groups_split_by_context() {
    use crate::model::{Entry, TableView, indexed_table_page};
    fn row(name: &str, cluster: &str, argo: Option<&str>, helm: Option<&str>) -> ResourceSummary {
        serde_json::from_value(json!({
            "name":name, "kind":"Pod", "cluster":cluster, "namespace":"payments",
            "age":"1d", "argoApp":argo, "helmRelease":helm
        }))
        .unwrap()
    }
    /// The page's group labels and the distinct owner filter keys.
    fn groups(rows: Vec<ResourceSummary>, contexts: &[String]) -> (Vec<String>, Vec<String>) {
        let source = Rc::new(PreparedRows::new(rows, contexts));
        let owner_keys: std::collections::BTreeSet<String> = source
            .keyed_rows()
            .filter_map(|(_, key)| source.owner(key))
            .map(|(key, _)| key.to_owned())
            .collect();
        let mut visible = Visible::default();
        visible.refilter(source, &["Pod".into()], "");
        let collapsed = std::collections::HashSet::new();
        let view = TableView {
            page: 0,
            sort: Default::default(),
            collapsed: &collapsed,
            selected: None,
        };
        let labels = indexed_table_page(&visible, &view, |_| Default::default())
            .entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Group { label, .. } => Some(label),
                _ => None,
            })
            .collect();
        (labels, owner_keys.into_iter().collect())
    }
    let (labels, keys) = groups(
        vec![
            row("p1", "mock-dev", Some("payments"), None),
            row("p1", "admin@solid-k8s", Some("payments"), None),
            row("h1", "mock-dev", None, Some("chart")),
            row("h1", "admin@solid-k8s", None, Some("chart")),
        ],
        &workspace(),
    );
    assert_eq!(
        labels,
        [
            "Owned by Argo CD: payments (partial evidence) · mock-dev",
            "Owned by Argo CD: payments (partial evidence) · admin@solid-k8s",
            "Helm release: chart · mock-dev",
            "Helm release: chart · admin@solid-k8s",
        ]
    );
    assert_eq!(
        keys,
        [
            "admin@solid-k8s:argo:Application::payments",
            "mock-dev:argo:Application::payments",
        ]
    );

    let (labels, keys) = groups(
        vec![
            row("p1", "mock-dev", Some("payments"), None),
            row("h1", "mock-dev", None, Some("chart")),
        ],
        &["mock-dev".to_owned()],
    );
    assert_eq!(
        labels,
        [
            "Owned by Argo CD: payments (partial evidence)",
            "Helm release: chart",
        ]
    );
    assert_eq!(keys, ["argo:Application::payments"]);
}

// Failure modes: a one-context view matches its context name; a view of two
// contexts misses a context name or matches it in the other context's rows;
// search follows the contexts the rows hold instead of the workspace's list.
#[test]
fn search_matches_a_context_name_only_when_the_view_lists_several() {
    fn matches(rows: Vec<ResourceSummary>, contexts: &[String], query: &str) -> Vec<String> {
        let mut visible = Visible::default();
        visible.refilter(
            Rc::new(PreparedRows::new(rows, contexts)),
            &["Pod".into()],
            query,
        );
        visible
            .iter()
            .map(|row| format!("{}/{}", row.cluster, row.name))
            .collect()
    }
    let one = || vec![resource("payments-api", "mock-dev")];
    assert!(matches(one(), &["mock-dev".to_owned()], "mock-dev").is_empty());
    let both = || {
        vec![
            resource("payments-api", "mock-dev"),
            resource("payments-api", "admin@solid-k8s"),
        ]
    };
    assert_eq!(
        matches(both(), &workspace(), "solid-k8s"),
        ["admin@solid-k8s/payments-api"]
    );
    assert_eq!(
        matches(both(), &workspace(), "mock-dev"),
        ["mock-dev/payments-api"]
    );
    // The workspace lists two contexts, so search names the context even
    // while only one of them has rows.
    assert_eq!(
        matches(one(), &workspace(), "mock-dev"),
        ["mock-dev/payments-api"]
    );
}
