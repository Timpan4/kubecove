//! Defensive one-time import of Tauri 0.15.1's `tauri-migration.json` (ADR 0036)
//! into GPUI's `live-sessions.json` (ADR 0030). The snapshot is read-only input.
//!
//! Every field is projected and validated individually; raw source objects are
//! never cloned into the persisted document. Existing GPUI entries win on conflict.

mod workspaces;

use super::{
    ArgoProfile, ConnectionPreference, DeliveryMode, DiffStyle, Document, Endpoint, KindLanes,
    LogLayout, Preferences, TimestampTimezone, YamlFormat, YamlShape,
};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

pub(super) const MIGRATION_FILE: &str = "tauri-migration.json";

/// Mirrors `app-v0.15.1`'s `commands/migration/sanitize.rs` credential detector so
/// secrets cannot enter `live-sessions.json` even if the migration file was edited.
static SENSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r#"(?ix)-----BEGIN[^\r\n]*(?:PRIVATE\sKEY|CERTIFICATE)-----|\b(?:bearer|basic)\s+\S+|"#,
        r#"(?:client-key-data|client-certificate-data|certificate-authority-data)["']?\s*[:=]|"#,
        r#"(?:password|passwd|authorization|cookie|token|secret|signature|credential|api[_-]?key|private[_-]?key)["']?\s*[:=]|"#,
        r#"\b(?:gh[pousr]_|github_pat_|glpat-|sk-(?:proj-)?|xox[baprs]-)[a-z0-9_-]+|"#,
        r#"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b|\beyJ[\w-]+\.[\w-]+\.[\w-]+|"#,
        r#"[a-z][a-z0-9+.-]*://[^\s/]*@|https?://[^\s]*[?\#]"#
    ))
    .expect("valid credential patterns")
});

#[derive(Default)]
pub(super) struct MergeReport {
    /// The snapshot's `exportedAt`, recorded as the import marker.
    pub exported_at: String,
    /// Fields rejected as invalid or unsafe.
    pub skipped: usize,
    /// Valid Tauri entries that lost to an existing GPUI entry.
    pub kept_existing: usize,
}

/// Parses `bytes` as a schema-v1 Tauri migration snapshot and merges valid
/// non-conflicting state into `document`. Returns `Err(reason)` when the
/// top-level document is corrupt or uses an unsupported schema version, so
/// startup continues without importing.
///
/// `existing_preference_keys` names the preference keys already stored by GPUI;
/// those keep their value. `source_key` is the active kubeconfig source, which
/// scopes migrated port-forward presets the way the inspector looks them up.
/// Without one, presets wait in `Document::tauri_migration_pending_presets`.
pub(super) fn merge_bytes(
    bytes: &[u8],
    document: &mut Document,
    existing_preference_keys: &HashSet<String>,
    source_key: Option<&str>,
) -> Result<MergeReport, String> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid JSON ({error})"))?;
    let Value::Object(object) = root else {
        return Err("top-level value must be a JSON object".into());
    };
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported or missing schemaVersion (expected 1)".into());
    }
    let exported_at = object
        .get("exportedAt")
        .and_then(Value::as_str)
        .filter(|text| chrono::DateTime::parse_from_rfc3339(text).is_ok())
        .ok_or("missing or invalid exportedAt")?
        .to_owned();
    let Some(entries) = object.get("entries").and_then(Value::as_object) else {
        return Err("missing or invalid entries object".into());
    };

    let mut report = MergeReport {
        exported_at,
        ..MergeReport::default()
    };
    let mut workspace_id_map = HashMap::new();

    if let Some(workspaces_entry) = entries.get("kubecove-workspaces") {
        match entry_state(workspaces_entry) {
            Some(state) => workspaces::merge_workspaces(
                state,
                document,
                source_key,
                &mut workspace_id_map,
                &mut report.skipped,
            ),
            None => report.skipped += 1,
        }
    }

    if let Some(settings_entry) = entries.get("kubecove-settings") {
        match entry_state(settings_entry) {
            Some(state) => merge_settings(
                state,
                &mut document.preferences,
                existing_preference_keys,
                &workspace_id_map,
                &mut report,
            ),
            None => report.skipped += 1,
        }
    }

    Ok(report)
}

