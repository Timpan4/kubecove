use crate::models::{
    AppError, PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary,
    PodExecTerminalSize,
};
use crate::runtime::{Channel, State};

mod registry;
mod runner;
mod validation;

#[cfg(feature = "bench-support")]
pub fn benchmark_pod_exec_session_summary() -> PodExecSessionSummary {
    let request = validation::ValidatedPodExecRequest {
        cluster_context: "benchmark".to_string(),
        kubeconfig_env_var: None,
        kubeconfig_source_key: Some("fixture-source".to_string()),
        kubeconfig_source_label: Some("Benchmark source".to_string()),
        namespace: "payments".to_string(),
        pod_name: "checkout-0".to_string(),
        container: Some("app".to_string()),
        command: vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf benchmark".to_string(),
        ],
        stdin: true,
        tty: true,
        terminal_size: PodExecTerminalSize {
            cols: 120,
            rows: 40,
        },
    };
    let mut summary = registry::session_summary("exec-session-1".to_string(), &request);
    summary.started_at = "2026-10-11T00:00:00Z".to_string();
    summary
}

#[cfg(feature = "bench-support")]
pub use runner::{benchmark_decode_exec_chunks, EXEC_OUTPUT_CHUNK_BYTES};

use registry::ExecCommand;
pub use registry::PodExecRegistry;
use runner::start_pod_exec_session_in_registry;
#[cfg(test)]
use validation::validate_request;
use validation::validate_terminal_size;

pub async fn start_pod_exec_session(
    request: PodExecSessionRequest,
    channel: Channel<PodExecSessionMessage>,
    registry: State<'_, PodExecRegistry>,
) -> Result<PodExecSessionSummary, AppError> {
    start_pod_exec_session_in_registry(request, channel, registry.inner()).await
}

pub async fn write_pod_exec_stdin(
    session_id: String,
    data: String,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    registry.write_stdin(&session_id, data).await?;
    Ok(true)
}

pub async fn resize_pod_exec_terminal(
    session_id: String,
    size: PodExecTerminalSize,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    validate_terminal_size(&size)?;
    registry.send_command(&session_id, ExecCommand::Resize(size))?;
    Ok(true)
}

pub async fn stop_pod_exec_session(
    session_id: String,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    Ok(registry.stop(&session_id))
}

pub async fn list_pod_exec_sessions(
    registry: State<'_, PodExecRegistry>,
) -> Result<Vec<PodExecSessionSummary>, AppError> {
    Ok(registry.list())
}

#[cfg(test)]
mod tests;
