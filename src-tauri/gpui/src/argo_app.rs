//! The Argo CD Application tab model: the derived values in
//! `ArgoApplicationDetails.svelte`, `argo-workspace-model.ts`,
//! `argo-resource-display.ts` and `argo-application-yaml.ts`. It reads the
//! Kubernetes transport snapshot; the tab opens from the GitOps page, where the
//! web passes no matching Application summary, so delivery facts come from the
//! Application spec. Builds diffs once per snapshot.

use crate::unified_diff::{self, DiffLine};
use gpui_kit::SharedString;
use kubecove_lib::models::{
    ArgoApplicationHistory, ArgoApplicationInspector, ArgoManagedResource, ArgoResourceComparison,
    ResourceSummary,
};
use serde_json::Value;

fn normalized(value: Option<&str>) -> String {
    value.unwrap_or_default().trim().to_lowercase()
}

/// `argoResourceCounts` as the three signal cards show them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub needs_sync: usize,
    pub needs_attention: usize,
    pub prune: usize,
}

/// `argoResourceCounts`, with degraded and progressing summed as the card does.
pub fn counts(resources: &[ArgoManagedResource]) -> Counts {
    let mut counts = Counts::default();
    for resource in resources {
        if normalized(resource.status.as_deref()) != "synced" {
            counts.needs_sync += 1;
        }
        if matches!(
            normalized(resource.health.as_deref()).as_str(),
            "degraded" | "progressing"
        ) {
            counts.needs_attention += 1;
        }
        if resource.requires_pruning == Some(true) {
            counts.prune += 1;
        }
    }
    counts
}

/// `argoReconciliationResources`.
fn needs_reconciliation(resource: &ArgoManagedResource) -> bool {
    let health = normalized(resource.health.as_deref());
    normalized(resource.status.as_deref()) != "synced"
        || health == "degraded"
        || health == "progressing"
        || resource.requires_pruning == Some(true)
}

/// `argoResourceIdentityKey`.
pub(crate) fn identity_key(resource: &ArgoManagedResource) -> Option<String> {
    let kind = normalized(resource.kind.as_deref());
    let name = normalized(resource.name.as_deref());
    if kind.is_empty() || name.is_empty() {
        return None;
    }
    Some(format!(
        "{}:{kind}:{}:{name}",
        normalized(resource.group.as_deref()),
        normalized(resource.namespace.as_deref())
    ))
}

/// `resourceLabel`.
fn resource_label(resource: &ArgoManagedResource) -> String {
    format!(
        "{} {}{}",
        resource.kind.as_deref().unwrap_or("Resource"),
        resource
            .namespace
            .as_deref()
            .map(|namespace| format!("{namespace}/"))
            .unwrap_or_default(),
        resource.name.as_deref().unwrap_or("unknown")
    )
}

/// `historyRevision`.
fn history_revision(entry: &ArgoApplicationHistory) -> String {
    entry.revision.clone().unwrap_or_else(|| {
        let joined = entry.revisions.join(", ");
        if joined.is_empty() {
            "Unknown revision".to_owned()
        } else {
            joined
        }
    })
}

fn value_at<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value?, |current, key| match current {
        Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
        Value::Object(map) => map.get(*key),
        _ => None,
    })
}

/// `statusValue` and `stringAt`: a non-blank string at `path`.
fn string_at(value: Option<&Value>, path: &[&str]) -> Option<String> {
    value_at(value, path)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
}

fn bool_at(value: Option<&Value>, path: &[&str]) -> Option<bool> {
    value_at(value, path).and_then(Value::as_bool)
}

/// `applicationSpecFromYaml`: the Application `spec`, or nothing when the
/// YAML is missing or unreadable.
fn spec_from_yaml(yaml: &str) -> Option<Value> {
    let value: Value = serde_yaml::from_str(yaml).ok()?;
    value.get("spec").filter(|spec| spec.is_object()).cloned()
}

/// `argoDocumentText`: strings as given, other values as YAML, without trailing space.
fn document_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.trim_end().to_owned(),
        Some(value) => serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim_end()
            .to_owned(),
    }
}

/// The badge an Application snapshot article shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeBadge {
    Remove,
    Added,
    Modified,
    InSync,
}

/// One reconciliation resource with its documents (`argoComparisonDocument`).
pub struct Change {
    pub key: String,
    pub label: String,
    /// `Kind · name` in the change navigator.
    pub chip: String,
    pub status: String,
    pub prune: bool,
    pub badge: ChangeBadge,
    pub desired: SharedString,
    pub live: SharedString,
    /// The target-to-live diff, when both documents exist.
    pub diff: Option<Vec<DiffLine>>,
    /// `comparison?.exact === false`: the fallback note applies.
    pub inexact: bool,
}