/// Files presets that waited for a kubeconfig source. See `merge_bytes`.
pub(super) fn file_pending_presets(document: &mut Document, source_key: &str) {
    workspaces::file_pending_presets(document, source_key);
}

fn entry_state(entry: &Value) -> Option<&serde_json::Map<String, Value>> {
    let obj = entry.as_object()?;
    if obj.get("version").is_some_and(|v| v.as_u64() != Some(0)) {
        return None;
    }
    obj.get("state")?.as_object()
}

pub(super) fn is_safe_text(text: &str) -> bool {
    !SENSITIVE.is_match(text)
}

pub(super) fn is_forbidden_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "token"
            | "password"
            | "secret"
            | "client-key-data"
            | "client-certificate-data"
            | "certificate-authority-data"
            | "kubeconfig"
    )
}

pub(super) fn safe_nonempty(value: Option<&Value>) -> Option<String> {
    let trimmed = value?.as_str()?.trim();
    if trimmed.is_empty() || !is_safe_text(trimmed) {
        return None;
    }
    Some(trimmed.to_owned())
}

pub(super) fn safe_optional(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        None | Some(Value::Null) => Some(None),
        Some(v) => safe_nonempty(Some(v)).map(Some),
    }
}

pub(super) fn parse_port(value: &Value) -> Option<u16> {
    u16::try_from(value.as_u64()?)
        .ok()
        .filter(|port| *port != 0)
}

fn merge_settings(
    state: &serde_json::Map<String, Value>,
    prefs: &mut Preferences,
    existing_preference_keys: &HashSet<String>,
    workspace_id_map: &HashMap<String, String>,
    report: &mut MergeReport,
) {
    let skipped = &mut report.skipped;
    apply_scalar_preferences(state, prefs, existing_preference_keys, skipped);

    // Incoming profiles a migrated selection may name: imported ones, and ones
    // equal to the GPUI profile that already holds the ID. A different GPUI
    // profile with the same ID is not the endpoint the user selected in Tauri.
    let mut selectable = HashSet::new();
    let mut kept_existing = HashSet::new();
    if let Some(raw_profiles) = state.get("argoProfiles") {
        match raw_profiles.as_array() {
            Some(items) => {
                for item in items {
                    let Some(profile) = parse_argo_profile(item, workspace_id_map) else {
                        *skipped += 1;
                        continue;
                    };
                    match prefs.argo_profiles.iter().find(|p| p.id == profile.id) {
                        Some(existing) if *existing == profile => {
                            selectable.insert(profile.id);
                        }
                        Some(_) => {
                            kept_existing.insert(profile.id);
                        }
                        None => {
                            selectable.insert(profile.id.clone());
                            prefs.argo_profiles.push(profile);
                        }
                    }
                }
            }
            None => *skipped += 1,
        }
    }

    if let Some(raw_conns) = state.get("argoConnectionPreferences") {
        match raw_conns.as_object() {
            Some(map) => {
                for (raw_key, raw_pref) in map {
                    let trimmed = raw_key.trim();
                    if trimmed.is_empty() || !is_safe_text(trimmed) || is_forbidden_key(trimmed) {
                        *skipped += 1;
                        continue;
                    }
                    // GPUI keys these by its own workspace ID, so an unmapped Tauri
                    // key could attach to an unrelated workspace with the same ID.
                    let Some(key) = workspace_id_map.get(trimmed).cloned() else {
                        *skipped += 1;
                        continue;
                    };
                    let Some(parsed) = parse_connection_preference(raw_pref) else {
                        *skipped += 1;
                        continue;
                    };
                    if let ConnectionPreference::Connected { profile_id } = &parsed
                        && !selectable.contains(profile_id)
                    {
                        if kept_existing.contains(profile_id) {
                            report.kept_existing += 1;
                        } else {
                            *skipped += 1;
                        }
                        continue;
                    }
                    prefs
                        .argo_connection_preferences
                        .entry(key)
                        .or_insert(parsed);
                }
            }
            None => *skipped += 1,
        }
    }
    report.kept_existing += kept_existing.len();
}

