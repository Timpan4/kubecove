use crate::models::{
    AppError, PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary,
    PodExecTerminalSize,
};
use chrono::Utc;
use futures_util::SinkExt;
use k8s_openapi::{
    api::core::v1::Pod,
    apimachinery::pkg::apis::meta::v1::Status,
};
use kube::{
    api::{Api, AttachParams, TerminalSize},
    config::KubeConfigOptions,
    Client,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{async_runtime::JoinHandle, ipc::Channel, State};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot},
};

const MIN_TERMINAL_SIZE: u16 = 1;
const MAX_TERMINAL_SIZE: u16 = 500;

#[derive(Debug, Clone)]
struct ValidatedPodExecRequest {
    cluster_context: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    command: Vec<String>,
    stdin: bool,
    tty: bool,
    terminal_size: PodExecTerminalSize,
}

enum ExecCommand {
    Stdin(Vec<u8>),
    Resize(PodExecTerminalSize),
}

struct PodExecSession {
    summary: PodExecSessionSummary,
    commands: mpsc::UnboundedSender<ExecCommand>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct PodExecState {
    sessions: BTreeMap<String, PodExecSession>,
}

#[derive(Clone, Default)]
pub struct PodExecRegistry {
    next_id: Arc<AtomicU64>,
    state: Arc<Mutex<PodExecState>>,
}

impl PodExecRegistry {
    fn session_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("pod-exec-{id}")
    }

    fn insert(
        &self,
        summary: PodExecSessionSummary,
        commands: mpsc::UnboundedSender<ExecCommand>,
        handle: JoinHandle<()>,
    ) -> PodExecSessionSummary {
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .insert(
                summary.id.clone(),
                PodExecSession {
                    summary: summary.clone(),
                    commands,
                    handle: Some(handle),
                },
            );
        summary
    }

    fn list(&self) -> Vec<PodExecSessionSummary> {
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .values()
            .map(|session| session.summary.clone())
            .collect()
    }

    fn mark_status(&self, session_id: &str, status: &str, last_error: Option<String>) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.status = status.to_string();
            session.summary.last_error = last_error;
        }
    }

    fn mark_running(&self, session_id: &str) {
        self.mark_status(session_id, "running", None);
    }

    fn mark_error(&self, session_id: &str, message: String) {
        self.mark_status(session_id, "error", Some(message));
    }

    fn mark_terminal_size(&self, session_id: &str, size: PodExecTerminalSize) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.terminal_cols = size.cols;
            session.summary.terminal_rows = size.rows;
        }
    }

    fn mark_exited(&self, session_id: &str, exit_code: Option<i32>) {
        if let Some(session) = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get_mut(session_id)
        {
            session.summary.status = "exited".to_string();
            session.summary.finished_at = Some(Utc::now().to_rfc3339());
            session.summary.exit_code = exit_code;
        }
    }

    fn send_command(&self, session_id: &str, command: ExecCommand) -> Result<(), AppError> {
        let commands = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .get(session_id)
            .map(|session| session.commands.clone())
            .ok_or_else(|| AppError::new("exec session was not found", "session"))?;
        commands
            .send(command)
            .map_err(|_| AppError::new("exec session is no longer accepting input", "session"))
    }

    fn stop(&self, session_id: &str) -> bool {
        let session = self
            .state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .remove(session_id);
        let Some(mut session) = session else {
            return false;
        };
        if let Some(handle) = session.handle.take() {
            handle.abort();
        }
        true
    }

    #[cfg(test)]
    fn insert_summary_for_test(&self, summary: PodExecSessionSummary) {
        let (commands, _rx) = mpsc::unbounded_channel();
        self.state
            .lock()
            .expect("pod exec registry lock")
            .sessions
            .insert(
                summary.id.clone(),
                PodExecSession {
                    summary,
                    commands,
                    handle: None,
                },
            );
    }
}

fn trimmed(value: &str) -> String {
    value.trim().to_string()
}

fn validate_terminal_size(size: &PodExecTerminalSize) -> Result<(), AppError> {
    if !(MIN_TERMINAL_SIZE..=MAX_TERMINAL_SIZE).contains(&size.cols)
        || !(MIN_TERMINAL_SIZE..=MAX_TERMINAL_SIZE).contains(&size.rows)
    {
        return Err(AppError::new(
            "terminal size must be between 1 and 500 columns and rows",
            "validation",
        ));
    }
    Ok(())
}

fn exec_target_text(cluster_context: &str, namespace: &str, pod_name: &str) -> String {
    format!("{cluster_context}/{namespace}/Pod/{pod_name}")
}

