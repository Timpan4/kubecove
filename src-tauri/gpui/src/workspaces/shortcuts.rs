//! Saved shortcut identities, matching WorkspaceShortcut and makeWorkspaceShortcuts.
use super::SavedWorkspace;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub id: String,
    pub label: String,
    #[serde(flatten)]
    pub target: Target,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Target {
    Resources {
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },
    Namespace {
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },
    Argo {
        #[serde(skip_serializing_if = "Option::is_none")]
        argo_app: Option<String>,
    },
    Compare {
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        compare: Option<CompareEntry>,
    },
}

/// `WorkspaceCompareEntry`: the two sides a Compare shortcut was generated for.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEntry {
    pub id: String,
    pub kind: CompareKind,
    pub label: String,
    pub left_label: String,
    pub right_label: String,
    pub cluster_contexts: Vec<String>,
    pub namespaces: Vec<String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CompareKind {
    Contexts,
    Namespaces,
}

/// `buildWorkspaceCompareEntries`: the first two contexts, then the first two namespaces.
pub fn compare_entries(saved: &SavedWorkspace) -> Vec<CompareEntry> {
    let contexts: Vec<String> = saved.contexts().into_iter().map(str::to_owned).collect();
    let mut entries = vec![];
    if let [left, right, ..] = contexts.as_slice() {
        entries.push(CompareEntry {
            id: format!("ctx:{left}:{right}"),
            kind: CompareKind::Contexts,
            label: "Compare contexts".into(),
            left_label: left.clone(),
            right_label: right.clone(),
            cluster_contexts: vec![left.clone(), right.clone()],
            namespaces: saved.namespaces.clone(),
        });
    }
    if let [left, right, ..] = saved.namespaces.as_slice() {
        entries.push(CompareEntry {
            id: format!("namespaces:{left}:{right}"),
            kind: CompareKind::Namespaces,
            label: "Compare namespaces".into(),
            left_label: left.clone(),
            right_label: right.clone(),
            cluster_contexts: contexts.clone(),
            namespaces: vec![left.clone(), right.clone()],
        });
    }
    entries
}

pub fn for_workspace(
    saved: &SavedWorkspace,
    document: Option<&serde_json::Value>,
) -> Result<Vec<Shortcut>, String> {
    if let Some(shortcuts) = document.and_then(|document| document["spec"].get("shortcuts")) {
        return serde_json::from_value(shortcuts.clone())
            .map_err(|error| format!("Invalid workspace shortcuts: {error}"));
    }
    Ok(generated(saved, document))
}

pub fn generated(saved: &SavedWorkspace, document: Option<&serde_json::Value>) -> Vec<Shortcut> {
    let enabled = |key| {
        document
            .and_then(|document| document["spec"]["scope"]["shortcutPreferences"][key].as_bool())
            .unwrap_or(true)
    };
    let mut shortcuts = vec![];
    if enabled("includeResources") {
        shortcuts.push(Shortcut {
            id: "resources:all".into(),
            label: "Resources".into(),
            target: Target::Resources { namespace: None },
        });
    }
    // The web's makeWorkspaceShortcuts includes its first four saved namespaces.
    if enabled("includeNamespaces") {
        shortcuts.extend(saved.namespaces.iter().take(4).map(|namespace| Shortcut {
            id: format!("namespace:{namespace}"),
            label: namespace.clone(),
            target: Target::Namespace {
                namespace: Some(namespace.clone()),
            },
        }));
    }
    if enabled("includeCompare")
        && let Some(compare) = compare_entries(saved).into_iter().next()
    {
        shortcuts.push(Shortcut {
            id: format!("compare:{}", compare.id),
            label: "Compare".into(),
            target: Target::Compare {
                namespace: None,
                compare: Some(compare),
            },
        });
    }
    shortcuts
}
