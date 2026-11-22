//! Workspace, RBAC review, entry-point and port-forward merging for Tauri
//! migration snapshots. Existing GPUI entries win on conflict.

use super::{is_safe_text, parse_port, safe_nonempty, safe_optional};
use crate::{
    model::WORKSPACE_KINDS,
    preferences::{Document, SavedForward},
    rbac::review::{Disposition, ReviewRecord},
    workspaces::{
        ApiKind, SavedKind, SavedWorkspace,
        entries::{Entries, Entry, EntryKind},
        transfer,
    },
};
use kubecove_lib::models::PortForwardRequest;
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) fn merge_workspaces(
    state: &serde_json::Map<String, Value>,
    document: &mut Document,
    source_key: Option<&str>,
    workspace_id_map: &mut HashMap<String, String>,
    skipped: &mut usize,
) {
    let Some(raw_workspaces) = state.get("workspaces") else {
        return;
    };
    let Some(items) = raw_workspaces.as_array() else {
        *skipped += 1;
        return;
    };

    let mut next_id = document
        .workspaces
        .iter()
        .map(|w| w.id.max(w.updated))
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    for item in items {
        let Some(parsed) = parse_workspace(item, skipped) else {
            *skipped += 1;
            continue;
        };

        let (target_id, target_name) = if let Some(existing) = document
            .workspaces
            .iter_mut()
            .find(|w| w.name.eq_ignore_ascii_case(&parsed.name) && w.context == parsed.context)
        {
            merge_reviews(&mut existing.reviews, parsed.reviews);
            // Shortcuts live only in the retained document. GPUI's shortcuts win;
            // an empty or missing slot takes the incoming ones.
            match document.workspace_documents.get_mut(&existing.id) {
                None => {
                    if let Some(retained) = item.as_object().and_then(|raw| {
                        build_retained_document(
                            raw,
                            &existing.name,
                            &existing.context,
                            &existing.group,
                            existing.group_name.as_deref(),
                            &existing.namespaces,
                            &existing.kinds,
                        )
                    }) {
                        document.workspace_documents.insert(existing.id, retained);
                    }
                }
                Some(retained)
                    if retained["spec"]["shortcuts"]
                        .as_array()
                        .is_none_or(Vec::is_empty) =>
                {
                    let shortcuts = item
                        .get("shortcuts")
                        .map_or_else(Vec::new, sanitize_shortcuts);
                    if !shortcuts.is_empty() {
                        let mut updated = retained.clone();
                        updated["spec"]["shortcuts"] = Value::Array(shortcuts);
                        if transfer::parse(&updated.to_string()).is_ok() {
                            *retained = updated;
                        }
                    }
                }
                Some(_) => {}
            }
            (existing.id, existing.name.clone())
        } else {
            if next_id == u64::MAX {
                *skipped += 1;
                continue;
            }
            let id = next_id;
            next_id = next_id.saturating_add(1);
            let saved = SavedWorkspace {
                id,
                name: parsed.name.clone(),
                context: parsed.context.clone(),
                group: parsed.group,
                group_name: parsed.group_name,
                namespaces: parsed.namespaces,
                kinds: parsed.kinds,
                updated: id,
                reviews: parsed.reviews,
            };
            if let Some(retained) = parsed.retained_document {
                document.workspace_documents.insert(id, retained);
            }
            document.workspaces.push(saved);
            (id, parsed.name)
        };

        if let Some(source_id) = parsed.source_id {
            workspace_id_map.insert(source_id, target_id.to_string());
        }

        if !parsed.entries.pinned.is_empty() || !parsed.entries.recent.is_empty() {
            let slot = document
                .workspace_entry_points
                .entry(target_id)
                .or_default();
            merge_entries(slot, parsed.entries);
        }

        match source_key {
            Some(source_key) => file_presets(
                document,
                source_key,
                &parsed.context,
                &target_name,
                parsed.port_forwards,
            ),
            // Without a source the presets wait in the document, keyed by the
            // GPUI workspace ID so a later rename does not lose them.
            None if !parsed.port_forwards.is_empty() => merge_port_forwards(
                document
                    .tauri_migration_pending_presets
                    .entry(target_id)
                    .or_default(),
                parsed.port_forwards,
            ),
            None => {}
        }
    }
}