pub(crate) fn change(
    resource: &ArgoManagedResource,
    comparisons: &[ArgoResourceComparison],
) -> Change {
    let key = identity_key(resource);
    let comparison = key.as_ref().and_then(|key| {
        comparisons
            .iter()
            .find(|comparison| identity_key(&comparison.resource).as_ref() == Some(key))
    });
    let target = comparison
        .and_then(|comparison| comparison.target_state.as_ref())
        .or(resource.target_state.as_ref());
    let live = comparison
        .and_then(|comparison| comparison.live_state.as_ref())
        .or(resource.live_state.as_ref());
    let normalized_live = comparison
        .and_then(|comparison| comparison.normalized_live_state.as_ref())
        .or(live);
    let (desired, live, normalized_live) = (
        document_text(target),
        document_text(live),
        document_text(normalized_live),
    );
    let prune = resource.requires_pruning == Some(true);
    let modified = comparison
        .and_then(|comparison| comparison.modified)
        .unwrap_or(desired != normalized_live);
    let badge = if prune {
        ChangeBadge::Remove
    } else if live.is_empty() && !desired.is_empty() {
        ChangeBadge::Added
    } else if modified {
        ChangeBadge::Modified
    } else {
        ChangeBadge::InSync
    };
    let diff = (!desired.is_empty() && !normalized_live.is_empty())
        .then(|| unified_diff::argo(&desired, &normalized_live));
    let label = resource_label(resource);
    Change {
        key: key.unwrap_or_else(|| label.clone()),
        chip: format!(
            "{} · {}",
            resource.kind.as_deref().unwrap_or("Resource"),
            resource.name.as_deref().unwrap_or("unknown")
        ),
        label,
        status: resource.status.clone().unwrap_or_else(|| "Unknown".into()),
        prune,
        badge,
        desired: desired.into(),
        live: live.into(),
        diff,
        inexact: comparison.and_then(|comparison| comparison.exact) == Some(false),
    }
}

/// One `history` entry as the Recent deployment card shows it.
pub struct Deployment {
    pub revision: String,
    pub deployed_at: Option<String>,
    pub initiated_by: String,
}

/// Everything the Argo CD tab renders for one snapshot.
pub struct ArgoView {
    /// The header badge: transport and provenance.
    pub source: String,
    pub kubernetes: bool,
    pub health_status: String,
    pub sync_status: String,
    pub operation_state: Option<String>,
    pub resolved_revision: String,
    pub counts: Counts,
    pub conditions: Vec<(String, String)>,
    pub changes: Vec<Change>,
    pub repository: String,
    pub path: String,
    pub configured_revision: String,
    pub destination: String,
    pub policy: String,
    pub history: Vec<Deployment>,
    pub freshness: &'static str,
}

