mod kinds;
mod logs;
mod registry;
mod watch;

use crate::models::{AppError, PodLogStreamRequest, StreamMessage, WatchResourceKey};
use kube::{config::KubeConfigOptions, Client};
pub use registry::StreamRegistry;
use tauri::{ipc::Channel, State};

fn send(channel: &Channel<StreamMessage>, message: StreamMessage) -> bool {
    channel.send(message).is_ok()
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

#[tauri::command]
pub async fn start_resource_watch(
    cluster_context: String,
    keys: Vec<WatchResourceKey>,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    if cluster_context.trim().is_empty() || keys.is_empty() {
        return Err(AppError::new(
            "resource watch scope is required",
            "validation",
        ));
    }
    let stream_id = registry.stream_id("resources");
    let mut handles = Vec::with_capacity(keys.len());
    for key in keys {
        let handle = tauri::async_runtime::spawn(watch::run_resource_watch(
            stream_id.clone(),
            cluster_context.clone(),
            key,
            channel.clone(),
        ));
        handles.push(handle);
    }
    registry.insert(stream_id.clone(), handles);
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "resources".to_string(),
        },
    );
    Ok(stream_id)
}

#[tauri::command]
pub async fn start_resource_event_watch(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    if cluster_context.trim().is_empty() || kind.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::new(
            "event watch target is required",
            "validation",
        ));
    }
    let stream_id = registry.stream_id("events");
    let handle = tauri::async_runtime::spawn(watch::run_event_watch(
        stream_id.clone(),
        cluster_context,
        kind,
        name,
        namespace,
        channel.clone(),
    ));
    registry.insert(stream_id.clone(), vec![handle]);
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "events".to_string(),
        },
    );
    Ok(stream_id)
}

#[tauri::command]
pub async fn start_pod_log_stream(
    request: PodLogStreamRequest,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    if request.cluster_context.trim().is_empty()
        || request.namespace.trim().is_empty()
        || request.pod_name.trim().is_empty()
    {
        return Err(AppError::new(
            "pod log stream target is required",
            "validation",
        ));
    }
    if matches!(request.tail_lines, Some(tail_lines) if tail_lines < 0) {
        return Err(AppError::new(
            "tail_lines must be non-negative",
            "validation",
        ));
    }
    let stream_id = registry.stream_id("logs");
    let handle = tauri::async_runtime::spawn(logs::run_pod_log_stream(
        stream_id.clone(),
        request,
        channel.clone(),
    ));
    registry.insert(stream_id.clone(), vec![handle]);
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "logs".to_string(),
        },
    );
    Ok(stream_id)
}

#[tauri::command]
pub async fn stop_stream(
    stream_id: String,
    registry: State<'_, StreamRegistry>,
) -> Result<bool, AppError> {
    Ok(registry.stop(&stream_id))
}