/// Files presets that waited for a kubeconfig source under `source_key`, for
/// the workspaces GPUI still holds, by their current context and name.
pub(super) fn file_pending_presets(document: &mut Document, source_key: &str) {
    for (id, forwards) in std::mem::take(&mut document.tauri_migration_pending_presets) {
        let Some((context, name)) = document
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .map(|w| (w.context.clone(), w.name.clone()))
        else {
            continue;
        };
        file_presets(document, source_key, &context, &name, forwards);
    }
}

fn file_presets(
    document: &mut Document,
    source_key: &str,
    context: &str,
    workspace_name: &str,
    forwards: Vec<SavedForward>,
) {
    if forwards.is_empty() {
        return;
    }
    // Matches `Inspector::preset_key`: source, context, workspace name.
    let preset_key =
        serde_json::to_string(&(Some(source_key), context, workspace_name)).expect("string tuple");
    let slot = document
        .saved_forward_presets
        .entry(preset_key)
        .or_default();
    merge_port_forwards(slot, forwards);
}

struct ParsedWorkspace {
    source_id: Option<String>,
    name: String,
    context: String,
    group: Vec<String>,
    group_name: Option<String>,
    namespaces: Vec<String>,
    kinds: Vec<SavedKind>,
    reviews: Vec<ReviewRecord>,
    entries: Entries,
    port_forwards: Vec<SavedForward>,
    retained_document: Option<Value>,
}

fn parse_workspace(value: &Value, skipped: &mut usize) -> Option<ParsedWorkspace> {
    let obj = value.as_object()?;
    let source_id = safe_optional(obj.get("id"))?;
    let name = safe_nonempty(obj.get("name"))?;
    let scope = obj.get("scope")?.as_object()?;
    let context = safe_nonempty(scope.get("clusterContext"))?;

    let mut namespaces = Vec::new();
    if let Some(raw_ns) = scope.get("namespaces").and_then(Value::as_array) {
        for ns in raw_ns {
            match safe_nonempty(Some(ns)) {
                Some(ns) => namespaces.push(ns),
                None => *skipped += 1,
            }
        }
        namespaces.sort_by(|a, b| crate::collate::collate(a, b, false));
        namespaces.dedup();
    }

    let mut group = Vec::new();
    let mut group_name = None;
    if let Some(raw_group) = scope.get("clusterGroup").and_then(Value::as_object) {
        if let Some(members) = raw_group.get("members").and_then(Value::as_array) {
            for member in members {
                match safe_nonempty(Some(member)) {
                    Some(m) if m != context && !group.contains(&m) => group.push(m),
                    Some(_) => {}
                    None => *skipped += 1,
                }
            }
        }
        if !group.is_empty() {
            group_name =
                safe_nonempty(raw_group.get("name")).or_else(|| Some(format!("{name} group")));
        }
    }

    let mut kinds = Vec::new();
    if let Some(raw_kinds) = scope.get("kinds").and_then(Value::as_array) {
        for raw_kind in raw_kinds {
            match parse_saved_kind(raw_kind) {
                Some(kind) => {
                    if !kinds
                        .iter()
                        .any(|existing: &SavedKind| existing.key() == kind.key())
                    {
                        kinds.push(kind);
                    }
                }
                None => *skipped += 1,
            }
        }
    }
    if kinds.is_empty() {
        kinds = WORKSPACE_KINDS
            .iter()
            .map(|kind| SavedKind::Name((*kind).to_owned()))
            .collect();
    }

    let mut reviews = Vec::new();
    if let Some(raw_reviews) = obj.get("rbacReviews").and_then(Value::as_array) {
        for raw_review in raw_reviews {
            match parse_rbac_review(raw_review) {
                Some(review) => {
                    if !reviews.iter().any(|r: &ReviewRecord| {
                        r.cluster_context == review.cluster_context
                            && r.object_key == review.object_key
                    }) {
                        reviews.push(review);
                    }
                }
                None => *skipped += 1,
            }
        }
    }

    let entries = parse_entries(obj.get("entryPoints"), skipped);

    let mut port_forwards = Vec::new();
    if let Some(raw_forwards) = obj.get("portForwards").and_then(Value::as_array) {
        for raw_forward in raw_forwards {
            match parse_port_forward(raw_forward) {
                Some(forward) => port_forwards.push(forward),
                None => *skipped += 1,
            }
        }
    }

    let retained_document = build_retained_document(
        obj,
        &name,
        &context,
        &group,
        group_name.as_deref(),
        &namespaces,
        &kinds,
    );

    Some(ParsedWorkspace {
        source_id,
        name,
        context,
        group,
        group_name,
        namespaces,
        kinds,
        reviews,
        entries,
        port_forwards,
        retained_document,
    })
}

