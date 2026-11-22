//! Compact projections of backend results for agents, and limit/offset paging.
//! Rows keep the evidence an agent reasons with and drop raw health evidence trees,
//! managed fields and repeated cluster names.

use kubecove_models::{
    AppError, AppErrorKind, HealthAssessmentState, ResourceEventSummary, ResourceSummary,
};
use serde_json::{Map, Value, json};

/// Default page size chosen by the product owner on 2026-11-02 (ADR 0028).
pub const DEFAULT_PAGE_SIZE: usize = 500;

/// One page of rows and where the next one starts.
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

pub fn page<T>(
    rows: Vec<T>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Page<T>, AppError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 {
        return Err(AppError::new(
            "limit must be at least 1",
            AppErrorKind::Validation,
        ));
    }
    let total = rows.len();
    let offset = offset.unwrap_or(0).min(total);
    let items: Vec<T> = rows.into_iter().skip(offset).take(limit).collect();
    let end = offset + items.len();
    Ok(Page {
        items,
        total,
        offset,
        next_offset: (end < total).then_some(end),
    })
}

impl<T> Page<T> {
    /// `{ <key>: [...], total, offset, nextOffset? }`.
    pub fn into_json(self, key: &str, project: impl Fn(T) -> Value) -> Map<String, Value> {
        let mut body = Map::new();
        body.insert(
            key.to_owned(),
            Value::Array(self.items.into_iter().map(project).collect()),
        );
        body.insert("total".into(), self.total.into());
        body.insert("offset".into(), self.offset.into());
        if let Some(next) = self.next_offset {
            body.insert("nextOffset".into(), next.into());
        }
        body
    }
}

pub fn health_state(state: HealthAssessmentState) -> &'static str {
    match state {
        HealthAssessmentState::Healthy => "healthy",
        HealthAssessmentState::NeedsAttention => "needsAttention",
        HealthAssessmentState::Degraded => "degraded",
        HealthAssessmentState::Unknown => "unknown",
        HealthAssessmentState::NotEvaluated => "notEvaluated",
    }
}

pub fn is_unhealthy(summary: &ResourceSummary) -> bool {
    matches!(
        summary.health_assessment.state,
        HealthAssessmentState::Degraded
            | HealthAssessmentState::NeedsAttention
            | HealthAssessmentState::Unknown
    )
}

pub fn resource(summary: &ResourceSummary) -> Value {
    let mut row = Map::new();
    row.insert("kind".into(), summary.kind.clone().into());
    row.insert("name".into(), summary.name.clone().into());
    insert_some(&mut row, "namespace", summary.namespace.clone());
    insert_some(&mut row, "status", summary.status.clone());
    insert_some(&mut row, "ready", summary.ready.clone());
    insert_some(&mut row, "restarts", summary.restarts);
    row.insert("age".into(), summary.age.clone().into());
    let assessment = &summary.health_assessment;
    row.insert("health".into(), health_state(assessment.state).into());
    if is_unhealthy(summary) {
        row.insert("healthReasons".into(), json!(assessment.reasons));
    }
    insert_some(&mut row, "owner", summary.owner_ref.clone());
    let gitops = summary
        .git_ops_owner
        .as_ref()
        .map(|owner| format!("{} {}/{}", owner.provider, owner.kind, owner.name))
        .or_else(|| {
            summary
                .argo_app
                .as_ref()
                .map(|app| format!("argocd Application/{app}"))
        });
    insert_some(&mut row, "gitOps", gitops);
    insert_some(&mut row, "helmRelease", summary.helm_release.clone());
    Value::Object(row)
}

pub fn event(event: &ResourceEventSummary) -> Value {
    json!({
        "type": event.event_type,
        "reason": event.reason,
        "message": event.message,
        "count": event.count,
        "lastSeen": event.last_seen_at.as_deref().unwrap_or(&event.last_seen),
        "source": event.source,
    })
}

pub fn insert_some(row: &mut Map<String, Value>, key: &str, value: Option<impl Into<Value>>) {
    if let Some(value) = value {
        row.insert(key.to_owned(), value.into());
    }
}
