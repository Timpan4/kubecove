//! Tool arguments. Doc comments become the JSON Schema descriptions agents read.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoArgs {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListResourceKindsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Case-insensitive text matched against kind, plural, short names and API group.
    pub query: Option<String>,
    /// Maximum rows to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HealthFilter {
    /// Every resource.
    #[default]
    All,
    /// Only resources assessed as degraded, needing attention or unknown.
    Unhealthy,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListResourcesArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Kind, plural or short name, such as `Pod`, `deployments`, `svc` or a custom
    /// resource kind like `Application`.
    pub kind: String,
    /// API version, such as `argoproj.io/v1alpha1`, when a kind name exists in several groups.
    pub api_version: Option<String>,
    /// Namespace to list. Omit for all namespaces. Ignored for cluster-scoped kinds.
    pub namespace: Option<String>,
    /// Case-insensitive substring of the resource name.
    pub name_contains: Option<String>,
    #[serde(default)]
    pub health: HealthFilter,
    /// Maximum rows to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Kind, plural or short name, such as `Pod`, `deploy` or `Application`.
    pub kind: String,
    /// API version, when a kind name exists in several groups.
    pub api_version: Option<String>,
    /// Resource name.
    pub name: String,
    /// Namespace. Required for namespaced kinds.
    pub namespace: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindProblemsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Namespace to scan. Omit for all namespaces. Nodes are always included.
    pub namespace: Option<String>,
    /// Also return resolved and historical signals, such as restarts that have stopped.
    #[serde(default)]
    pub include_resolved: bool,
    /// Maximum rows to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PodLogsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Pod namespace.
    pub namespace: String,
    /// Pod name.
    pub pod: String,
    /// Container name. Required when the Pod has more than one container.
    pub container: Option<String>,
    /// Lines from the end of the log. Defaults to 200; at most 1000.
    #[schemars(range(min = 0))]
    pub tail_lines: Option<i64>,
    /// Only lines newer than this many seconds.
    #[schemars(range(min = 0))]
    pub since_seconds: Option<i64>,
    /// Read the previous, terminated container instance, such as after a crash.
    #[serde(default)]
    pub previous: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Namespace for Pod and workload usage. Omit for all namespaces.
    pub namespace: Option<String>,
    /// Maximum rows to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}
