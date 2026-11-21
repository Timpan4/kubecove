//! Project untrusted browser state field by field. Never clone a source object.
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

static SENSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r#"(?ix)-----BEGIN[^\r\n]*(?:PRIVATE\sKEY|CERTIFICATE)-----|\b(?:bearer|basic)\s+\S+|"#,
        r#"(?:client-key-data|client-certificate-data|certificate-authority-data)["']?\s*[:=]|"#,
        r#"(?:password|passwd|authorization|cookie|token|secret|signature|credential|api[_-]?key|private[_-]?key)["']?\s*[:=]|"#,
        r#"\b(?:gh[pousr]_|github_pat_|glpat-|sk-(?:proj-)?|xox[baprs]-)[a-z0-9_-]+|"#,
        r#"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b|\beyJ[\w-]+\.[\w-]+\.[\w-]+|"#,
        r#"[a-z][a-z0-9+.-]*://[^\s/]*@|https?://[^\s]*[?\#]"#
    )).expect("valid credential patterns")
});

fn safe(text: &str) -> bool {
    !SENSITIVE.is_match(text)
}

fn text(value: &Value) -> Option<Value> {
    value
        .as_str()
        .filter(|s| safe(s))
        .map(|s| Value::String(s.into()))
}

fn fields(input: &Value, names: &str) -> Value {
    let mut out = Map::new();
    for name in names.split_ascii_whitespace() {
        if let Some(value) = text(&input[name]) {
            out.insert(name.into(), value);
        }
    }
    Value::Object(out)
}

fn booleans(input: &Value, out: &mut Value, names: &str) {
    for name in names.split_ascii_whitespace() {
        if let Some(v) = input[name].as_bool() {
            out[name] = v.into();
        }
    }
}

fn array(value: &Value, project: impl Fn(&Value) -> Option<Value>) -> Value {
    Value::Array(
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(project)
            .collect(),
    )
}

fn profile(p: &Value) -> Option<Value> {
    let mut out = fields(p, "id workspaceId clusterContext kubeconfigSourceKey");
    out.get("id")?;
    let e = &p["endpoint"];
    let endpoint = match e["kind"].as_str() {
        Some("serviceTunnel") => {
            let mut t = fields(
                e,
                "kind namespace serviceName scheme rootPath tlsServerName",
            );
            if e["rootPath"]
                .as_str()
                .is_some_and(|p| p.contains(['?', '#']))
            {
                return None;
            }
            t["servicePort"] = e["servicePort"].as_u64()?.into();
            t
        }
        Some("externalHttps") | None => {
            let raw = e.get("url").unwrap_or(&p["url"]).as_str()?;
            let url = reqwest::Url::parse(raw).ok()?;
            if !safe(raw)
                || url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return None;
            }
            json!({"kind":"externalHttps", "url":raw})
        }
        _ => return None,
    };
    out["endpoint"] = endpoint;
    // Saved web profiles omit the transport used by the connected-profile wire type.
    if p.get("transport")
        .is_some_and(|v| v.as_str() != Some("connected"))
    {
        return None;
    }
    out["transport"] = "connected".into();
    out["rememberCredential"] = p["rememberCredential"].as_bool().unwrap_or(false).into();
    serde_json::from_value::<crate::models::ArgoConnectionProfile>(out.clone()).ok()?;
    Some(out)
}

pub(super) fn settings(input: &Value) -> Value {
    let mut out = fields(
        input,
        "gitOpsViewMode helmViewMode timestampTimezone yamlViewModeDefault yamlEncodingDefault yamlDiffStyle logLayout",
    );
    booleans(
        input,
        &mut out,
        "showExactTimestamps showUsageFooter measureWorkingSet expandTimelineByDefault showOwnershipMapByDefault showFullTopologyOnSelection showUnavailableGitOpsProviders skipChangeReview redactSecrets showCustomResources debugModeEnabled autoStartSavedPortForwards keepLiveSessionsOnWorkspaceSwitch allowYamlForceConflicts yamlErrorLensEnabled",
    );
    out["argoProfiles"] = array(&input["argoProfiles"], profile);
    let mut connections = Map::new();
    if let Some(prefs) = input["argoConnectionPreferences"].as_object() {
        for (id, pref) in prefs {
            if !safe(id) {
                continue;
            }
            let choice = match pref["kind"].as_str() {
                Some("automatic" | "kubernetes") => fields(pref, "kind"),
                Some("connected") if text(&pref["profileId"]).is_some() => {
                    fields(pref, "kind profileId")
                }
                _ => continue,
            };
            connections.insert(id.clone(), choice);
        }
    }
    out["argoConnectionPreferences"] = connections.into();
    if let Some(lanes) = input["resourceKindLanes"].as_object() {
        for lane in ["first", "normal", "last"] {
            if let Some(kinds) = lanes.get(lane) {
                out["resourceKindLanes"][lane] = array(kinds, text);
            }
        }
    }
    out
}