fn parse_saved_kind(value: &Value) -> Option<SavedKind> {
    if value.is_string() {
        return safe_nonempty(Some(value)).map(SavedKind::Name);
    }
    let obj = value.as_object()?;
    let namespaced = obj.get("namespaced")?.as_bool()?;
    // The core API group is the empty string.
    let group = obj.get("group")?.as_str()?.trim();
    if !group.is_empty() && !is_safe_text(group) {
        return None;
    }
    Some(SavedKind::Api(ApiKind {
        group: group.to_owned(),
        version: safe_nonempty(obj.get("version"))?,
        api_version: safe_nonempty(obj.get("apiVersion"))?,
        kind: safe_nonempty(obj.get("kind"))?,
        plural: safe_nonempty(obj.get("plural"))?,
        namespaced,
    }))
}

fn parse_rbac_review(value: &Value) -> Option<ReviewRecord> {
    let obj = value.as_object()?;
    let cluster_context = safe_nonempty(obj.get("clusterContext"))?;
    let note = safe_nonempty(obj.get("note"))?;
    let disposition = match obj.get("disposition")?.as_str()? {
        "expected" => Disposition::Expected,
        "anomalous" => Disposition::Anomalous,
        _ => return None,
    };
    let evidence_fingerprint = safe_nonempty(obj.get("evidenceFingerprint"))?;
    let suffix = evidence_fingerprint.strip_prefix("rbac-v1-")?;
    if suffix.is_empty()
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return None;
    }
    let reviewed_at = safe_nonempty(obj.get("reviewedAt"))?;
    chrono::DateTime::parse_from_rfc3339(&reviewed_at).ok()?;

    // RBAC keys delimit category, kind, namespace and name with colons. A namespace
    // named "token" is an identity component, while a note containing "token:" is sensitive.
    let object_key = obj.get("objectKey")?.as_str()?;
    let mut parts = object_key.splitn(4, ':');
    let (category, kind, namespace, name) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if !matches!(
        (category, kind),
        ("Roles", "Role")
            | ("Cluster Roles", "ClusterRole")
            | ("Bindings", "RoleBinding" | "ClusterRoleBinding")
            | ("Service Accounts", "ServiceAccount")
    ) || namespace.is_empty()
        || !is_safe_text(namespace)
        || name.trim().is_empty()
        || !is_safe_text(name)
    {
        return None;
    }

    Some(ReviewRecord {
        cluster_context,
        object_key: object_key.to_owned(),
        evidence_fingerprint,
        disposition,
        note,
        reviewed_at,
    })
}

