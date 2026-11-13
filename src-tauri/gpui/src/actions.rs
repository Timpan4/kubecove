//! Selected-resource actions (ADR 0004, ADR 0024), ported from
//! `src/features/resource-detail/resource-actions-model.ts` and the review
//! rules in `resourceActionSession.ts`.

use kubecove_lib::models::{
    ClusterOperationTarget, ResourceActionKind, ResourceActionPermission, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceSummary,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    Replicas,
    Rollout,
    Restart,
    Rollback,
    Image,
    Environment,
    Resources,
    Labels,
    Annotations,
    Delete,
}

impl RowId {
    pub fn key(self) -> &'static str {
        match self {
            Self::Replicas => "replicas",
            Self::Rollout => "rollout",
            Self::Restart => "restart",
            Self::Rollback => "rollback",
            Self::Image => "image",
            Self::Environment => "environment",
            Self::Resources => "resources",
            Self::Labels => "labels",
            Self::Annotations => "annotations",
            Self::Delete => "delete",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ActionRow {
    pub id: RowId,
    pub label: &'static str,
    pub group: &'static str,
    search: &'static str,
    pub action: ResourceActionKind,
}

impl ActionRow {
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || format!("{} {} {}", self.label, self.group, self.search)
                .to_lowercase()
                .contains(query)
    }
}

const fn row(
    id: RowId,
    label: &'static str,
    group: &'static str,
    search: &'static str,
    action: ResourceActionKind,
) -> ActionRow {
    ActionRow {
        id,
        label,
        group,
        search,
        action,
    }
}

const EDIT: ResourceActionKind = ResourceActionKind::Edit;
const WORKLOAD_EDIT_ROWS: [ActionRow; 3] = [
    row(RowId::Image, "Image", "Container", "image container", EDIT),
    row(
        RowId::Environment,
        "Environment",
        "Container",
        "environment env variables",
        EDIT,
    ),
    row(
        RowId::Resources,
        "Requests and limits",
        "Container",
        "resources cpu memory requests limits",
        EDIT,
    ),
];
const METADATA_ROWS: [ActionRow; 2] = [
    row(RowId::Labels, "Labels", "Metadata", "labels metadata", EDIT),
    row(
        RowId::Annotations,
        "Annotations",
        "Metadata",
        "annotations metadata",
        EDIT,
    ),
];
const DELETE_ROW: ActionRow = row(
    RowId::Delete,
    "Delete resource",
    "",
    "delete remove",
    ResourceActionKind::Delete,
);

fn builtin_api_version(kind: &str) -> Option<&'static str> {
    match kind {
        "Deployment" | "StatefulSet" | "DaemonSet" => Some("apps/v1"),
        "Pod" | "ConfigMap" => Some("v1"),
        _ => None,
    }
}

/// `isBuiltinOperationTarget`: never a CRD that shares a built-in kind name.
fn builtin_target(resource: &ResourceSummary) -> bool {
    resource.dynamic != Some(true)
        && resource
            .api_version
            .as_deref()
            .is_none_or(|version| Some(version) == builtin_api_version(&resource.kind))
}

/// `resourceActionRows`: the rows the ADR allows for this built-in kind.
pub fn rows(resource: &ResourceSummary) -> Vec<ActionRow> {
    let kind = resource.kind.as_str();
    if !builtin_target(resource) || kind == "Secret" {
        return Vec::new();
    }
    let mut rows = Vec::new();
    if matches!(kind, "Deployment" | "StatefulSet") {
        rows.push(row(
            RowId::Replicas,
            "Replicas",
            "Workload",
            "scale replicas",
            EDIT,
        ));
    }
    if kind == "Deployment" {
        rows.push(row(
            RowId::Rollout,
            "Rollout",
            "Workload",
            "pause resume rollout",
            ResourceActionKind::Pause,
        ));
    }
    if matches!(kind, "Deployment" | "StatefulSet" | "DaemonSet") {
        rows.push(row(
            RowId::Restart,
            "Restart",
            "Workload",
            "restart rollout",
            ResourceActionKind::Restart,
        ));
        if kind == "Deployment" {
            rows.push(row(
                RowId::Rollback,
                "Revision",
                "Workload",
                "rollback revision history",
                ResourceActionKind::Rollback,
            ));
        }
        rows.extend(WORKLOAD_EDIT_ROWS);
    }
    if matches!(
        kind,
        "Deployment" | "StatefulSet" | "DaemonSet" | "Pod" | "ConfigMap"
    ) {
        rows.extend(METADATA_ROWS);
        rows.push(DELETE_ROW);
    }
    rows
}

pub fn permission(
    state: Option<&ResourceActionState>,
    action: ResourceActionKind,
) -> Option<&ResourceActionPermission> {
    state?
        .permissions
        .iter()
        .find(|permission| permission.action == action)
}

/// Failed or unconfirmed permission checks never claim access.
pub fn allowed(state: Option<&ResourceActionState>, action: ResourceActionKind) -> bool {
    permission(state, action).is_some_and(|permission| permission.allowed)
}

