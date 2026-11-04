//! The detail-tab part of the browser-mock fixture: log streams, Deployment
//! revisions, action state with its dry-run previews and executions, and the
//! YAML apply review. Requests must equal the ones `export-detail-tabs.ts`
//! sent, so a wrong target, action, draft or missing guard reads as an error.

use kubecove_gpui::backend::{LogStream, LogTarget, Read};
use kubecove_lib::models::{
    AppError, AppErrorKind, ClusterOperationResult, DeploymentRevision, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceSummary,
    StreamMessage, YamlApplyPreview, YamlApplyRequest, YamlApplyResult,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DetailTabs {
    logs: Logs,
    revisions: Vec<DeploymentRevision>,
    actions: Actions,
    #[serde(rename = "yamlApply")]
    yaml_apply: Vec<YamlApply>,
}

#[derive(Deserialize)]
struct Logs {
    pod: Vec<StreamMessage>,
    aggregate: Vec<StreamMessage>,
}

#[derive(Deserialize)]
struct Actions {
    states: Vec<ResourceActionState>,
    previews: Vec<Previewed>,
    executions: Vec<Executed>,
}

#[derive(Deserialize)]
struct Previewed {
    request: ResourceActionRequest,
    preview: ResourceActionPreview,
}

#[derive(Deserialize)]
struct Executed {
    request: ResourceActionRequest,
    result: ClusterOperationResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YamlApply {
    kind: String,
    name: String,
    namespace: Option<String>,
    draft: String,
    preview: YamlApplyPreview,
    apply_error: String,
}

fn ready<T: Send + 'static>(result: Result<T, AppError>) -> Read<T> {
    Box::pin(async move { result })
}

fn missing<T>(what: &str) -> Result<T, AppError> {
    Err(AppError::new(
        format!("No fixture {what} for this request"),
        AppErrorKind::NotFound,
    ))
}

/// The request as JSON without the kubeconfig source, which the clients name differently.
fn comparable(request: &ResourceActionRequest) -> serde_json::Value {
    let mut request = request.clone();
    request.kubeconfig_env_var = None;
    serde_json::to_value(request).expect("requests serialize")
}

impl DetailTabs {
    pub fn log_stream(&self, target: LogTarget) -> Read<LogStream> {
        let messages = match target {
            LogTarget::Pod(_) => self.logs.pod.clone(),
            LogTarget::Aggregate(_) => self.logs.aggregate.clone(),
        };
        ready(Ok(LogStream::replay(messages)))
    }

    pub fn deployment_revisions(&self, resource: ResourceSummary) -> Read<Vec<DeploymentRevision>> {
        let result = if resource.kind == "Deployment" {
            Ok(self.revisions.clone())
        } else {
            missing("revisions")
        };
        ready(result)
    }

    pub fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        let target = request.target;
        let found = self.actions.states.iter().find(|state| {
            state.target.cluster_context == target.cluster_context
                && state.target.kind == target.kind
                && state.target.name == target.name
                && state.target.namespace == target.namespace
        });
        ready(found.cloned().map_or_else(|| missing("action state"), Ok))
    }

    pub fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        let request = comparable(&request);
        let found = self
            .actions
            .previews
            .iter()
            .find(|entry| comparable(&entry.request) == request);
        ready(found.map_or_else(
            || missing("action preview"),
            |entry| Ok(entry.preview.clone()),
        ))
    }

    pub fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        let request = comparable(&request);
        let found = self
            .actions
            .executions
            .iter()
            .find(|entry| comparable(&entry.request) == request);
        ready(found.map_or_else(
            || missing("action execution"),
            |entry| Ok(entry.result.clone()),
        ))
    }

    fn yaml_entry(&self, request: &YamlApplyRequest) -> Option<&YamlApply> {
        self.yaml_apply.iter().find(|entry| {
            entry.kind == request.kind
                && entry.name == request.name
                && entry.namespace == request.namespace
                && entry.draft == request.yaml
        })
    }

    pub fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        let found = self.yaml_entry(&request).map(|entry| entry.preview.clone());
        ready(found.map_or_else(|| missing("YAML dry run"), Ok))
    }

    /// The browser mocks refuse every apply; the fixture reports the same error.
    pub fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        let result = match self.yaml_entry(&request) {
            Some(entry) => Err(AppError::new(
                entry.apply_error.clone(),
                AppErrorKind::UnsupportedOperation,
            )),
            None => missing("YAML apply"),
        };
        ready(result)
    }
}