fn parse_entries(value: Option<&Value>, skipped: &mut usize) -> Entries {
    let mut out = Entries::default();
    let Some(obj) = value.and_then(Value::as_object) else {
        return out;
    };
    if let Some(pinned) = obj.get("pinned").and_then(Value::as_array) {
        for item in pinned {
            match parse_entry(item) {
                Some(entry) => {
                    if !out.pinned.iter().any(|e| e.same_target(&entry)) {
                        out.pinned.push(entry);
                    }
                }
                None => *skipped += 1,
            }
        }
    }
    if let Some(recent) = obj.get("recent").and_then(Value::as_array) {
        for item in recent {
            match parse_entry(item) {
                Some(entry) => {
                    if !out.recent.iter().any(|e| e.same_target(&entry)) {
                        out.recent.push(entry);
                    }
                }
                None => *skipped += 1,
            }
        }
        out.recent.truncate(8);
    }
    out
}

fn parse_entry(value: &Value) -> Option<Entry> {
    let obj = value.as_object()?;
    let kind = match obj.get("kind")?.as_str()? {
        "namespace" => EntryKind::Namespace,
        "app" => EntryKind::App,
        "resource" => EntryKind::Resource,
        _ => return None,
    };
    let cluster_context = safe_nonempty(obj.get("clusterContext"))?;
    let namespace = safe_optional(obj.get("namespace"))?;
    let name = safe_nonempty(obj.get("name"))?;
    let resource_kind = safe_optional(obj.get("resourceKind"))?;
    let api_version = safe_optional(obj.get("apiVersion"))?;
    let last_visited_at = safe_nonempty(obj.get("lastVisitedAt"))?;
    let entry = Entry {
        kind,
        cluster_context,
        namespace,
        name,
        resource_kind,
        api_version,
        last_visited_at,
    };
    entry.valid().then_some(entry)
}

fn parse_port_forward(value: &Value) -> Option<SavedForward> {
    let obj = value.as_object()?;
    let cluster_context = safe_nonempty(obj.get("clusterContext"))?;
    let namespace = safe_nonempty(obj.get("namespace"))?;
    let service_name = safe_nonempty(obj.get("serviceName"))?;
    let service_port = parse_port(obj.get("servicePort")?)?;
    let local_port = match obj.get("localPort") {
        None | Some(Value::Null) => None,
        Some(v) => Some(parse_port(v)?),
    };
    let label = match obj.get("label") {
        None | Some(Value::Null) => String::new(),
        Some(v) => {
            let s = v.as_str()?.trim();
            if !is_safe_text(s) {
                return None;
            }
            s.to_owned()
        }
    };
    Some(SavedForward {
        label,
        request: PortForwardRequest {
            cluster_context,
            kubeconfig_env_var: None,
            namespace,
            target_kind: Some("Service".into()),
            target_name: Some(service_name),
            pod_name: None,
            remote_port: i64::from(service_port),
            local_port: local_port.map(i64::from),
        },
    })
}

/// Retains a `kubecove.dev/workspace/v1` document when the Tauri workspace carried
/// custom shortcuts so GPUI preserves them for display and export.
fn build_retained_document(
    workspace: &serde_json::Map<String, Value>,
    name: &str,
    context: &str,
    group: &[String],
    group_name: Option<&str>,
    namespaces: &[String],
    kinds: &[SavedKind],
) -> Option<Value> {
    let shortcuts = sanitize_shortcuts(workspace.get("shortcuts")?);
    if shortcuts.is_empty() {
        return None;
    }
    let layout = match workspace
        .get("scope")
        .and_then(|s| s.get("layout"))
        .and_then(Value::as_str)
    {
        Some("resources") => "resources",
        _ => "overview",
    };
    let metadata_name = safe_nonempty(workspace.get("sharedKey")).unwrap_or_else(|| {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let trimmed = slug.trim_matches('-');
        if trimmed.is_empty() {
            "workspace".into()
        } else {
            trimmed.into()
        }
    });
    // The shared workspace format requires a nonempty group, so core-group
    // discovered kinds stay only on the saved workspace.
    let kinds: Vec<&SavedKind> = kinds
        .iter()
        .filter(|kind| !matches!(kind, SavedKind::Api(api) if api.group.is_empty()))
        .collect();
    let mut scope = json!({
        "clusterContext": context,
        "namespaces": namespaces,
        "kinds": kinds,
        "layout": layout,
    });
    if !group.is_empty() {
        let mut members = Vec::with_capacity(group.len() + 1);
        members.push(context.to_owned());
        members.extend(group.iter().cloned());
        scope["clusterGroup"] = json!({
            "id": format!("group:{metadata_name}"),
            "name": group_name.unwrap_or("Cluster group"),
            "members": members,
        });
    }
    let doc = json!({
        "apiVersion": transfer::API_VERSION,
        "kind": "Workspace",
        "metadata": { "name": metadata_name },
        "spec": {
            "displayName": name,
            "scope": scope,
            "shortcuts": shortcuts,
            "portForwards": [],
        }
    });
    transfer::parse(&doc.to_string()).ok()?;
    Some(doc)
}