pub fn fallback_permission(action: ResourceActionKind, kind: &str) -> String {
    let resource = match kind {
        "Deployment" => "deployments",
        "StatefulSet" => "statefulsets",
        "DaemonSet" => "daemonsets",
        "Pod" => "pods",
        "ConfigMap" => "configmaps",
        _ => "resources",
    };
    let verb = if action == ResourceActionKind::Delete {
        "delete"
    } else {
        "patch"
    };
    format!("{verb} {resource}")
}

/// The rollout row resumes a paused Deployment.
pub fn action_for_row(row: &ActionRow, state: Option<&ResourceActionState>) -> ResourceActionKind {
    if row.id == RowId::Rollout && state.and_then(|state| state.paused) == Some(true) {
        ResourceActionKind::Resume
    } else {
        row.action
    }
}

/// The wire name, as `review.action` prints in the web dialog title.
pub fn action_name(action: ResourceActionKind) -> &'static str {
    match action {
        ResourceActionKind::Edit => "edit",
        ResourceActionKind::Restart => "restart",
        ResourceActionKind::Pause => "pause",
        ResourceActionKind::Resume => "resume",
        ResourceActionKind::Rollback => "rollback",
        ResourceActionKind::Delete => "delete",
    }
}

fn entries<'a>(map: impl IntoIterator<Item = (&'a String, &'a String)>, sep: &str) -> String {
    map.into_iter()
        .map(|(key, value)| format!("{key}{sep}{value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `currentRowLabel`: the row's value in the loaded action state.
pub fn current_label(row: &ActionRow, state: Option<&ResourceActionState>, kind: &str) -> String {
    let Some(state) = state else {
        return String::new();
    };
    let containers = &state.containers;
    let or = |text: String, fallback: &str| {
        if text.is_empty() {
            fallback.to_owned()
        } else {
            text
        }
    };
    match row.id {
        RowId::Replicas => format!("{} desired", state.replicas.unwrap_or(0)),
        RowId::Rollout => if state.paused == Some(true) {
            "Paused"
        } else {
            "Running"
        }
        .into(),
        RowId::Restart => "Replace Pods gradually".into(),
        RowId::Rollback => match state.revisions.len() {
            0 => "No owned revisions found".into(),
            count => format!("{count} available revisions"),
        },
        RowId::Image => containers
            .iter()
            .map(|container| format!("{}: {}", container.name, container.image))
            .collect::<Vec<_>>()
            .join(", "),
        RowId::Environment => containers
            .iter()
            .map(|container| {
                let names = container
                    .env
                    .iter()
                    .map(|variable| variable.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: {}", container.name, or(names, "no variables"))
            })
            .collect::<Vec<_>>()
            .join("; "),
        RowId::Resources => containers
            .iter()
            .map(|container| {
                format!(
                    "{}: requests {}; limits {}",
                    container.name,
                    or(entries(&container.requests, " "), "none"),
                    or(entries(&container.limits, " "), "none")
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
        RowId::Labels => or(entries(&state.labels, "="), "No labels"),
        RowId::Annotations => or(entries(&state.annotations, "="), "No annotations"),
        RowId::Delete => {
            format!("Deletes this {kind} and its dependents where Kubernetes applies them.")
        }
    }
}

/// `actionTarget`: the exact selected resource.
pub fn target(resource: &ResourceSummary) -> ClusterOperationTarget {
    ClusterOperationTarget {
        cluster_context: resource.cluster.clone(),
        kind: resource.kind.clone(),
        name: resource.name.clone(),
        api_version: resource.api_version.clone(),
        namespace: resource.namespace.clone(),
    }
}

/// The unconfirmed request a review sends for its server dry run.
pub fn review_request(
    resource: &ResourceSummary,
    action: ResourceActionKind,
    replicas: Option<i32>,
) -> ResourceActionRequest {
    ResourceActionRequest {
        target: target(resource),
        action,
        edits: replicas.map(|replicas| kubecove_lib::models::ResourceActionEdits {
            replicas: Some(replicas),
            ..Default::default()
        }),
        revision: None,
        expected_uid: None,
        expected_resource_version: None,
        expected_revision_uid: None,
        expected_revision_resource_version: None,
        require_replacement: false,
        confirmed: false,
        kubeconfig_env_var: None,
    }
}

/// `executionRequest`: the reviewed request, guarded by the previewed identity.
pub fn execution_request(
    request: &ResourceActionRequest,
    preview: &ResourceActionPreview,
) -> ResourceActionRequest {
    let mut guarded = request.clone();
    guarded.expected_uid = Some(preview.uid.clone());
    guarded.expected_resource_version = Some(preview.resource_version.clone());
    guarded.confirmed = true;
    guarded.require_replacement = request.action == ResourceActionKind::Delete
        && preview
            .controller
            .as_ref()
            .is_some_and(|controller| controller.recreates);
    if request.action == ResourceActionKind::Rollback {
        guarded.expected_revision_uid = preview.revision_uid.clone();
        guarded.expected_revision_resource_version = preview.revision_resource_version.clone();
    }
    guarded
}
