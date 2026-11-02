//! The incident scan behind `find_problems`.

use super::{access_for, args};
use crate::{
    compact::{self, insert_some, page},
    handler::McpContext,
};
use kubecove_inspection::commands::incident_cockpit_from;
use kubecove_models::{
    AppError, IncidentCockpitItem, IncidentSeverity, IncidentSignalState, ResourceListRequest,
};
use serde_json::{Map, Value, json};
use std::cmp::Reverse;

/// Namespaced kinds whose health, restarts and Warning events make up an incident scan.
const NAMESPACED_KINDS: [&str; 7] = [
    "Pod",
    "Deployment",
    "StatefulSet",
    "DaemonSet",
    "Job",
    "CronJob",
    "PersistentVolumeClaim",
];

pub async fn find_problems(
    context: &McpContext,
    arguments: args::FindProblemsArgs,
) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let namespace = arguments
        .namespace
        .filter(|namespace| !namespace.is_empty());
    let requests = NAMESPACED_KINDS
        .iter()
        .map(|kind| request(kind, namespace.clone()))
        .chain([request("Node", None)])
        .collect();
    let summary = incident_cockpit_from(
        &access,
        cluster.clone(),
        requests,
        context.live_store.clone(),
    )
    .await?;
    let mut items: Vec<IncidentCockpitItem> = summary
        .items
        .into_iter()
        .filter(|item| arguments.include_resolved || item.state == IncidentSignalState::Active)
        .collect();
    // Active before resolved, then most severe, then most recent.
    items.sort_by_cached_key(|item| {
        (
            state_rank(item.state),
            severity_rank(item.severity),
            Reverse(item.latest_signal_at.clone()),
        )
    });
    let mut body = page(items, arguments.limit, arguments.offset)?.into_json("problems", item);
    body.insert("context".into(), cluster.into());
    body.insert("generatedAt".into(), summary.generated_at.into());
    if !summary.warnings.is_empty() {
        body.insert("warnings".into(), json!(summary.warnings));
    }
    Ok(Value::Object(body))
}

fn request(kind: &str, namespace: Option<String>) -> ResourceListRequest {
    ResourceListRequest {
        kind: Some(kind.to_owned()),
        resource_kind: None,
        namespace,
    }
}

fn state_rank(state: IncidentSignalState) -> u8 {
    match state {
        IncidentSignalState::Active => 0,
        IncidentSignalState::Resolved => 1,
        IncidentSignalState::Historical => 2,
    }
}

fn severity_rank(severity: IncidentSeverity) -> u8 {
    match severity {
        IncidentSeverity::Degraded => 0,
        IncidentSeverity::Attention => 1,
        IncidentSeverity::Restarted => 2,
        IncidentSeverity::Warning => 3,
    }
}

fn item(item: IncidentCockpitItem) -> Value {
    let Value::Object(mut row) = compact::resource(&item.resource) else {
        unreachable!("compact::resource returns an object")
    };
    row.insert("severity".into(), json!(item.severity));
    row.insert("state".into(), json!(item.state));
    row.insert(
        "signals".into(),
        Value::Array(
            item.signals
                .iter()
                .map(|signal| {
                    let mut entry = Map::new();
                    entry.insert("label".into(), signal.label.clone().into());
                    entry.insert("message".into(), signal.message.clone().into());
                    entry.insert("source".into(), signal.source.clone().into());
                    entry.insert("state".into(), json!(signal.state));
                    insert_some(&mut entry, "lastSeen", signal.last_seen_at.clone());
                    Value::Object(entry)
                })
                .collect(),
        ),
    );
    if item.warning_event_count > 0 {
        row.insert("warningEventCount".into(), item.warning_event_count.into());
    }
    if let Some(event) = &item.latest_warning_event {
        row.insert("latestWarningEvent".into(), compact::event(event));
    }
    Value::Object(row)
}
