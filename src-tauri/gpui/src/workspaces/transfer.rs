//! Workspace file schema, shared with `workspace-sharing-parse.ts`.
//! Imported documents stay in the launcher's existing session memory.
use super::SavedWorkspace;
use serde_json::{Value, json};

pub const API_VERSION: &str = "kubecove.dev/workspace/v1";
pub struct ImportedWorkspace {
    pub document: Value,
    pub name: String,
    pub context: String,
    pub group: Vec<String>,
    pub group_name: Option<String>,
    pub namespaces: Vec<String>,
    pub kinds: Vec<String>,
}
pub fn parse(raw: &str) -> Result<Vec<ImportedWorkspace>, String> {
    let value: Value =
        serde_json::from_str(raw).map_err(|_| "Workspace import must be valid JSON.")?;
    if value["apiVersion"] != API_VERSION {
        return Err("Unsupported workspace export version".into());
    }
    let documents = match value["kind"].as_str() {
        Some("Workspace") => vec![value],
        Some("WorkspaceList") => value["items"]
            .as_array()
            .ok_or("WorkspaceList.items must be an array")?
            .clone(),
        _ => return Err("Workspace import kind must be Workspace or WorkspaceList.".into()),
    };
    let mut keys = std::collections::HashSet::new();
    let mut imported = Vec::new();
    for document in documents {
        if document["apiVersion"] != API_VERSION || document["kind"] != "Workspace" {
            return Err("Invalid workspace item".into());
        }
        let key = required(&document["metadata"]["name"])?;
        if !keys.insert(key) {
            return Err("Duplicate workspace metadata.name".into());
        }
        let spec = &document["spec"];
        let scope = &spec["scope"];
        if !matches!(scope["layout"].as_str(), Some("overview" | "resources")) {
            return Err("scope.layout must be overview or resources".into());
        }
        let name = required(&spec["displayName"])?;
        let context = required(&scope["clusterContext"])?;
        let namespaces = strings(&scope["namespaces"])?;
        let mut kinds = Vec::new();
        for kind in scope["kinds"]
            .as_array()
            .ok_or("scope.kinds must be an array")?
        {
            if kind.is_string() {
                kinds.push(required(kind)?);
            } else {
                for field in ["group", "version", "apiVersion", "kind", "plural"] {
                    required(&kind[field])?;
                }
                if !kind["namespaced"].is_boolean() {
                    return Err("kind.namespaced must be a boolean".into());
                }
                kinds.push(required(&kind["kind"])?);
            }
        }
        let (group, group_name) = if scope["clusterGroup"].is_null() {
            (vec![], None)
        } else {
            required(&scope["clusterGroup"]["id"])?;
            (
                strings(&scope["clusterGroup"]["members"])?,
                Some(required(&scope["clusterGroup"]["name"])?),
            )
        };
        if let Some(preferences) = scope.get("shortcutPreferences") {
            for key in [
                "includeResources",
                "includeNamespaces",
                "includeCompare",
                "includeArgo",
            ] {
                if !preferences[key].is_boolean() {
                    return Err("Invalid shortcut preferences".into());
                }
            }
        }
        for shortcut in optional_array(spec, "shortcuts")? {
            required(&shortcut["id"])?;
            required(&shortcut["label"])?;
            if !matches!(
                shortcut["kind"].as_str(),
                Some("resources" | "namespace" | "argo" | "compare")
            ) {
                return Err("Unsupported shortcut kind".into());
            }
            if let Some(compare) = shortcut.get("compare") {
                if !matches!(compare["kind"].as_str(), Some("contexts" | "namespaces")) {
                    return Err("Invalid comparison kind".into());
                }
                for key in ["id", "label", "leftLabel", "rightLabel"] {
                    required(&compare[key])?;
                }
                strings(&compare["clusterContexts"])?;
                strings(&compare["namespaces"])?;
            }
        }
        for forward in optional_array(spec, "portForwards")? {
            for key in ["clusterContext", "namespace", "serviceName"] {
                required(&forward[key])?;
            }
            port(&forward["servicePort"])?;
            if let Some(value) = forward.get("localPort") {
                port(value)?;
            }
        }
        imported.push(ImportedWorkspace {
            document,
            name,
            context,
            group,
            group_name,
            namespaces,
            kinds,
        });
    }
    Ok(imported)
}
fn required(value: &Value) -> Result<String, String> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_owned())
        .ok_or_else(|| "Workspace field must be a nonempty string".into())
}
fn strings(value: &Value) -> Result<Vec<String>, String> {
    if value.is_null() {
        return Ok(vec![]);
    }
    value
        .as_array()
        .ok_or("Workspace field must be an array")?
        .iter()
        .map(required)
        .collect()
}
fn optional_array<'a>(object: &'a Value, field: &str) -> Result<&'a [Value], String> {
    match object.get(field) {
        None => Ok(&[]),
        Some(value) => value
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| format!("{field} must be an array")),
    }
}
fn port(value: &Value) -> Result<(), String> {
    // Network port range, matching `portValue` in the web import parser.
    if value
        .as_u64()
        .is_some_and(|port| (1..=u16::MAX.into()).contains(&port))
    {
        Ok(())
    } else {
        Err("Invalid port".into())
    }
}
pub fn export(workspaces: &[(&SavedWorkspace, Option<&Value>)]) -> String {
    let mut used = std::collections::HashSet::new();
    let slug = regex::Regex::new("[^a-z0-9]+").expect("static expression");
    let items: Vec<_> = workspaces.iter().map(|(workspace, original)| {
        let base = original.and_then(|document| document["metadata"]["name"].as_str()).map(str::to_owned).unwrap_or_else(|| {
            let name = slug.replace_all(&workspace.name.to_lowercase(), "-").trim_matches('-').to_owned();
            if name.is_empty() { "workspace".to_owned() } else { name }
        });
        let mut key = base.clone();
        // Match uniqueExportNames in workspace-sharing.ts.
        let mut index = 2;
        while !used.insert(key.clone()) { key = format!("{base}-{index}"); index += 1; }
        let mut document = original.map(|value| (*value).clone()).unwrap_or_else(|| {
            let mut scope = json!({"clusterContext":workspace.context,"namespaces":workspace.namespaces,"kinds":workspace.kinds,"layout":"overview","argoAppFilter":""});
            if !workspace.group.is_empty() { scope["clusterGroup"] = json!({"id":format!("group:{}",workspace.id),"name":workspace.group_name,"members":workspace.contexts()}); }
            json!({"apiVersion":API_VERSION,"kind":"Workspace","metadata":{},"spec":{"scope":scope,"shortcuts":[],"portForwards":[]}})
        });
        document["metadata"]["name"] = json!(key);
        document["spec"]["displayName"] = json!(workspace.name);
        document
    }).collect();
    let document = if items.len() == 1 {
        items.into_iter().next().expect("one document")
    } else {
        json!({"apiVersion":API_VERSION,"kind":"WorkspaceList","metadata":{"name":"kubecove-workspaces"},"items":items})
    };
    format!(
        "{}\n",
        serde_json::to_string_pretty(&document).expect("JSON values serialize")
    )
}
