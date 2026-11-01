//! The Grafana dashboard in `deploy/grafana` must query only series that the
//! server exports, or its panels go empty without an error.
use super::*;
use crate::observe::{
    self, AuthMode, AuthRefusal, CloseReason, CommandOutcome, RefreshOutcome, RequestRefusal,
    SessionId, SignInFailure,
};
use axum::http::Method;
use kubecove_lib::models::{AppError, AppErrorKind, NamespaceSummary};
use std::collections::{BTreeMap, BTreeSet};

const DASHBOARD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../deploy/grafana/kubecove-server.json"
);

/// Labels that Prometheus attaches at scrape time.
const SCRAPE_LABELS: [&str; 2] = ["job", "instance"];
const AGGREGATIONS: [&str; 12] = [
    "sum",
    "avg",
    "min",
    "max",
    "count",
    "group",
    "stddev",
    "stdvar",
    "topk",
    "bottomk",
    "quantile",
    "count_values",
];
const LABEL_LISTS: [&str; 4] = ["by", "without", "on", "ignoring"];
const OPERATORS: [&str; 7] = [
    "or",
    "and",
    "unless",
    "bool",
    "offset",
    "group_left",
    "group_right",
];

/// Sample name -> label name -> label values seen in a scrape.
type Series<'a> = BTreeMap<&'a str, BTreeMap<&'a str, BTreeSet<&'a str>>>;

// Failure modes: a metric is renamed, removed or misspelled, including its
// prefix; a matcher, grouping label or legend names a label that the queried
// metric lacks, even if another metric has it; a matcher expects a value the
// queried metric never emits; a legend names a label the aggregation drops.
#[tokio::test]
async fn grafana_dashboard_queries_only_exported_series() {
    let text = scrape_with_every_family().await;
    let series = exported(&text);
    let dashboard: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(DASHBOARD).unwrap()).unwrap();
    let mut queries = Vec::new();
    collect(&dashboard, &mut queries);
    assert!(!queries.is_empty(), "the dashboard has no queries");

    for (query, legend) in queries {
        // A `label_values(selector, label)` variable reads `label` from the selector.
        let (expr, read_label) = match query.strip_prefix("label_values(") {
            Some(args) => {
                let (expr, label) = args.rsplit_once(',').expect("label_values arguments");
                (expr, Some(label.trim().trim_end_matches(')').trim()))
            }
            None => (query, None),
        };
        let parsed = parse(expr);
        assert!(!parsed.selectors.is_empty(), "no metric in {query}");
        for selector in &parsed.selectors {
            let labels = series
                .get(selector.metric)
                .unwrap_or_else(|| panic!("{} is not exported: {query}", selector.metric));
            for matcher in &selector.matchers {
                if SCRAPE_LABELS.contains(&matcher.label) {
                    continue;
                }
                let values = labels.get(matcher.label).unwrap_or_else(|| {
                    panic!(
                        "{} has no label {}: {query}",
                        selector.metric, matcher.label
                    )
                });
                if let Some(value) = matcher.exact {
                    assert!(
                        values.contains(value),
                        "{} never has {}=\"{value}\": {query}",
                        selector.metric,
                        matcher.label
                    );
                }
            }
        }
        let on_every_metric = |label: &str| {
            SCRAPE_LABELS.contains(&label)
                || parsed
                    .selectors
                    .iter()
                    .all(|selector| series[selector.metric].contains_key(label))
        };
        for label in parsed.label_lists.iter().chain(read_label.as_ref()) {
            assert!(
                on_every_metric(label),
                "not every metric has {label}: {query}"
            );
        }
        for label in legend_labels(legend.unwrap_or_default()) {
            let kept = if parsed.by.is_empty() {
                !(parsed.aggregated && parsed.label_lists.is_empty()) && on_every_metric(label)
            } else {
                parsed.by.contains(&label)
            };
            assert!(
                kept,
                "the legend names {label}, which {query} does not keep"
            );
        }
    }
}

/// Records one event through every observation function, with production
/// label values, and one cache read, then scrapes.
async fn scrape_with_every_family() -> String {
    let session = SessionId::random().unwrap();
    let kind = AppErrorKind::Forbidden;
    observe::session_opened(session, AuthMode::Token, None);
    observe::session_resumed(session, AuthMode::Token, None);
    observe::session_closed(session, CloseReason::Client);
    observe::session_expired(session, Duration::ZERO);
    observe::session_cleanup_failed(session, kind);
    observe::sign_in_succeeded(AuthMode::Token, None);
    observe::sign_in_failed(AuthMode::Token, SignInFailure::InvalidToken);
    observe::authentication_refused(AuthMode::Token, AuthRefusal::Missing);
    observe::request_refused(RequestRefusal::Host, &Method::GET, None, None);
    observe::oidc_refresh(RefreshOutcome::Renewed);
    observe::command_completed(
        session,
        "list_namespaces",
        CommandOutcome::Error(kind),
        Duration::ZERO,
    );
    let sessions = Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Token));
    let info = sessions.authenticate_as(None, None).await.unwrap();
    let owner = sessions.owner(&info.resume_token).await.unwrap();
    owner
        .backend
        .live_store
        .namespaces("source".into(), "context".into(), || async {
            Ok::<_, AppError>(vec![NamespaceSummary {
                name: "namespace".into(),
                age: "1m".into(),
                created_at: None,
            }])
        })
        .await
        .unwrap();
    global().render(&sessions).await
}

