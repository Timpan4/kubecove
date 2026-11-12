//! The detail-tab part of the browser-mock fixture: log streams, Deployment
//! revisions, action state with its dry-run previews and executions, and the
//! YAML apply review. Requests must equal the ones `export-detail-tabs.ts`
//! sent, so a wrong target, action, draft or missing guard reads as an error.

use kubecove_gpui::backend::{LogStream, LogTarget, Read};
use kubecove_lib::models::{
    AppError, AppErrorKind, ClusterOperationResult, ClusterOperationTarget, DeploymentRevision,
    KubernetesYamlLintResult, ResourceActionPreview, ResourceActionRequest, ResourceActionState,
    ResourceActionStateRequest, ResourceSummary, StreamMessage, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlEncoding, YamlViewMode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DetailTabs {
    #[serde(skip)]
    log_sender: std::sync::Mutex<
        Option<(
            tokio::sync::mpsc::UnboundedSender<StreamMessage>,
            StreamMessage,
        )>,
    >,
    #[serde(skip)]
    paused: std::sync::Mutex<bool>,
    logs: Logs,
    #[serde(rename = "yamlReads")]
    yaml_reads: Vec<YamlRead>,
    revisions: Vec<DeploymentRevision>,
    actions: Actions,
    #[serde(rename = "yamlApply")]
    yaml_apply: Vec<YamlApply>,
    /// Lint results for the drafts the scenarios edit. Fixtures exported before
    /// the lint existed have none, so every lint read reports it missing.
    #[serde(rename = "yamlLint", default)]
    yaml_lint: Vec<YamlLint>,
}

#[derive(Deserialize)]
struct YamlLint {
    request: YamlApplyRequest,
    result: KubernetesYamlLintResult,
}

#[derive(Deserialize)]
struct YamlRead {
    target: ClusterOperationTarget,
    mode: YamlViewMode,
    encoding: YamlEncoding,
    yaml: String,
}

#[derive(Deserialize)]
struct Logs {
    pod: Vec<StreamMessage>,
    aggregate: Vec<StreamMessage>,
    #[serde(default)]
    cases: Vec<LogCase>,
}

#[derive(Deserialize)]
struct LogCase {
    script: String,
    request: serde_json::Value,
    messages: Vec<StreamMessage>,
    next: Option<StreamMessage>,
}

#[derive(Deserialize)]
struct Actions {
    states: Vec<ResourceActionState>,
    #[serde(rename = "pausedState")]
    paused_state: Option<ResourceActionState>,
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
    request: YamlApplyRequest,
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
    // Other fixture harnesses do not inject a subsequent log message.
    #[allow(dead_code)]
    pub fn emit_log(&self) {
        let sender = self.log_sender.lock().unwrap();
        let (sender, message) = sender.as_ref().expect("active controlled stream");
        sender
            .send(message.clone())
            .expect("controlled log receiver");
    }
    pub fn resource_yaml(
        &self,
        resource: ResourceSummary,
        mode: YamlViewMode,
        encoding: YamlEncoding,
    ) -> Read<String> {
        let target = ClusterOperationTarget {
            cluster_context: resource.cluster,
            kind: resource.kind,
            name: resource.name,
            namespace: resource.namespace,
            api_version: resource.api_version,
        };
        eprintln!(
            "Fixture YAML {} mode={mode:?} encoding={encoding:?}",
            target.name
        );
        let found = self.yaml_reads.iter().find(|e| {
            serde_json::to_value(&e.target).unwrap() == serde_json::to_value(&target).unwrap()
                && e.mode == mode
                && e.encoding == encoding
        });
        ready(found.map_or_else(
            || {
                eprintln!(
                    "No fixture YAML target={}",
                    serde_json::to_string(&target).unwrap()
                );
                missing("YAML shape and encoding")
            },
            |e| Ok(e.yaml.clone()),
        ))
    }

    pub fn log_stream(&self, target: LogTarget, script: &str) -> Read<LogStream> {
        if script.starts_with("logs-") {
            let mut request = match target {
                LogTarget::Pod(request) => serde_json::to_value(request).unwrap(),
                LogTarget::Aggregate(request) => serde_json::to_value(request).unwrap(),
            };
            request
                .as_object_mut()
                .unwrap()
                .retain(|key, value| key != "kubeconfigEnvVar" && !value.is_null());
            return ready(
                self.logs
                    .cases
                    .iter()
                    .find(|entry| entry.script == script && entry.request == request)
                    .map(|entry| {
                        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
                        for message in &entry.messages {
                            sender.send(message.clone()).unwrap();
                        }
                        if let Some(next) = &entry.next {
                            *self.log_sender.lock().unwrap() = Some((sender.clone(), next.clone()));
                        }
                        LogStream::new(receiver, sender)
                    })
                    .ok_or_else(|| {
                        AppError::new("No exact log fixture request", AppErrorKind::NotFound)
                    }),
            );
        }
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
        if *self.paused.lock().unwrap()
            && let Some(state) = &self.actions.paused_state
            && state.target.cluster_context == target.cluster_context
            && state.target.kind == target.kind
            && state.target.name == target.name
            && state.target.namespace == target.namespace
        {
            return ready(Ok(state.clone()));
        }
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
        let action = request.action;
        let request = comparable(&request);
        let found = self
            .actions
            .executions
            .iter()
            .find(|entry| comparable(&entry.request) == request);
        if found.is_some() {
            *self.paused.lock().unwrap() =
                action == kubecove_lib::models::ResourceActionKind::Pause;
        }
        ready(found.map_or_else(
            || missing("action execution"),
            |entry| Ok(entry.result.clone()),
        ))
    }

    fn yaml_entry(&self, request: &YamlApplyRequest) -> Option<&YamlApply> {
        self.yaml_apply.iter().find(|entry| {
            let mut actual = request.clone();
            actual.kubeconfig_env_var = None;
            actual.expected_uid = None;
            actual.expected_resource_version = None;
            serde_json::to_value(&entry.request).unwrap() == serde_json::to_value(actual).unwrap()
        })
    }

    pub fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        let found = self.yaml_entry(&request).map(|entry| entry.preview.clone());
        ready(found.map_or_else(|| missing("YAML dry run"), Ok))
    }

    /// The exported lint for this draft, matched like the dry run. The timing
    /// gate is applied by the caller, as for every other mock read.
    pub fn lint_yaml(
        &self,
        request: &YamlApplyRequest,
    ) -> Result<KubernetesYamlLintResult, AppError> {
        let mut actual = request.clone();
        actual.kubeconfig_env_var = None;
        let actual = serde_json::to_value(actual).unwrap();
        let found = self
            .yaml_lint
            .iter()
            .find(|entry| serde_json::to_value(&entry.request).unwrap() == actual);
        found.map_or_else(|| missing("YAML lint"), |entry| Ok(entry.result.clone()))
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
