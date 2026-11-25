//! Tool arguments. Doc comments become the JSON Schema descriptions agents read.

use kubecove_models::{RbacAccessReviewIdentity, RbacAccessReviewTarget};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

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
    /// Kubernetes label selector, such as `app=web,tier!=cache`.
    pub label_selector: Option<String>,
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
    /// Container name. Omit to read the container `kubectl logs` would: the
    /// `kubectl.kubernetes.io/default-container` annotation, else the first container.
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
pub struct GitOpsListArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Namespace the Application or release lives in. Omit for all namespaces.
    pub namespace: Option<String>,
    /// Maximum rows to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TopologyMode {
    /// Owner references and Argo CD, Flux or Helm ownership, such as an Application, a
    /// Deployment and its Pods.
    #[default]
    Ownership,
    /// Traffic: Ingresses to Services to Pods.
    NetworkFlow,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopologyArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Namespace to map.
    pub namespace: String,
    #[serde(default)]
    pub mode: TopologyMode,
    /// With `focusName`, return only the resources connected to this resource. The kind
    /// name as Kubernetes writes it, such as `Ingress` or `Deployment`; plurals and short
    /// names are not matched.
    pub focus_kind: Option<String>,
    /// With `focusKind`, the name of the resource to trace from.
    pub focus_name: Option<String>,
    /// Maximum nodes to return. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Nodes to skip, from a previous result's `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RbacReviewArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Who to check: `{"kind":"serviceAccount","name":"web","namespace":"shop"}`,
    /// `{"kind":"user","username":"alice","groups":["dev"]}` or
    /// `{"kind":"group","group":"dev"}`.
    #[schemars(with = "Value")]
    pub identity: RbacAccessReviewIdentity,
    /// What to check: `{"kind":"resource","verb":"list","resource":"pods","apiGroup":"",
    /// "namespace":"shop"}`, optionally with `subresource` and `name`, or
    /// `{"kind":"nonResource","verb":"get","nonResourceUrl":"/healthz"}`.
    #[schemars(with = "Value")]
    pub target: RbacAccessReviewTarget,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeploymentRevisionsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Deployment namespace.
    pub namespace: String,
    /// Deployment name.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelmReleaseArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Release namespace.
    pub namespace: String,
    /// Release name.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetricsArgs {
    /// Kubeconfig context name. Omit to use the current context.
    pub context: Option<String>,
    /// Namespace for Pod and workload usage. Omit for all namespaces.
    pub namespace: Option<String>,
    /// Return only this list. Required with `offset`, because each list pages separately.
    pub list: Option<MetricsList>,
    /// Maximum rows to return per list. Defaults to 500.
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Rows to skip in `list`, from that list's previous `nextOffset`.
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MetricsList {
    Pods,
    Workloads,
    Nodes,
}