fn exec_command_text(command: &[String]) -> String {
    command.join(" ")
}

fn validate_request(request: &PodExecSessionRequest) -> Result<ValidatedPodExecRequest, AppError> {
    let cluster_context = trimmed(&request.cluster_context);
    let namespace = trimmed(&request.namespace);
    let pod_name = trimmed(&request.pod_name);
    if cluster_context.is_empty() || namespace.is_empty() || pod_name.is_empty() {
        return Err(AppError::new("pod exec target is required", "validation"));
    }

    let command = request
        .command
        .iter()
        .map(|part| trimmed(part))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if command.is_empty() {
        return Err(AppError::new("pod exec command is required", "validation"));
    }

    validate_terminal_size(&request.terminal_size)?;
    if !request.stdin && request.tty {
        return Err(AppError::new(
            "tty exec sessions require stdin",
            "validation",
        ));
    }
    if !request.confirmation.acknowledged {
        return Err(AppError::new(
            "pod exec requires explicit confirmation",
            "validation",
        ));
    }
    if request.confirmation.target != exec_target_text(&cluster_context, &namespace, &pod_name) {
        return Err(AppError::new(
            "pod exec confirmation target does not match the request",
            "validation",
        ));
    }
    if request.confirmation.command != exec_command_text(&command) {
        return Err(AppError::new(
            "pod exec confirmation command does not match the request",
            "validation",
        ));
    }

    Ok(ValidatedPodExecRequest {
        cluster_context,
        namespace,
        pod_name,
        container: request
            .container
            .as_ref()
            .map(|container| trimmed(container))
            .filter(|container| !container.is_empty()),
        command,
        stdin: request.stdin,
        tty: request.tty,
        terminal_size: request.terminal_size.clone(),
    })
}

async fn client_for_context(cluster_context: &str) -> Result<Client, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.to_string()),
        ..Default::default()
    };
    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))
}

fn send(channel: &Channel<PodExecSessionMessage>, message: PodExecSessionMessage) -> bool {
    channel.send(message).is_ok()
}

fn exit_code_from_status(status: &Status) -> Option<i32> {
    status.details.as_ref().and_then(|details| {
        details.causes.as_ref().and_then(|causes| {
            causes.iter().find_map(|cause| {
                if cause.reason.as_deref() == Some("ExitCode") {
                    cause.message.as_ref()?.parse::<i32>().ok()
                } else {
                    None
                }
            })
        })
    })
}

async fn read_exec_output(
    session_id: String,
    stream: &'static str,
    mut reader: impl AsyncRead + Unpin,
    channel: Channel<PodExecSessionMessage>,
) {
    let mut buffer = [0_u8; 4096];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(size) => {
                let data = String::from_utf8_lossy(&buffer[..size]).to_string();
                if !send(
                    &channel,
                    PodExecSessionMessage::Output {
                        session_id: session_id.clone(),
                        stream: stream.to_string(),
                        data,
                    },
                ) {
                    break;
                }
            }
            Err(err) => {
                send(
                    &channel,
                    PodExecSessionMessage::Error {
                        session_id: session_id.clone(),
                        message: err.to_string(),
                    },
                );
                break;
            }
        }
    }
}

