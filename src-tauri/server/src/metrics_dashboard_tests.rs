//! The Grafana dashboard in `deploy/grafana` must query only series that the
//! server exports, or its panels go empty without an error.
use super::*;
use crate::observe::{
    self, AuthMode, AuthRefusal, CloseReason, CommandOutcome, RefreshOutcome, RequestRefusal,
    SessionId, SignInFailure,
};
use axum::http::Method;
use kubecove_lib::models::{AppError, AppErrorKind, NamespaceSummary};
use std::collections::BTreeMap;

const DASHBOARD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../deploy/grafana/kubecove-server.json"
);

/// Labels that Prometheus attaches at scrape time.
const SCRAPE_LABELS: [&str; 2] = ["job", "instance"];
/// Aggregations whose output keeps only the labels their grouping names.
const AGGREGATIONS: [&str; 9] = [
    "sum", "avg", "min", "max", "count", "group", "stddev", "stdvar", "quantile",
];
/// Words that pass labels through or between operands.
const OPERATORS: [&str; 5] = ["or", "and", "unless", "bool", "offset"];
/// Forms whose output labels `parse` does not model. A query that uses one
/// fails until `parse` learns it, instead of passing unchecked.
const UNSUPPORTED: [&str; 9] = [
    "by",
    "without",
    "on",
    "ignoring",
    "group_left",
    "group_right",
    "count_values",
    "label_replace",
    "label_join",
];

/// Sample name -> the label set of each sample with that name.
type Series<'a> = BTreeMap<&'a str, Vec<BTreeMap<&'a str, &'a str>>>;

// Failure modes: a metric is renamed, removed or misspelled, including its
// prefix; a matcher, grouping label or legend names a label that the queried
// metric lacks, even if another metric has it; a selector's exact matchers
// match no single sample of its metric; a legend names a label that an
// aggregation drops; a query uses a form whose labels the parser cannot follow.
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
            let samples = series
                .get(selector.metric)
                .unwrap_or_else(|| panic!("{} is not exported: {query}", selector.metric));
            for matcher in &selector.matchers {
                assert!(
                    SCRAPE_LABELS.contains(&matcher.label)
                        || samples
                            .iter()
                            .any(|labels| labels.contains_key(matcher.label)),
                    "{} has no label {}: {query}",
                    selector.metric,
                    matcher.label
                );
            }
            let exact: Vec<_> = selector
                .matchers
                .iter()
                .filter_map(|matcher| Some((matcher.label, matcher.exact?)))
                .collect();
            assert!(
                exact.is_empty()
                    || samples.iter().any(|labels| {
                        exact
                            .iter()
                            .all(|(label, value)| labels.get(label) == Some(value))
                    }),
                "no {} sample has {exact:?}: {query}",
                selector.metric
            );
        }
        let on_every_metric = |label: &str| {
            SCRAPE_LABELS.contains(&label)
                || parsed.selectors.iter().all(|selector| {
                    series[selector.metric]
                        .iter()
                        .any(|labels| labels.contains_key(label))
                })
        };
        let listed = parsed
            .aggregations
            .iter()
            .flat_map(Grouping::labels)
            .copied();
        for label in listed.chain(read_label) {
            assert!(
                on_every_metric(label),
                "not every metric has {label}: {query}"
            );
        }
        for label in legend_labels(legend.unwrap_or_default()) {
            let kept = on_every_metric(label)
                && !(parsed.drops_le && label == "le")
                && parsed
                    .aggregations
                    .iter()
                    .all(|grouping| grouping.keeps(label));
            assert!(
                kept,
                "the legend names {label}, which {query} does not keep"
            );
        }
    }
}