fn apply_scalar_preferences(
    state: &serde_json::Map<String, Value>,
    prefs: &mut Preferences,
    existing_preference_keys: &HashSet<String>,
    skipped: &mut usize,
) {
    let incoming = |key: &str| {
        state
            .get(key)
            .filter(|_| !existing_preference_keys.contains(key))
    };
    let mut apply_bool = |key: &str, slot: &mut bool| {
        if let Some(value) = incoming(key) {
            match value.as_bool() {
                Some(b) => *slot = b,
                None => *skipped += 1,
            }
        }
    };
    apply_bool("showExactTimestamps", &mut prefs.show_exact_timestamps);
    apply_bool("showUsageFooter", &mut prefs.show_usage_footer);
    apply_bool("measureWorkingSet", &mut prefs.measure_working_set);
    apply_bool(
        "expandTimelineByDefault",
        &mut prefs.expand_timeline_by_default,
    );
    apply_bool(
        "showOwnershipMapByDefault",
        &mut prefs.show_ownership_map_by_default,
    );
    apply_bool(
        "showFullTopologyOnSelection",
        &mut prefs.show_full_topology_on_selection,
    );
    apply_bool(
        "showUnavailableGitOpsProviders",
        &mut prefs.show_unavailable_git_ops_providers,
    );
    apply_bool("skipChangeReview", &mut prefs.skip_change_review);
    apply_bool("redactSecrets", &mut prefs.redact_secrets);
    apply_bool("showCustomResources", &mut prefs.show_custom_resources);
    apply_bool("debugModeEnabled", &mut prefs.debug_mode_enabled);
    apply_bool(
        "autoStartSavedPortForwards",
        &mut prefs.auto_start_saved_port_forwards,
    );
    apply_bool(
        "keepLiveSessionsOnWorkspaceSwitch",
        &mut prefs.keep_live_sessions_on_workspace_switch,
    );
    apply_bool(
        "allowYamlForceConflicts",
        &mut prefs.allow_yaml_force_conflicts,
    );
    apply_bool("yamlErrorLensEnabled", &mut prefs.yaml_error_lens_enabled);

    if let Some(value) = incoming("gitOpsViewMode") {
        match parse_delivery_mode(value) {
            Some(mode) => prefs.git_ops_view_mode = mode,
            None => *skipped += 1,
        }
    }
    if let Some(value) = incoming("helmViewMode") {
        match parse_delivery_mode(value) {
            Some(mode) => prefs.helm_view_mode = mode,
            None => *skipped += 1,
        }
    }
    if let Some(value) = incoming("timestampTimezone") {
        match value.as_str() {
            Some("local") => prefs.timestamp_timezone = TimestampTimezone::Local,
            Some("utc") => prefs.timestamp_timezone = TimestampTimezone::Utc,
            _ => *skipped += 1,
        }
    }
    if let Some(value) = incoming("yamlViewModeDefault") {
        match value.as_str() {
            Some("kubectl") => prefs.yaml_view_mode_default = YamlShape::Kubectl,
            Some("applyClean") => prefs.yaml_view_mode_default = YamlShape::ApplyClean,
            _ => *skipped += 1,
        }
    }
    if let Some(value) = incoming("yamlEncodingDefault") {
        match value.as_str() {
            Some("yaml") => prefs.yaml_encoding_default = YamlFormat::Yaml,
            Some("kyaml") => prefs.yaml_encoding_default = YamlFormat::Kyaml,
            _ => *skipped += 1,
        }
    }
    if let Some(value) = incoming("yamlDiffStyle") {
        match value.as_str() {
            Some("clean") => prefs.yaml_diff_style = DiffStyle::Clean,
            Some("git") => prefs.yaml_diff_style = DiffStyle::Git,
            _ => *skipped += 1,
        }
    }
    if let Some(value) = incoming("logLayout") {
        match value.as_str() {
            Some("stacked") => prefs.log_layout = LogLayout::Stacked,
            Some("gutter") => prefs.log_layout = LogLayout::Gutter,
            Some("oneLine") => prefs.log_layout = LogLayout::OneLine,
            _ => *skipped += 1,
        }
    }
    if let Some(value) = incoming("resourceKindLanes") {
        match parse_kind_lanes(value) {
            Some(lanes) => prefs.resource_kind_lanes = lanes,
            None => *skipped += 1,
        }
    }
}

