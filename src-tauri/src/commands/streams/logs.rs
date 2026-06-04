use super::{client_for_context, send};
use crate::models::{PodLogStreamRequest, StreamMessage};
use futures_util::{AsyncBufReadExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, LogParams};
use tauri::ipc::Channel;

pub(super) async fn run_pod_log_stream(
    stream_id: String,
    request: PodLogStreamRequest,
    channel: Channel<StreamMessage>,
) {
    let client = match client_for_context(
        &request.cluster_context,
        request.kubeconfig_env_var.clone(),
    )
    .await
    {
        Ok(client) => client,
        Err(err) => {
            send(
                &channel,
                StreamMessage::Error {
                    stream_id,
                    message: err.message,
                },
            );
            return;
        }
    };
    let pods: Api<Pod> = Api::namespaced(client, &request.namespace);
    let params = LogParams {
        container: request.container.clone(),
        follow: true,
        tail_lines: Some(request.tail_lines.unwrap_or(200)),
        timestamps: true,
        ..LogParams::default()
    };

    if !send(
        &channel,
        StreamMessage::Status {
            stream_id: stream_id.clone(),
            status: "connected".to_string(),
            message: "Streaming logs".to_string(),
        },
    ) {
        return;
    }

    match pods.log_stream(&request.pod_name, &params).await {
        Ok(logs) => {
            let mut lines = logs.lines();
            loop {
                match lines.try_next().await {
                    Ok(Some(line)) => {
                        if !send(
                            &channel,
                            StreamMessage::LogLine {
                                stream_id: stream_id.clone(),
                                line,
                            },
                        ) {
                            return;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        send(
                            &channel,
                            StreamMessage::Error {
                                stream_id: stream_id.clone(),
                                message: err.to_string(),
                            },
                        );
                        break;
                    }
                }
            }
        }
        Err(err) => {
            send(
                &channel,
                StreamMessage::Error {
                    stream_id: stream_id.clone(),
                    message: err.to_string(),
                },
            );
        }
    }

    send(&channel, StreamMessage::Stopped { stream_id });
}
