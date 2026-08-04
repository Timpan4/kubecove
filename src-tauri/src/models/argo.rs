use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSourceSummary {
    pub repo_url: Option<String>,
    pub target_revision: Option<String>,
    pub resolved_revision: Option<String>,
    pub path: Option<String>,
    pub chart: Option<String>,
    pub source_mode: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub project: Option<String>,
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub destination_namespace: Option<String>,
    pub destination_server: Option<String>,
    pub source_repo: Option<String>,
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default)]
    pub sources: Vec<ArgoApplicationSourceSummary>,
    #[serde(default)]
    pub resource_namespaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracked_resource_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationDetails {
    pub summary: ArgoApplicationSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
    pub status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSetSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub project: Option<String>,
    pub status: Option<String>,
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub destination_namespace: Option<String>,
    pub destination_server: Option<String>,
    pub source_repo: Option<String>,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSetDetails {
    pub summary: ArgoApplicationSetSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoAppProjectSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoAppProjectDetails {
    pub summary: ArgoAppProjectSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArgoServerEndpoint {
    ExternalHttps {
        url: String,
    },
    ServiceTunnel {
        namespace: String,
        service_name: String,
        service_port: u16,
        scheme: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tls_server_name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoConnectionProfile {
    pub id: String,
    // Kept during the frontend migration so saved external profiles still display correctly.
    pub url: String,
    pub endpoint: ArgoServerEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_source_key: Option<String>,
    pub transport: String,
    pub remember_credential: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArgoConnectionProfileWire {
    id: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    endpoint: Option<ArgoServerEndpoint>,
    #[serde(default)]
    cluster_context: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    kubeconfig_source_key: Option<String>,
    transport: String,
    remember_credential: bool,
}

impl<'de> Deserialize<'de> for ArgoConnectionProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ArgoConnectionProfileWire::deserialize(deserializer)?;
        let endpoint = wire
            .endpoint
            .unwrap_or_else(|| ArgoServerEndpoint::ExternalHttps {
                url: wire.url.clone(),
            });
        let url = match &endpoint {
            ArgoServerEndpoint::ExternalHttps { url } => url.clone(),
            ArgoServerEndpoint::ServiceTunnel {
                namespace,
                service_name,
                scheme,
                root_path,
                tls_server_name,
                ..
            } => {
                let host = tls_server_name
                    .clone()
                    .unwrap_or_else(|| format!("{service_name}.{namespace}.svc"));
                format!("{scheme}://{host}{}", root_path.as_deref().unwrap_or("/"))
            }
        };
        Ok(Self {
            id: wire.id,
            url,
            endpoint,
            cluster_context: wire.cluster_context,
            workspace_id: wire.workspace_id,
            kubeconfig_source_key: wire.kubeconfig_source_key,
            transport: wire.transport,
            remember_credential: wire.remember_credential,
        })
    }
}

impl Default for ArgoConnectionProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            url: String::new(),
            endpoint: ArgoServerEndpoint::ExternalHttps { url: String::new() },
            cluster_context: None,
            workspace_id: None,
            kubeconfig_source_key: None,
            transport: String::new(),
            remember_credential: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoConnectionStatus {
    pub profile: Option<ArgoConnectionProfile>,
    pub connected: bool,
    pub username: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgoServiceTunnelUnavailableReason {
    ExternalName,
    SelectorRequired,
    NoTcpPorts,
    NoReadyPod,
    TargetUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoServerCapability {
    pub id: String,
    pub name: String,
    pub namespace: Option<String>,
    pub url: Option<String>,
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<ArgoServerEndpoint>,
    pub unavailable_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<ArgoServiceTunnelUnavailableReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationRef {
    pub name: String,
    pub namespace: Option<String>,
    pub project: Option<String>,
    pub resource_version: Option<String>,
    pub uid: Option<String>,
    pub api_version: Option<String>,
    pub context: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationHistory {
    pub id: Option<i64>,
    pub revision: Option<String>,
    pub revisions: Vec<String>,
    pub deployed_at: Option<String>,
    pub initiated_by: Option<String>,
    pub source: Option<Value>,
    pub sources: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoManagedResource {
    pub group: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub health: Option<String>,
    pub hook: Option<bool>,
    pub requires_pruning: Option<bool>,
    pub target_state: Option<Value>,
    pub live_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoResourceComparison {
    pub resource: ArgoManagedResource,
    pub target_state: Option<Value>,
    pub live_state: Option<Value>,
    pub normalized_live_state: Option<Value>,
    pub predicted_live_state: Option<Value>,
    pub modified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    #[serde(default)]
    pub available_actions: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoInspectionFailure {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoConnectedFallback {
    pub transport: String,
    pub failure: ArgoInspectionFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationInspector {
    pub application: ArgoApplicationRef,
    pub status: Option<Value>,
    pub history: Vec<ArgoApplicationHistory>,
    pub resources: Vec<ArgoManagedResource>,
    pub comparisons: Vec<ArgoResourceComparison>,
    pub conditions: Vec<Value>,
    pub operation_state: Option<Value>,
    pub connected: bool,
    pub transport: String,
    pub provenance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_fallback: Option<ArgoConnectedFallback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoOperationRequest {
    pub connection_id: Option<String>,
    pub transport: String,
    pub application: ArgoApplicationRef,
    pub action: String,
    pub revision: Option<String>,
    pub resources: Vec<ArgoManagedResource>,
    pub prune: Option<bool>,
    pub dry_run: Option<bool>,
    pub force: Option<bool>,
    pub history_id: Option<i64>,
    pub resource_action: Option<String>,
    pub resource_action_parameters: Option<Value>,
    pub resource_version: Option<String>,
    pub cluster_context: Option<String>,
    pub kubeconfig_env_var: Option<String>,
    pub sync_payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoOperationPreflight {
    pub allowed: bool,
    pub transport: String,
    pub action: String,
    pub reason: Option<String>,
    pub session_id: Option<String>,
    pub expires_at: Option<u64>,
    pub reviewed_request: Option<ArgoOperationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoOperationConfirmation {
    pub session_id: String,
    pub confirmation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArgoOperationResult {
    pub accepted: bool,
    pub transport: String,
    pub message: String,
    pub operation: Option<Value>,
}
