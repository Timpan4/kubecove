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
    },
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
    if enabled("includeCompare") && (saved.contexts().len() >= 2 || saved.namespaces.len() >= 2) {
        shortcuts.push(Shortcut {
            id: "compare".into(),
            label: "Compare".into(),
            target: Target::Compare { namespace: None },
        });
    }
    shortcuts
}
