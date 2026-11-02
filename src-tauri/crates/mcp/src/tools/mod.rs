//! The read-only tool set (ADR 0028). Each tool names its arguments type, which supplies
//! its JSON Schema, and runs through `FiniteRead` so desktop diagnostics record it.

mod args;
mod cluster;
mod logs_gitops;
mod problems;
mod resources;

use crate::handler::McpContext;
use kubecove_kube::commands::{ClusterAccess, FiniteRead};
use kubecove_models::{AppError, AppErrorKind};
use rmcp::{
    handler::server::common::schema_for_type,
    model::{Tool as ToolDefinition, ToolAnnotations},
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

macro_rules! tools {
    ($($variant:ident => $name:literal, $args:ty, $title:literal, $description:literal;)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Tool { $($variant),+ }

        /// Every tool name, in listing order.
        pub const TOOL_NAMES: &[&str] = &[$($name),+];

        impl Tool {
            pub fn named(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// `mcp.<name>`, for diagnostics and server observations.
            pub fn diagnostic_name(self) -> &'static str {
                match self { $(Self::$variant => concat!("mcp.", $name)),+ }
            }
        }

        pub fn definitions() -> Vec<ToolDefinition> {
            vec![$(definition::<$args>($name, $title, $description)),+]
        }
    };
}

tools! {
    ListContexts => "list_contexts", args::NoArgs, "List contexts",
        "Kubeconfig contexts KubeCove can read, and which one is current.";
    ListNamespaces => "list_namespaces", args::ContextArgs, "List namespaces",
        "Namespaces in a context, with their age.";
    ListResourceKinds => "list_resource_kinds", args::ListResourceKindsArgs, "List resource kinds",
        "Built-in kinds KubeCove lists plus the cluster's custom resource kinds, with API \
         version, plural, short names and scope.";
    ListResources => "list_resources", args::ListResourcesArgs, "List resources",
        "Compact rows for one kind: status, readiness, restarts, age, assessed health with \
         reasons, owner and GitOps ownership. Use health=unhealthy to see only problems.";
    FindProblems => "find_problems", args::FindProblemsArgs, "Find problems",
        "Scans Pods, workloads, Jobs, PersistentVolumeClaims and Nodes for degraded health, \
         restarts and recent Warning events, most severe first. Start troubleshooting here.";
    DescribeResource => "describe_resource", args::ResourceArgs, "Describe resource",
        "One resource's summary, labels, annotations, owners, top-level fields such as spec \
         or data, status and recent events. Secret values are redacted.";
    GetResourceYaml => "get_resource_yaml", args::ResourceArgs, "Get resource YAML",
        "One resource as YAML without managedFields. Secret values are redacted.";
    GetPodLogs => "get_pod_logs", args::PodLogsArgs, "Get Pod logs",
        "The last lines of one Pod container's log, with timestamps. Without container it \
         reads the container kubectl logs would and reports its name. Set previous=true for \
         the crashed instance of a restarting container.";
    GetResourceMetrics => "get_resource_metrics", args::MetricsArgs, "Get resource usage",
        "Current CPU and memory usage of Pods, workloads and Nodes from the metrics API, \
         highest CPU first.";
    ListArgocdApplications => "list_argocd_applications", args::ContextArgs,
        "List Argo CD Applications",
        "Argo CD Applications read from the cluster, with sync status, health, source and \
         destination.";
    ListHelmReleases => "list_helm_releases", args::ContextArgs, "List Helm releases",
        "Helm releases stored in the cluster, with chart, app version, revision and status.";
}

fn definition<A: JsonSchema + 'static>(
    name: &'static str,
    title: &'static str,
    description: &'static str,
) -> ToolDefinition {
    let mut tool = ToolDefinition::new(name, description, schema_for_type::<A>());
    tool.title = Some(title.to_owned());
    tool.annotations = Some(
        ToolAnnotations::with_title(title)
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(true),
    );
    tool
}

pub async fn run(context: &McpContext, tool: Tool, arguments: Value) -> Result<Value, AppError> {
    FiniteRead::new(tool.diagnostic_name())
        .run(
            Box::pin(async move {
                match tool {
                    Tool::ListContexts => {
                        parse::<args::NoArgs>(arguments)?;
                        cluster::list_contexts()
                    }
                    Tool::ListNamespaces => {
                        cluster::list_namespaces(context, parse(arguments)?).await
                    }
                    Tool::ListResourceKinds => {
                        cluster::list_resource_kinds(context, parse(arguments)?).await
                    }
                    Tool::ListResources => {
                        resources::list_resources(context, parse(arguments)?).await
                    }
                    Tool::FindProblems => problems::find_problems(context, parse(arguments)?).await,
                    Tool::DescribeResource => {
                        resources::describe_resource(context, parse(arguments)?).await
                    }
                    Tool::GetResourceYaml => {
                        resources::get_resource_yaml(context, parse(arguments)?).await
                    }
                    Tool::GetPodLogs => logs_gitops::get_pod_logs(parse(arguments)?).await,
                    Tool::GetResourceMetrics => {
                        cluster::get_resource_metrics(parse(arguments)?).await
                    }
                    Tool::ListArgocdApplications => {
                        logs_gitops::list_argocd_applications(parse(arguments)?).await
                    }
                    Tool::ListHelmReleases => {
                        logs_gitops::list_helm_releases(parse(arguments)?).await
                    }
                }
            }),
            |_| Vec::new(),
        )
        .await
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, AppError> {
    serde_json::from_value(arguments).map_err(|error| {
        AppError::new(
            format!("invalid arguments: {error}"),
            AppErrorKind::Validation,
        )
    })
}

/// The context to read and the access for it. Every tool reads the host's configured
/// kubeconfig source; agents cannot select another one.
pub(crate) fn access_for(context: Option<String>) -> Result<(ClusterAccess, String), AppError> {
    let access = ClusterAccess::resolve(None)?;
    let context = match context.filter(|context| !context.trim().is_empty()) {
        Some(context) => context,
        None => access
            .contexts()?
            .into_iter()
            .find(|context| context.is_current)
            .map(|context| context.name)
            .ok_or_else(|| {
                AppError::new(
                    "the kubeconfig has no current context; pass context",
                    AppErrorKind::Validation,
                )
            })?,
    };
    Ok((access, context))
}