async fn run_exec_session(
    summary: PodExecSessionSummary,
    request: ValidatedPodExecRequest,
    mut commands: mpsc::UnboundedReceiver<ExecCommand>,
    channel: Channel<PodExecSessionMessage>,
    registry: PodExecRegistry,
) {
    let session_id = summary.id.clone();
    send(
        &channel,
        PodExecSessionMessage::Status {
            session_id: session_id.clone(),
            status: "connecting".to_string(),
            message: "Opening Kubernetes exec session".to_string(),
        },
    );

    let client = match client_for_context(&request.cluster_context).await {
        Ok(client) => client,
        Err(err) => {
            registry.mark_error(&session_id, err.message.clone());
            send(
                &channel,
                PodExecSessionMessage::Error {
                    session_id,
                    message: err.message,
                },
            );
            return;
        }
    };
    let pods: Api<Pod> = Api::namespaced(client, &request.namespace);
    let mut params = AttachParams::default()
        .stdin(request.stdin)
        .stdout(true)
        .stderr(!request.tty)
        .tty(request.tty)
        .max_stdin_buf_size(16 * 1024)
        .max_stdout_buf_size(64 * 1024)
        .max_stderr_buf_size(64 * 1024);
    if let Some(container) = &request.container {
        params = params.container(container.clone());
    }

    let mut attached = match pods.exec(&request.pod_name, request.command.clone(), &params).await {
        Ok(attached) => attached,
        Err(err) => {
            let message = err.to_string();
            registry.mark_error(&session_id, message.clone());
            send(
                &channel,
                PodExecSessionMessage::Error {
                    session_id,
                    message,
                },
            );
            return;
        }
    };

    registry.mark_running(&session_id);
    send(
        &channel,
        PodExecSessionMessage::Status {
            session_id: session_id.clone(),
            status: "running".to_string(),
            message: "Exec session is running".to_string(),
        },
    );

    let mut stdin = attached.stdin();
    let mut terminal_size = attached.terminal_size();
    if let Some(sender) = terminal_size.as_mut() {
        let _ = sender
            .send(TerminalSize {
                width: request.terminal_size.cols,
                height: request.terminal_size.rows,
            })
            .await;
    }

    let mut readers = Vec::new();
    if let Some(stdout) = attached.stdout() {
        readers.push(tauri::async_runtime::spawn(read_exec_output(
            session_id.clone(),
            if request.tty { "terminal" } else { "stdout" },
            stdout,
            channel.clone(),
        )));
    }
    if let Some(stderr) = attached.stderr() {
        readers.push(tauri::async_runtime::spawn(read_exec_output(
            session_id.clone(),
            "stderr",
            stderr,
            channel.clone(),
        )));
    }
    let (status_tx, mut status_rx) = oneshot::channel();
    if let Some(status) = attached.take_status() {
        tauri::async_runtime::spawn(async move {
            let _ = status_tx.send(status.await);
        });
    } else {
        drop(status_tx);
    }

    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
                match command {
                    ExecCommand::Stdin(data) => {
                        if let Some(writer) = stdin.as_mut() {
                            if let Err(err) = writer.write_all(&data).await {
                                registry.mark_error(&session_id, err.to_string());
                                send(
                                    &channel,
                                    PodExecSessionMessage::Error {
                                        session_id: session_id.clone(),
                                        message: err.to_string(),
                                    },
                                );
                                break;
                            }
                        }
                    }
                    ExecCommand::Resize(size) => {
                        if let Some(sender) = terminal_size.as_mut() {
                            if sender
                                .send(TerminalSize {
                                    width: size.cols,
                                    height: size.rows,
                                })
                                .await
                                .is_ok()
                            {
                                registry.mark_terminal_size(&session_id, size);
                            }
                        }
                    }
                }
            }
            status = &mut status_rx => {
                let status = status.ok().flatten();
                let exit_code = status.as_ref().and_then(exit_code_from_status);
                registry.mark_exited(&session_id, exit_code);
                send(
                    &channel,
                    PodExecSessionMessage::Exited {
                        session_id: session_id.clone(),
                        exit_code,
                        reason: status.as_ref().and_then(|status| status.reason.clone()),
                        message: status.as_ref().and_then(|status| status.message.clone()),
                    },
                );
                break;
            }
        }
    }

    drop(stdin);
    for reader in readers {
        let _ = reader.await;
    }
    let _ = attached.join().await;
    send(&channel, PodExecSessionMessage::Stopped { session_id });
}