fn scope(s: &Value) -> Value {
    let mut out = fields(s, "clusterContext layout gitOpsFilter argoAppFilter");
    out["namespaces"] = array(&s["namespaces"], text);
    out["kinds"] = array(&s["kinds"], |k| {
        if !k.is_object() {
            return text(k);
        }
        let mut kind = fields(k, "group version apiVersion kind plural");
        booleans(k, &mut kind, "namespaced");
        kind["shortNames"] = array(&k["shortNames"], text);
        Some(kind)
    });
    if let Some(g) = s.get("clusterGroup").filter(|g| g.is_object()) {
        let mut group = fields(g, "id name");
        group["members"] = array(&g["members"], text);
        out["clusterGroup"] = group;
    }
    if let Some(p) = s.get("shortcutPreferences") {
        let mut prefs = json!({});
        booleans(
            p,
            &mut prefs,
            "includeResources includeNamespaces includeCompare includeArgo",
        );
        out["shortcutPreferences"] = prefs;
    }
    out
}

fn shortcut(s: &Value) -> Option<Value> {
    let mut out = fields(s, "id label kind namespace argoApp");
    out.get("id")?;
    if let Some(c) = s.get("compare").filter(|c| c.is_object()) {
        let mut compare = fields(c, "id kind label leftLabel rightLabel");
        for key in ["clusterContexts", "namespaces"] {
            compare[key] = array(&c[key], text);
        }
        out["compare"] = compare;
    }
    Some(out)
}

fn required(record: &Value, names: &str) -> bool {
    names
        .split_ascii_whitespace()
        .all(|name| record[name].as_str().is_some_and(|s| !s.trim().is_empty()))
}

fn port(value: &Value) -> Option<u16> {
    u16::try_from(value.as_u64()?).ok().filter(|n| *n != 0)
}

fn port_forward(p: &Value) -> Option<Value> {
    let mut out = fields(
        p,
        "id clusterContext namespace serviceName label createdAt updatedAt",
    );
    if !required(
        &out,
        "id clusterContext namespace serviceName createdAt updatedAt",
    ) {
        return None;
    }
    out["servicePort"] = port(&p["servicePort"])?.into();
    if !p["localPort"].is_null() {
        out["localPort"] = port(&p["localPort"])?.into();
    }
    Some(out)
}

fn rbac_review(r: &Value) -> Option<Value> {
    let mut out = fields(
        r,
        "clusterContext evidenceFingerprint disposition note reviewedAt",
    );
    if !required(&out, "clusterContext note")
        || !matches!(out["disposition"].as_str(), Some("expected" | "anomalous"))
    {
        return None;
    }
    let suffix = out["evidenceFingerprint"]
        .as_str()?
        .strip_prefix("rbac-v1-")?;
    if suffix.is_empty()
        || !suffix
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(out["reviewedAt"].as_str()?).ok()?;
    // RBAC keys delimit category, kind, namespace and name with colons. A namespace
    // named "token" is an identity component, while a note containing "token:" is sensitive.
    let key = r["objectKey"].as_str()?;
    let mut parts = key.splitn(4, ':');
    let (category, kind, namespace, name) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if !matches!(
        (category, kind),
        ("Roles", "Role")
            | ("Cluster Roles", "ClusterRole")
            | ("Bindings", "RoleBinding" | "ClusterRoleBinding")
            | ("Service Accounts", "ServiceAccount")
    ) || namespace.is_empty()
        || !safe(namespace)
        || name.trim().is_empty()
        || !safe(name)
    {
        return None;
    }
    out["objectKey"] = key.into();
    Some(out)
}

fn workspace(w: &Value) -> Option<Value> {
    let mut out = fields(w, "id name sharedKey createdAt updatedAt");
    out.get("id")?;
    out.get("name")?;
    out["scope"] = scope(w.get("scope")?);
    out["shortcuts"] = array(&w["shortcuts"], shortcut);
    out["portForwards"] = array(&w["portForwards"], port_forward);
    out["rbacReviews"] = array(&w["rbacReviews"], rbac_review);
    for key in ["pinned", "recent"] {
        out["entryPoints"][key] = array(&w["entryPoints"][key], |e| {
            let entry = fields(
                e,
                "kind clusterContext namespace name resourceKind apiVersion lastVisitedAt",
            );
            entry.get("name")?;
            Some(entry)
        });
    }
    Some(out)
}

pub(super) fn workspaces(input: &Value) -> Value {
    json!({"workspaces":array(&input["workspaces"], workspace)})
}