impl ArgoView {
    pub fn new(
        inspector: &ArgoApplicationInspector,
        resource: &ResourceSummary,
        yaml: &str,
    ) -> Self {
        let status = inspector.status.as_ref();
        let spec = spec_from_yaml(yaml);
        let spec = spec.as_ref();
        let or =
            |value: Option<String>, fallback: &str| value.unwrap_or_else(|| fallback.to_owned());
        let source_at = |field: &str| {
            string_at(spec, &["source", field])
                .or_else(|| string_at(spec, &["sources", "0", field]))
        };
        let (prune, self_heal) = (
            bool_at(spec, &["syncPolicy", "automated", "prune"]),
            bool_at(spec, &["syncPolicy", "automated", "selfHeal"]),
        );
        let policy = if prune.is_none() && self_heal.is_none() {
            "Policy unavailable".to_owned()
        } else {
            let mut parts = vec!["Automated"];
            if prune == Some(true) {
                parts.push("prune");
            }
            if self_heal == Some(true) {
                parts.push("self-heal");
            }
            parts.join(" · ")
        };
        let operation = inspector.operation_state.as_ref();
        let kubernetes = inspector.transport != "connected";
        let transport = if kubernetes {
            "Kubernetes Application status"
        } else {
            "Connected Argo CD API"
        };
        Self {
            source: if inspector.provenance.is_empty() {
                transport.to_owned()
            } else {
                format!("{transport} · {}", inspector.provenance)
            },
            kubernetes,
            health_status: or(
                string_at(status, &["health", "status"]).or(resource.status.clone()),
                "Unknown",
            ),
            sync_status: or(string_at(status, &["sync", "status"]), "Unknown"),
            operation_state: string_at(operation, &["phase"])
                .or_else(|| string_at(operation, &["message"])),
            resolved_revision: or(
                string_at(status, &["sync", "revision"]).or_else(|| {
                    inspector
                        .history
                        .first()
                        .and_then(|entry| entry.revision.clone())
                }),
                "Revision unavailable",
            ),
            counts: counts(&inspector.resources),
            conditions: inspector
                .conditions
                .iter()
                .filter_map(|condition| {
                    let kind = string_at(Some(condition), &["type"]);
                    let message = string_at(Some(condition), &["message"]);
                    (kind.is_some() || message.is_some()).then(|| {
                        (
                            or(kind, "Application condition"),
                            or(message, "No condition detail reported."),
                        )
                    })
                })
                .collect(),
            changes: inspector
                .resources
                .iter()
                .filter(|resource| needs_reconciliation(resource))
                .map(|resource| change(resource, &inspector.comparisons))
                .collect(),
            repository: or(source_at("repoURL"), "Repository unavailable"),
            path: or(
                string_at(spec, &["source", "path"])
                    .or_else(|| string_at(spec, &["source", "chart"]))
                    .or_else(|| string_at(spec, &["sources", "0", "path"])),
                "Path unavailable",
            ),
            configured_revision: or(source_at("targetRevision"), "Revision unavailable"),
            destination: or(
                string_at(spec, &["destination", "namespace"])
                    .or_else(|| string_at(spec, &["destination", "server"])),
                "Destination unavailable",
            ),
            policy,
            history: inspector
                .history
                .iter()
                .map(|entry| Deployment {
                    revision: history_revision(entry),
                    deployed_at: entry.deployed_at.clone(),
                    initiated_by: entry
                        .initiated_by
                        .clone()
                        .unwrap_or_else(|| "unknown".into()),
                })
                .collect(),
            freshness: if inspector.connected_fallback.is_some() {
                "Connected Argo CD is unavailable; Kubernetes fallback snapshot loaded when this view opened."
            } else if kubernetes {
                "Kubernetes snapshot loaded when this view opened."
            } else {
                "Connected Argo CD snapshot loaded when this view opened."
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Failure modes: a synced but degraded resource dropped from the navigator,
    // a pruned resource counted as modified instead of removed, a comparison
    // matched to the wrong resource, and an unreadable spec breaking the tab.

    fn resource(
        kind: &str,
        name: &str,
        status: &str,
        health: &str,
        prune: bool,
    ) -> ArgoManagedResource {
        serde_json::from_value(json!({
            "group": "", "kind": kind, "namespace": "argocd", "name": name,
            "status": status, "health": health, "requiresPruning": prune,
        }))
        .unwrap()
    }

    #[test]
    fn reconciliation_keeps_unsettled_and_pruned_resources() {
        let resources = [
            resource("Service", "a", "Synced", "Healthy", false),
            resource("Deployment", "a", "Synced", "Degraded", false),
            resource("Secret", "a", "OutOfSync", "Missing", true),
        ];
        assert_eq!(
            counts(&resources),
            Counts {
                needs_sync: 1,
                needs_attention: 1,
                prune: 1
            }
        );
        let kept: Vec<_> = resources
            .iter()
            .filter(|r| needs_reconciliation(r))
            .collect();
        assert_eq!(kept.len(), 2);
        assert_eq!(change(kept[1], &[]).badge, ChangeBadge::Remove);
    }

    #[test]
    fn comparisons_match_by_identity() {
        let mut other = resource("ConfigMap", "b", "OutOfSync", "Healthy", false);
        other.target_state = Some(json!({"kind": "ConfigMap"}));
        let comparison: ArgoResourceComparison = serde_json::from_value(json!({
            "resource": other, "targetState": {"data": {"a": "1"}},
            "liveState": {"data": {"a": "2"}}, "availableActions": [],
        }))
        .unwrap();
        let target = resource("ConfigMap", "a", "OutOfSync", "Healthy", false);
        assert!(
            change(&target, std::slice::from_ref(&comparison))
                .diff
                .is_none()
        );
        let matched = change(&other, &[comparison]);
        assert_eq!(matched.badge, ChangeBadge::Modified);
        assert!(matched.diff.is_some());
    }

    #[test]
    fn unreadable_spec_falls_back() {
        assert!(spec_from_yaml("spec: [").is_none());
        assert_eq!(
            string_at(
                spec_from_yaml("spec:\n  sources:\n  - repoURL: r\n").as_ref(),
                &["sources", "0", "repoURL"]
            ),
            Some("r".to_owned())
        );
    }
}
