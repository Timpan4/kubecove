//! Finite Pod log read for callers that need one bounded snapshot instead of a stream.
//! It shares the stream's tail clamp, window validation and per-line byte cap.

use super::{clamp_tail_lines, logs::read_log_line, validate_pod_log_stream_request};
use crate::commands::ClusterAccess;
use crate::models::{AppError, AppErrorKind, PodLogStreamRequest};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, LogParams};
use kubecove_kube::commands::{FiniteRead, diagnostic_field};

// kubectl's annotation for the container `kubectl logs` reads when none is named.
const DEFAULT_CONTAINER_ANNOTATION: &str = "kubectl.kubernetes.io/default-container";

/// One container's log lines, oldest first.
pub struct PodLogRead {
    pub container: String,
    pub lines: Vec<String>,
}

/// Reads the last lines of one container's log, oldest first, with timestamps.
/// `previous` reads the last terminated container instance. Without a container it
/// reads the one `kubectl logs` would: the annotated default, else the first.
pub async fn read_pod_logs(
    request: PodLogStreamRequest,
    previous: bool,
) -> Result<PodLogRead, AppError> {
    validate_pod_log_stream_request(&request)?;
    FiniteRead::new("read_pod_logs")
        .label("context", &request.cluster_context)
        .label("namespace", &request.namespace)
        .label("pod", &request.pod_name)
        .run(
            async move {
                let access = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?;
                let client = access.read_client(&request.cluster_context).await?;
                let pods: Api<Pod> = Api::namespaced(client, &request.namespace);
                let container = match request.container {
                    Some(container) => container,
                    None => default_container(
                        &pods.get(&request.pod_name).await.map_err(AppError::from)?,
                    )?,
                };
                let params = LogParams {
                    container: Some(container.clone()),
                    follow: false,
                    tail_lines: Some(clamp_tail_lines(request.tail_lines)),
                    since_seconds: request.since_seconds,
                    timestamps: true,
                    previous,
                    ..LogParams::default()
                };
                let logs = pods
                    .log_stream(&request.pod_name, &params)
                    .await
                    .map_err(AppError::from)?;
                futures_util::pin_mut!(logs);
                let mut lines = Vec::new();
                while let Some(line) = read_log_line(&mut logs)
                    .await
                    .map_err(|error| AppError::new(error.to_string(), AppErrorKind::Cluster))?
                {
                    lines.push(line);
                }
                Ok(PodLogRead { container, lines })
            },
            |read| vec![diagnostic_field("lines", read.lines.len())],
        )
        .await
}

fn default_container(pod: &Pod) -> Result<String, AppError> {
    let containers = pod
        .spec
        .as_ref()
        .map(|spec| spec.containers.as_slice())
        .unwrap_or_default();
    let annotated = pod
        .metadata
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(DEFAULT_CONTAINER_ANNOTATION))
        .filter(|name| containers.iter().any(|container| &container.name == *name));
    annotated
        .or_else(|| containers.first().map(|container| &container.name))
        .cloned()
        .ok_or_else(|| AppError::new("the Pod has no containers", AppErrorKind::Validation))
}