async fn start_pod_exec_session_in_registry(
    request: PodExecSessionRequest,
    channel: Channel<PodExecSessionMessage>,
    registry: &PodExecRegistry,
) -> Result<PodExecSessionSummary, AppError> {
    let request = validate_request(&request)?;
    let session_id = registry.session_id();
    let summary = PodExecSessionSummary {
        id: session_id.clone(),
        cluster_context: request.cluster_context.clone(),
        namespace: request.namespace.clone(),
        pod_name: request.pod_name.clone(),
        container: request.container.clone(),
        command: request.command.clone(),
        stdin: request.stdin,
        tty: request.tty,
        terminal_cols: request.terminal_size.cols,
        terminal_rows: request.terminal_size.rows,
        status: "starting".to_string(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        exit_code: None,
        last_error: None,
    };
    let (commands, command_rx) = mpsc::unbounded_channel();
    let registry_for_task = registry.clone();
    let handle = tauri::async_runtime::spawn(run_exec_session(
        summary.clone(),
        request,
        command_rx,
        channel.clone(),
        registry_for_task,
    ));
    let summary = registry.insert(summary, commands, handle);
    send(
        &channel,
        PodExecSessionMessage::Started {
            session_id,
            summary: summary.clone(),
        },
    );
    Ok(summary)
}

#[tauri::command]
pub async fn start_pod_exec_session(
    request: PodExecSessionRequest,
    channel: Channel<PodExecSessionMessage>,
    registry: State<'_, PodExecRegistry>,
) -> Result<PodExecSessionSummary, AppError> {
    start_pod_exec_session_in_registry(request, channel, registry.inner()).await
}

#[tauri::command]
pub async fn write_pod_exec_stdin(
    session_id: String,
    data: String,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    registry.send_command(&session_id, ExecCommand::Stdin(data.into_bytes()))?;
    Ok(true)
}

#[tauri::command]
pub async fn resize_pod_exec_terminal(
    session_id: String,
    size: PodExecTerminalSize,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    validate_terminal_size(&size)?;
    registry.send_command(&session_id, ExecCommand::Resize(size))?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_pod_exec_session(
    session_id: String,
    registry: State<'_, PodExecRegistry>,
) -> Result<bool, AppError> {
    Ok(registry.stop(&session_id))
}

#[tauri::command]
pub async fn list_pod_exec_sessions(
    registry: State<'_, PodExecRegistry>,
) -> Result<Vec<PodExecSessionSummary>, AppError> {
    Ok(registry.list())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> PodExecSessionRequest {
        PodExecSessionRequest {
            cluster_context: "kind-dev".to_string(),
            namespace: "default".to_string(),
            pod_name: "api-0".to_string(),
            container: Some("api".to_string()),
            command: vec!["/bin/sh".to_string()],
            stdin: true,
            tty: true,
            terminal_size: PodExecTerminalSize { cols: 100, rows: 32 },
            confirmation: crate::models::PodExecConfirmation {
                acknowledged: true,
                target: "kind-dev/default/Pod/api-0".to_string(),
                command: "/bin/sh".to_string(),
            },
        }
    }

    fn test_summary(id: &str) -> PodExecSessionSummary {
        PodExecSessionSummary {
            id: id.to_string(),
            cluster_context: "kind-dev".to_string(),
            namespace: "default".to_string(),
            pod_name: "api-0".to_string(),
            container: Some("api".to_string()),
            command: vec!["/bin/sh".to_string()],
            stdin: true,
            tty: true,
            terminal_cols: 100,
            terminal_rows: 32,
            status: "running".to_string(),
            started_at: "2026-06-01T00:00:00Z".to_string(),
            finished_at: None,
            exit_code: None,
            last_error: None,
        }
    }

    #[test]
    fn validates_target_command_confirmation_and_terminal_size() {
        assert!(validate_request(&valid_request()).is_ok());

        assert_eq!(
            validate_request(&PodExecSessionRequest {
                pod_name: " ".to_string(),
                ..valid_request()
            })
            .expect_err("missing target")
            .message,
            "pod exec target is required",
        );
        assert_eq!(
            validate_request(&PodExecSessionRequest {
                command: vec![],
                ..valid_request()
            })
            .expect_err("missing command")
            .message,
            "pod exec command is required",
        );
        assert_eq!(
            validate_request(&PodExecSessionRequest {
                confirmation: crate::models::PodExecConfirmation {
                    acknowledged: false,
                    ..valid_request().confirmation
                },
                ..valid_request()
            })
            .expect_err("missing confirmation")
            .message,
            "pod exec requires explicit confirmation",
        );
        assert_eq!(
            validate_request(&PodExecSessionRequest {
                terminal_size: PodExecTerminalSize { cols: 0, rows: 24 },
                ..valid_request()
            })
            .expect_err("bad size")
            .message,
            "terminal size must be between 1 and 500 columns and rows",
        );
    }

    #[test]
    fn registry_lists_marks_resizes_and_stops_sessions() {
        let registry = PodExecRegistry::default();
        registry.insert_summary_for_test(test_summary("exec-1"));

        assert_eq!(registry.list().len(), 1);
        registry.mark_error("exec-1", "forbidden".to_string());
        let session = registry.list().pop().expect("session");
        assert_eq!(session.status, "error");
        assert_eq!(session.last_error.as_deref(), Some("forbidden"));

        registry.mark_terminal_size("exec-1", PodExecTerminalSize { cols: 120, rows: 40 });
        let session = registry.list().pop().expect("session");
        assert_eq!(session.terminal_cols, 120);
        assert_eq!(session.terminal_rows, 40);

        registry.mark_exited("exec-1", Some(0));
        let session = registry.list().pop().expect("session");
        assert_eq!(session.status, "exited");
        assert_eq!(session.exit_code, Some(0));

        assert!(registry.stop("exec-1"));
        assert!(!registry.stop("exec-1"));
        assert!(registry.list().is_empty());
    }
}
