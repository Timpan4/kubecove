//! Native persistence types mirror the browser's partializeSettings contract.
mod store;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use store::Store;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimestampTimezone {
    #[default]
    Local,
    Utc,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YamlShape {
    #[default]
    Kubectl,
    ApplyClean,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YamlFormat {
    #[default]
    Yaml,
    Kyaml,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffStyle {
    #[default]
    Clean,
    Git,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLayout {
    #[default]
    Stacked,
    Gutter,
    OneLine,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeliveryMode {
    #[default]
    Cards,
    List,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub show_exact_timestamps: bool,
    pub show_usage_footer: bool,
    pub measure_working_set: bool,
    pub expand_timeline_by_default: bool,
    pub show_ownership_map_by_default: bool,
    pub show_full_topology_on_selection: bool,
    #[serde(rename = "showUnavailableGitOpsProviders")]
    pub show_unavailable_git_ops_providers: bool,
    pub skip_change_review: bool,
    pub redact_secrets: bool,
    pub show_custom_resources: bool,
    pub debug_mode_enabled: bool,
    pub auto_start_saved_port_forwards: bool,
    pub keep_live_sessions_on_workspace_switch: bool,
    pub allow_yaml_force_conflicts: bool,
    pub yaml_error_lens_enabled: bool,
    #[serde(rename = "gitOpsViewMode")]
    pub git_ops_view_mode: DeliveryMode,
    pub helm_view_mode: DeliveryMode,
    pub timestamp_timezone: TimestampTimezone,
    pub yaml_view_mode_default: YamlShape,
    pub yaml_encoding_default: YamlFormat,
    pub yaml_diff_style: DiffStyle,
    pub log_layout: LogLayout,
    pub resource_kind_lanes: KindLanes,
    pub argo_profiles: Vec<ArgoProfile>,
    pub argo_connection_preferences: BTreeMap<String, ConnectionPreference>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_exact_timestamps: false,
            show_usage_footer: false,
            measure_working_set: false,
            expand_timeline_by_default: false,
            show_ownership_map_by_default: true,
            show_full_topology_on_selection: false,
            show_unavailable_git_ops_providers: false,
            skip_change_review: false,
            redact_secrets: true,
            show_custom_resources: true,
            debug_mode_enabled: false,
            auto_start_saved_port_forwards: false,
            keep_live_sessions_on_workspace_switch: false,
            allow_yaml_force_conflicts: false,
            yaml_error_lens_enabled: true,
            git_ops_view_mode: DeliveryMode::default(),
            helm_view_mode: DeliveryMode::default(),
            timestamp_timezone: TimestampTimezone::default(),
            yaml_view_mode_default: YamlShape::default(),
            yaml_encoding_default: YamlFormat::default(),
            yaml_diff_style: DiffStyle::default(),
            log_layout: LogLayout::default(),
            resource_kind_lanes: KindLanes::default(),
            argo_profiles: Vec::new(),
            argo_connection_preferences: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgoProfile {
    pub id: String,
    pub endpoint: Endpoint,
    pub cluster_context: Option<String>,
    pub workspace_id: Option<String>,
    pub kubeconfig_source_key: Option<String>,
    pub remember_credential: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Endpoint {
    ExternalHttps {
        url: String,
    },
    ServiceTunnel {
        namespace: String,
        service_name: String,
        service_port: u16,
        scheme: String,
        root_path: Option<String>,
        tls_server_name: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectionPreference {
    Automatic,
    Kubernetes,
    Connected { profile_id: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KindLanes {
    pub first: Vec<String>,
    pub normal: Vec<String>,
    pub last: Vec<String>,
}
impl Default for KindLanes {
    fn default() -> Self {
        let strings = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect();
        Self {
            first: strings(&["Pod", "Deployment", "StatefulSet", "DaemonSet"]),
            normal: strings(&[
                "CronJob",
                "Job",
                "Service",
                "Ingress",
                "ConfigMap",
                "Secret",
                "PersistentVolumeClaim",
                "*",
            ]),
            last: strings(&["ReplicaSet", "Node", "PersistentVolume", "StorageClass"]),
        }
    }
}
impl KindLanes {
    pub fn order(&self, kind: &str) -> usize {
        let mut fallback = None;
        let mut count = 0;
        for (index, candidate) in self
            .first
            .iter()
            .chain(&self.normal)
            .chain(&self.last)
            .enumerate()
        {
            if candidate == kind {
                return index;
            }
            if candidate == "*" {
                fallback = Some(index);
            }
            count = index + 1;
        }
        fallback.unwrap_or(count)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Updates {
    pub last_checked_at: Option<String>,
    pub dismissed_version: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedForward {
    pub request: kubecove_lib::models::PortForwardRequest,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    pub revision: u64,
    pub preferences: Preferences,
    pub updates: Updates,
    pub workspaces: Vec<crate::workspaces::SavedWorkspace>,
    pub workspace_documents: BTreeMap<u64, serde_json::Value>,
    pub workspace_entry_points: BTreeMap<u64, crate::workspaces::entries::Entries>,
    pub saved_forward_presets: std::collections::HashMap<String, Vec<SavedForward>>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            preferences: Preferences::default(),
            updates: Updates::default(),
            workspaces: Vec::new(),
            workspace_documents: BTreeMap::new(),
            workspace_entry_points: BTreeMap::new(),
            saved_forward_presets: std::collections::HashMap::new(),
        }
    }
}