fn sanitize_shortcuts(value: &Value) -> Vec<Value> {
    value.as_array().map_or_else(Vec::new, |items| {
        items.iter().filter_map(sanitize_shortcut).collect()
    })
}

fn sanitize_shortcut(value: &Value) -> Option<Value> {
    let obj = value.as_object()?;
    let id = safe_nonempty(obj.get("id"))?;
    let label = safe_nonempty(obj.get("label"))?;
    let kind = obj.get("kind")?.as_str()?;
    if !matches!(kind, "resources" | "namespace" | "argo" | "compare") {
        return None;
    }
    let mut out = json!({ "id": id, "label": label, "kind": kind });
    if let Some(ns) = safe_nonempty(obj.get("namespace")) {
        out["namespace"] = Value::String(ns);
    }
    if let Some(app) = safe_nonempty(obj.get("argoApp")) {
        out["argoApp"] = Value::String(app);
    }
    if let Some(compare) = obj.get("compare").and_then(Value::as_object) {
        let c_kind = compare.get("kind")?.as_str()?;
        if !matches!(c_kind, "contexts" | "namespaces") {
            return None;
        }
        let contexts: Option<Vec<String>> = compare
            .get("clusterContexts")?
            .as_array()?
            .iter()
            .map(|v| safe_nonempty(Some(v)))
            .collect();
        let namespaces: Option<Vec<String>> = compare
            .get("namespaces")?
            .as_array()?
            .iter()
            .map(|v| safe_nonempty(Some(v)))
            .collect();
        out["compare"] = json!({
            "id": safe_nonempty(compare.get("id"))?,
            "kind": c_kind,
            "label": safe_nonempty(compare.get("label"))?,
            "leftLabel": safe_nonempty(compare.get("leftLabel"))?,
            "rightLabel": safe_nonempty(compare.get("rightLabel"))?,
            "clusterContexts": contexts?,
            "namespaces": namespaces?,
        });
    }
    Some(out)
}

fn merge_reviews(existing: &mut Vec<ReviewRecord>, incoming: Vec<ReviewRecord>) {
    for review in incoming {
        if !existing.iter().any(|r| {
            r.cluster_context == review.cluster_context && r.object_key == review.object_key
        }) {
            existing.push(review);
        }
    }
}

fn merge_entries(existing: &mut Entries, incoming: Entries) {
    for entry in incoming.pinned {
        if !existing.pinned.iter().any(|e| e.same_target(&entry)) {
            existing.pinned.push(entry);
        }
    }
    for entry in incoming.recent {
        if existing.recent.len() >= 8 {
            break;
        }
        if !existing.recent.iter().any(|e| e.same_target(&entry)) {
            existing.recent.push(entry);
        }
    }
}

fn merge_port_forwards(existing: &mut Vec<SavedForward>, incoming: Vec<SavedForward>) {
    for forward in incoming {
        let duplicate = existing.iter().any(|item| {
            item.request.cluster_context == forward.request.cluster_context
                && item.request.namespace == forward.request.namespace
                && item.request.target_kind == forward.request.target_kind
                && item.request.target_name == forward.request.target_name
                && item.request.remote_port == forward.request.remote_port
                && item.request.local_port == forward.request.local_port
        });
        if !duplicate {
            existing.push(forward);
        }
    }
}
