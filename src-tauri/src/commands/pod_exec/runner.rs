use crate::models::{
    AppError, PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary,
};
use futures_util::SinkExt;
use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::Status};
use kube::api::{Api, AttachParams, TerminalSize};
use tauri::ipc::Channel;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot},
};

use super::registry::{session_summary, ExecCommand, PodExecRegistry};
use super::validation::{client_for_context, validate_request, ValidatedPodExecRequest};

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

    let client = match client_for_context(
        &request.cluster_context,
        request.kubeconfig_env_var.clone(),
    )
    .await
    {
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

    let mut attached = match pods
        .exec(&request.pod_name, request.command.clone(), &params)
        .await
    {
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

pub(super) async fn start_pod_exec_session_in_registry(
    request: PodExecSessionRequest,
    channel: Channel<PodExecSessionMessage>,
    registry: &PodExecRegistry,
) -> Result<PodExecSessionSummary, AppError> {
    let request = validate_request(&request)?;
    let session_id = registry.session_id();
    let summary = session_summary(session_id.clone(), &request);
    let (commands, command_rx) = mpsc::unbounded_channel();
    let summary = registry.insert(summary, commands);
    send(
        &channel,
        PodExecSessionMessage::Started {
            session_id: session_id.clone(),
            summary: summary.clone(),
        },
    );
    let registry_for_task = registry.clone();
    let handle = tauri::async_runtime::spawn(run_exec_session(
        summary.clone(),
        request,
        command_rx,
        channel.clone(),
        registry_for_task,
    ));
    registry.set_handle(&session_id, handle);
    Ok(summary)
}