/// Records every production label value through each observation function,
/// and one cache read, then scrapes. Other tests share the registry, so the
/// fixture seeds every value itself rather than rely on what they record.
async fn scrape_with_every_family() -> String {
    let session = SessionId::random().unwrap();
    let kind = AppErrorKind::Forbidden;
    for mode in [AuthMode::Token, AuthMode::CloudflareAccess, AuthMode::Oidc] {
        observe::session_opened(session, mode, None);
        observe::session_resumed(session, mode, None);
        observe::sign_in_succeeded(mode, None);
        for failure in [
            SignInFailure::InvalidToken,
            SignInFailure::ProviderDenied,
            SignInFailure::InvalidCallback,
            SignInFailure::ProviderUnavailable,
        ] {
            observe::sign_in_failed(mode, failure);
        }
        for refusal in [
            AuthRefusal::Missing,
            AuthRefusal::Invalid,
            AuthRefusal::Unavailable,
        ] {
            observe::authentication_refused(mode, refusal);
        }
    }
    for reason in [
        CloseReason::Client,
        CloseReason::Logout,
        CloseReason::Shutdown,
        CloseReason::ActivationFailed,
    ] {
        observe::session_closed(session, reason);
    }
    observe::session_expired(session, Duration::ZERO);
    observe::session_cleanup_failed(session, kind);
    for refusal in [RequestRefusal::Host, RequestRefusal::Origin] {
        observe::request_refused(refusal, &Method::GET, None, None);
    }
    for outcome in [
        RefreshOutcome::Renewed,
        RefreshOutcome::Refused,
        RefreshOutcome::Unusable,
        RefreshOutcome::Failed,
    ] {
        observe::oidc_refresh(outcome);
    }
    for outcome in [
        CommandOutcome::Ok,
        CommandOutcome::Cancelled,
        CommandOutcome::Error(kind),
        CommandOutcome::SessionEnded,
        CommandOutcome::AuthenticationEnded,
    ] {
        observe::command_completed(session, "list_namespaces", outcome, Duration::ZERO);
    }
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

/// Every sample in a scrape, with the label set it carries.
fn exported(text: &str) -> Series<'_> {
    let mut series = Series::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let Some(end) = line.find(['{', ' ']) else {
            continue;
        };
        let mut labels = BTreeMap::new();
        let mut rest = line[end..].strip_prefix('{').unwrap_or_default();
        while let Some((label, tail)) = rest.split_once("=\"") {
            let (value, tail) = tail.split_once('"').expect("closed label value");
            labels.insert(label.trim_start_matches(','), value);
            if tail.starts_with('}') {
                break;
            }
            rest = tail;
        }
        series.entry(&line[..end]).or_default().push(labels);
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
    aggregations: Vec<Grouping<'a>>,
    /// `histogram_quantile` removes the `le` label.
    drops_le: bool,
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

/// The labels an aggregation keeps.
enum Grouping<'a> {
    None,
    By(Vec<&'a str>),
    Without(Vec<&'a str>),
}

impl Grouping<'_> {
    fn labels(&self) -> &[&str] {
        match self {
            Self::None => &[],
            Self::By(labels) | Self::Without(labels) => labels,
        }
    }

    fn keeps(&self, label: &str) -> bool {
        match self {
            Self::None => false,
            Self::By(labels) => labels.contains(&label),
            Self::Without(labels) => !labels.contains(&label),
        }
    }
}

/// Splits the `PromQL` subset the dashboard uses into selectors and
/// aggregations. Any identifier that is not a function, aggregation or
/// operator counts as a metric, so a misspelled name fails the existence check.
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
            assert!(
                !UNSUPPORTED.contains(&word),
                "teach parse() the labels `{word}` keeps before using it: {query}"
            );
            if AGGREGATIONS.contains(&word) {
                parsed.aggregations.push(match next_word {
                    "by" | "without" => {
                        let (list, tail) = next[next_word.len()..]
                            .trim_start()
                            .strip_prefix('(')
                            .and_then(|list| list.split_once(')'))
                            .expect("label list");
                        let labels = list
                            .split(',')
                            .map(str::trim)
                            .filter(|l| !l.is_empty())
                            .collect();
                        rest = tail;
                        if next_word == "by" {
                            Grouping::By(labels)
                        } else {
                            Grouping::Without(labels)
                        }
                    }
                    _ => Grouping::None,
                });
            } else if next.starts_with('(') {
                parsed.drops_le |= word == "histogram_quantile";
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