fn parse_delivery_mode(value: &Value) -> Option<DeliveryMode> {
    match value.as_str()? {
        "cards" => Some(DeliveryMode::Cards),
        "list" => Some(DeliveryMode::List),
        _ => None,
    }
}

fn parse_kind_lanes(value: &Value) -> Option<KindLanes> {
    let obj = value.as_object()?;
    let parse_lane = |key: &str| -> Option<Vec<String>> {
        obj.get(key)?
            .as_array()?
            .iter()
            .map(|item| safe_nonempty(Some(item)))
            .collect()
    };
    let lanes = KindLanes {
        first: parse_lane("first")?,
        normal: parse_lane("normal")?,
        last: parse_lane("last")?,
    };
    let mut seen = HashSet::new();
    for kind in lanes.first.iter().chain(&lanes.normal).chain(&lanes.last) {
        if !seen.insert(kind.as_str()) {
            return None;
        }
    }
    seen.contains("*").then_some(lanes)
}

fn parse_argo_profile(
    value: &Value,
    workspace_id_map: &HashMap<String, String>,
) -> Option<ArgoProfile> {
    let obj = value.as_object()?;
    if obj
        .get("transport")
        .is_some_and(|v| v.as_str() != Some("connected"))
    {
        return None;
    }
    let id = safe_nonempty(obj.get("id"))?;
    let endpoint = parse_argo_endpoint(obj.get("endpoint"), obj.get("url"))?;
    let cluster_context = safe_optional(obj.get("clusterContext"))?;
    let workspace_id =
        safe_optional(obj.get("workspaceId"))?.and_then(|raw| workspace_id_map.get(&raw).cloned());
    let kubeconfig_source_key = safe_optional(obj.get("kubeconfigSourceKey"))?;
    let remember_credential = obj
        .get("rememberCredential")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(ArgoProfile {
        id,
        endpoint,
        cluster_context,
        workspace_id,
        kubeconfig_source_key,
        remember_credential,
    })
}

fn parse_argo_endpoint(endpoint: Option<&Value>, fallback_url: Option<&Value>) -> Option<Endpoint> {
    let endpoint_obj = endpoint.and_then(Value::as_object);
    match endpoint_obj
        .and_then(|e| e.get("kind"))
        .and_then(Value::as_str)
    {
        Some("serviceTunnel") => {
            let e = endpoint_obj?;
            let namespace = safe_nonempty(e.get("namespace"))?;
            let service_name = safe_nonempty(e.get("serviceName"))?;
            let service_port = parse_port(e.get("servicePort")?)?;
            let scheme = e.get("scheme")?.as_str()?;
            if !matches!(scheme, "http" | "https") {
                return None;
            }
            let root_path = safe_optional(e.get("rootPath"))?;
            if root_path.as_deref().is_some_and(|p| p.contains(['?', '#'])) {
                return None;
            }
            let tls_server_name = safe_optional(e.get("tlsServerName"))?;
            Some(Endpoint::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme: scheme.to_owned(),
                root_path,
                tls_server_name,
            })
        }
        Some("externalHttps") | None => {
            let raw_url = endpoint_obj
                .and_then(|e| e.get("url"))
                .or(fallback_url)?
                .as_str()?
                .trim();
            if !is_safe_text(raw_url) {
                return None;
            }
            let parsed = reqwest::Url::parse(raw_url).ok()?;
            if parsed.scheme() != "https"
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return None;
            }
            Some(Endpoint::ExternalHttps {
                url: raw_url.to_owned(),
            })
        }
        _ => None,
    }
}

fn parse_connection_preference(value: &Value) -> Option<ConnectionPreference> {
    let obj = value.as_object()?;
    match obj.get("kind")?.as_str()? {
        "automatic" => Some(ConnectionPreference::Automatic),
        "kubernetes" => Some(ConnectionPreference::Kubernetes),
        "connected" => Some(ConnectionPreference::Connected {
            profile_id: safe_nonempty(obj.get("profileId"))?,
        }),
        _ => None,
    }
}
