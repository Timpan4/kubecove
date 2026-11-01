//! The Grafana dashboard in `deploy/grafana` must query only series that the
//! server exports, or its panels go empty without an error.
use super::*;
use crate::observe::{
    self, AuthMode, AuthRefusal, CloseReason, CommandOutcome, RefreshOutcome, RequestRefusal,
    SessionId, SignInFailure,
};
use axum::http::Method;
use kubecove_lib::models::{AppError, AppErrorKind, NamespaceSummary};
use std::collections::BTreeSet;

const DASHBOARD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../deploy/grafana/kubecove-server.json"
);

/// Labels that Prometheus attaches at scrape time.
const SCRAPE_LABELS: [&str; 2] = ["job", "instance"];

// Failure modes: a metric is renamed or removed; a grouping label, matcher or
// legend names a label its series lacks; a matcher expects a label value the
// server never emits.
#[tokio::test]
async fn grafana_dashboard_queries_only_exported_series() {
    let text = scrape_with_every_family().await;
    let (names, labels) = exported(&text);
    let known = |label: &str| labels.contains(label) || SCRAPE_LABELS.contains(&label);
    let dashboard: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(DASHBOARD).unwrap()).unwrap();
    let (mut queries, mut legends) = (Vec::new(), Vec::new());
    collect(&dashboard, &mut queries, &mut legends);
    assert!(!queries.is_empty(), "the dashboard has no queries");

    for query in &queries {
        for name in metric_names(query) {
            assert!(names.contains(name), "{name} is not exported: {query}");
        }
        for label in grouping_labels(query) {
            assert!(known(label), "no series has label {label}: {query}");
        }
        for (label, value) in matchers(query) {
            assert!(known(label), "no series has label {label}: {query}");
            if let Some(value) = value {
                assert!(
                    text.contains(&format!("{label}=\"{value}\"")),
                    "no series has {label}=\"{value}\": {query}"
                );
            }
        }
    }
    for legend in &legends {
        for label in legend_labels(legend) {
            assert!(known(label), "no series has label {label}: {legend}");
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

/// Sample names and label names in a scrape.
fn exported(text: &str) -> (BTreeSet<&str>, BTreeSet<&str>) {
    let (mut names, mut labels) = (BTreeSet::new(), BTreeSet::new());
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let Some(end) = line.find(['{', ' ']) else {
            continue;
        };
        names.insert(&line[..end]);
        if let Some(set) = line[end..]
            .strip_prefix('{')
            .and_then(|rest| rest.split_once('}'))
        {
            labels.extend(
                set.0
                    .split(',')
                    .filter_map(|pair| pair.split_once('='))
                    .map(|p| p.0),
            );
        }
    }
    (names, labels)
}

/// Query and legend strings anywhere in the dashboard, including variables.
fn collect<'a>(
    value: &'a serde_json::Value,
    queries: &mut Vec<&'a str>,
    legends: &mut Vec<&'a str>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value.as_str()) {
                    ("expr" | "definition", Some(text)) => queries.push(text),
                    ("legendFormat", Some(text)) => legends.push(text),
                    _ => collect(value, queries, legends),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect(item, queries, legends);
            }
        }
        _ => {}
    }
}

fn metric_names(query: &str) -> impl Iterator<Item = &str> {
    query
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
        .filter(|token| token.starts_with("kubecove_server_") || token.starts_with("process_"))
}

/// Labels in `by (...)` clauses.
fn grouping_labels(query: &str) -> impl Iterator<Item = &str> {
    query
        .split("by (")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .flat_map(|(list, _)| list.split(','))
        .map(str::trim)
}

/// Label names in `{...}` selectors, with the value of exact, literal matches.
fn matchers(query: &str) -> Vec<(&str, Option<&str>)> {
    let mut found = Vec::new();
    for selector in query
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split_once('}'))
        .map(|(selector, _)| selector)
    {
        for matcher in selector.split(',').map(str::trim) {
            let Some(at) = matcher.find(['=', '!']) else {
                continue;
            };
            let (label, rest) = (matcher[..at].trim(), &matcher[at..]);
            let value = rest
                .strip_prefix('=')
                .filter(|value| !value.starts_with('~'))
                .map(|value| value.trim().trim_matches('"'))
                .filter(|value| !value.contains('$'));
            found.push((label, value));
        }
    }
    found
}

fn legend_labels(legend: &str) -> impl Iterator<Item = &str> {
    legend
        .split("{{")
        .skip(1)
        .filter_map(|rest| rest.split_once("}}"))
        .map(|(label, _)| label.trim())
}