/// Every sample in a scrape, with the label values it carries.
fn exported(text: &str) -> Series<'_> {
    let mut series = Series::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let Some(end) = line.find(['{', ' ']) else {
            continue;
        };
        let labels = series.entry(&line[..end]).or_default();
        let mut rest = line[end..].strip_prefix('{').unwrap_or_default();
        while let Some((label, tail)) = rest.split_once("=\"") {
            let (value, tail) = tail.split_once('"').expect("closed label value");
            labels
                .entry(label.trim_start_matches(','))
                .or_default()
                .insert(value);
            if tail.starts_with('}') {
                break;
            }
            rest = tail;
        }
    }
    series
}

/// Each query in the dashboard, including variables, with its legend.
fn collect<'a>(value: &'a serde_json::Value, queries: &mut Vec<(&'a str, Option<&'a str>)>) {
    match value {
        serde_json::Value::Object(map) => {
            let legend = map.get("legendFormat").and_then(|legend| legend.as_str());
            // A variable runs `query.query`; its `definition` is display text.
            let is_query_object = map.contains_key("refId");
            for (key, value) in map {
                match (key.as_str(), value.as_str()) {
                    ("expr" | "definition", Some(query)) => queries.push((query, legend)),
                    ("query", Some(query)) if is_query_object => queries.push((query, legend)),
                    _ => collect(value, queries),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect(item, queries);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Query<'a> {
    selectors: Vec<Selector<'a>>,
    /// Labels in `by`, `without`, `on` and `ignoring` lists.
    label_lists: Vec<&'a str>,
    by: Vec<&'a str>,
    aggregated: bool,
}

struct Selector<'a> {
    metric: &'a str,
    matchers: Vec<Matcher<'a>>,
}

struct Matcher<'a> {
    label: &'a str,
    /// The value of an exact match that names no dashboard variable.
    exact: Option<&'a str>,
}

/// Splits the `PromQL` subset the dashboard uses into selectors and label lists.
/// Any identifier that is not a function, aggregation or operator counts as a
/// metric, so a misspelled name fails the existence check.
fn parse(query: &str) -> Query<'_> {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':';
    let mut parsed = Query::default();
    let mut rest = query;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            let (body, tail) = rest[1..].split_once('}').expect("closed selector");
            let selector = parsed.selectors.last_mut().expect("a metric before {");
            selector.matchers = body
                .split(',')
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(matcher)
                .collect();
            rest = tail;
        } else if c == '[' {
            rest = rest.split_once(']').expect("closed range").1;
        } else if c.is_ascii_alphabetic() || c == '_' || c == ':' {
            let (word, tail) = rest.split_at(rest.find(|c| !is_word(c)).unwrap_or(rest.len()));
            let next = tail.trim_start();
            let next_word = &next[..next.find(|c| !is_word(c)).unwrap_or(next.len())];
            rest = tail;
            if LABEL_LISTS.contains(&word) {
                let (list, tail) = next
                    .strip_prefix('(')
                    .and_then(|list| list.split_once(')'))
                    .expect("label list");
                let labels = list.split(',').map(str::trim).filter(|l| !l.is_empty());
                if word == "by" {
                    parsed.by.extend(labels.clone());
                }
                parsed.label_lists.extend(labels);
                rest = tail;
            } else if next.starts_with('(') || LABEL_LISTS.contains(&next_word) {
                parsed.aggregated |= AGGREGATIONS.contains(&word);
            } else if !OPERATORS.contains(&word) {
                parsed.selectors.push(Selector {
                    metric: word,
                    matchers: Vec::new(),
                });
            }
        } else if c.is_ascii_digit() || c == '.' {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
        } else {
            rest = &rest[c.len_utf8()..];
        }
    }
    parsed
}

fn matcher(text: &str) -> Matcher<'_> {
    let at = text.find(['=', '!']).expect("matcher operator");
    let exact = text[at..]
        .strip_prefix('=')
        .filter(|value| !value.starts_with('~'))
        .map(|value| value.trim().trim_matches('"'))
        .filter(|value| !value.contains('$'));
    Matcher {
        label: text[..at].trim(),
        exact,
    }
}

fn legend_labels(legend: &str) -> impl Iterator<Item = &str> {
    legend
        .split("{{")
        .skip(1)
        .filter_map(|rest| rest.split_once("}}"))
        .map(|(label, _)| label.trim())
}
