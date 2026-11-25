//! Finite Pod log read for callers that need one bounded snapshot instead of a stream.
//! It shares the stream's tail clamp, window validation and per-line byte cap, and keeps
//! the newest lines within a total byte budget (#634).

use super::{
    clamp_tail_lines,
    logs::{LOG_LINE_TRUNCATION_NOTICE, read_log_line},
    validate_pod_log_stream_request,
};
use crate::commands::ClusterAccess;
use crate::models::{AppError, AppErrorKind, PodLogStreamRequest};
use futures_util::io::AsyncBufRead;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, LogParams};
use kubecove_kube::commands::{FiniteRead, diagnostic_field};
use std::collections::VecDeque;

// kubectl's annotation for the container `kubectl logs` reads when none is named.
const DEFAULT_CONTAINER_ANNOTATION: &str = "kubectl.kubernetes.io/default-container";

/// Total line bytes one read returns, chosen by the product owner for #634. The newest
/// line is always kept, so a single line at the per-line cap still comes back.
pub const MAX_LOG_READ_BYTES: usize = 256 * 1024;

/// One container's log lines, oldest first.
pub struct PodLogRead {
    pub container: String,
    pub lines: Vec<String>,
    /// Total bytes of `lines`, without line breaks.
    pub bytes: usize,
    /// Lines were dropped or shortened to stay within `MAX_LOG_READ_BYTES`: older lines in
    /// the tail window, or the end of a single newest line larger than the budget.
    pub truncated: bool,
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
                let tail = read_newest_lines(&mut logs, MAX_LOG_READ_BYTES)
                    .await
                    .map_err(|error| AppError::new(error.to_string(), AppErrorKind::Cluster))?;
                Ok(PodLogRead {
                    container,
                    lines: tail.lines.into(),
                    bytes: tail.bytes,
                    truncated: tail.truncated,
                })
            },
            |read| {
                vec![
                    diagnostic_field("lines", read.lines.len()),
                    diagnostic_field("bytes", read.bytes),
                    diagnostic_field("truncated", read.truncated),
                ]
            },
        )
        .await
}

struct NewestLines {
    lines: VecDeque<String>,
    bytes: usize,
    truncated: bool,
}

/// Kubernetes `limitBytes` would keep the oldest bytes of the tail window and cut the
/// newest, which are the ones a troubleshooting read needs, so the budget is applied
/// here: memory stays within `budget` plus one line.
async fn read_newest_lines(
    reader: &mut (impl AsyncBufRead + Unpin),
    budget: usize,
) -> std::io::Result<NewestLines> {
    let mut tail = NewestLines {
        lines: VecDeque::new(),
        bytes: 0,
        truncated: false,
    };
    while let Some(line) = read_log_line(reader).await? {
        tail.bytes += line.len();
        tail.lines.push_back(line);
        while tail.bytes > budget && tail.lines.len() > 1 {
            let dropped = tail.lines.pop_front().expect("more than one line");
            tail.bytes -= dropped.len();
            tail.truncated = true;
        }
        if tail.bytes > budget
            && let Some(newest) = tail.lines.back_mut()
        {
            *newest = fit_line(newest, budget);
            tail.bytes = newest.len();
            tail.truncated = true;
        }
    }
    Ok(tail)
}

/// Shortens one line to `budget` bytes, ending it with the per-line truncation notice
/// when the notice fits. The read's `truncated` flag still reports the cut when it does
/// not.
fn fit_line(line: &str, budget: usize) -> String {
    let text = line
        .strip_suffix(LOG_LINE_TRUNCATION_NOTICE)
        .unwrap_or(line);
    let Some(text_budget) = budget.checked_sub(LOG_LINE_TRUNCATION_NOTICE.len()) else {
        return text[..text.floor_char_boundary(budget)].to_owned();
    };
    let kept = text.floor_char_boundary(text_budget);
    let mut fitted = String::with_capacity(kept + LOG_LINE_TRUNCATION_NOTICE.len());
    fitted.push_str(&text[..kept]);
    fitted.push_str(LOG_LINE_TRUNCATION_NOTICE);
    fitted
}

/// Like kubectl, the annotation may name a regular, init or ephemeral container; the
/// fallback is the first regular container.
fn default_container(pod: &Pod) -> Result<String, AppError> {
    let Some(spec) = pod.spec.as_ref() else {
        return Err(no_containers());
    };
    let named = |name: &String| {
        let name = name.as_str();
        spec.containers
            .iter()
            .any(|container| container.name == name)
            || spec
                .init_containers
                .iter()
                .flatten()
                .any(|container| container.name == name)
            || spec
                .ephemeral_containers
                .iter()
                .flatten()
                .any(|container| container.name == name)
    };
    pod.metadata
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(DEFAULT_CONTAINER_ANNOTATION))
        .filter(|name| named(name))
        .or_else(|| spec.containers.first().map(|container| &container.name))
        .cloned()
        .ok_or_else(no_containers)
}

fn no_containers() -> AppError {
    AppError::new("the Pod has no containers", AppErrorKind::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::io::{BufReader, Cursor};

    async fn newest(input: &str, budget: usize) -> NewestLines {
        read_newest_lines(&mut BufReader::new(Cursor::new(input.as_bytes())), budget)
            .await
            .unwrap()
    }

    // Failure modes: the budget keeps the oldest lines instead of the newest; an exact fit
    // reports truncation; line breaks count against the budget inconsistently; a single
    // line over the budget is lost, or kept whole past the budget, or cut inside a UTF-8
    // character.
    #[tokio::test]
    async fn keeps_newest_lines_within_budget() {
        let exact = newest("alpha\r\nbeta\n", 9).await;
        assert_eq!(Vec::from(exact.lines), ["alpha", "beta"]);
        assert_eq!(exact.bytes, 9);
        assert!(!exact.truncated);

        let over = newest("alpha\nbeta\ngamma\n", 9).await;
        assert_eq!(Vec::from(over.lines), ["beta", "gamma"]);
        assert_eq!(over.bytes, 9);
        assert!(over.truncated);
    }

    #[tokio::test]
    async fn shortens_a_newest_line_larger_than_budget() {
        // read_log_line keeps 256 KiB of this line and appends its notice, which alone
        // would pass the budget. Two-byte characters put a boundary at every odd offset.
        let input = format!("old\n{}\n", "é".repeat(MAX_LOG_READ_BYTES));
        let read = newest(&input, MAX_LOG_READ_BYTES).await;
        assert!(read.truncated);
        assert_eq!(read.lines.len(), 1);
        let line = &read.lines[0];
        assert!(line.starts_with('é'));
        assert!(line.ends_with(LOG_LINE_TRUNCATION_NOTICE));
        assert!(line.len() <= MAX_LOG_READ_BYTES);
        assert_eq!(read.bytes, line.len());

        // A budget smaller than the notice keeps text only, still within the budget.
        let small = newest("old\néééé\n", 5).await;
        assert!(small.truncated);
        assert_eq!(Vec::from(small.lines), ["éé"]);
        assert_eq!(small.bytes, 4);
    }
}
