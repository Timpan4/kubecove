use super::{
    client_for_context,
    kinds::{api_resource_from_kind, normalize_resource_kind},
    send,
};
use crate::models::{StreamMessage, WatchResourceKey, WatchResourceTarget};
use futures_util::StreamExt;
use k8s_openapi::api::core::v1::Event;
use kube::{
    api::{Api, DynamicObject, WatchEvent, WatchParams},
    core::Status,
    Client,
};
use std::time::Duration;
use tauri::ipc::Channel;

async fn sleep(duration: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
}

fn event_action<K>(event: &WatchEvent<K>) -> String {
    match event {
        WatchEvent::Added(_) => "added",
        WatchEvent::Modified(_) => "modified",
        WatchEvent::Deleted(_) => "deleted",
        WatchEvent::Bookmark(_) => "bookmark",
        WatchEvent::Error(_) => "error",
    }
    .to_string()
}

fn watch_status_message(status: &Status) -> String {
    let reason = if status.reason.is_empty() {
        "unknown"
    } else {
        &status.reason
    };
    let message = if status.message.is_empty() {
        "Kubernetes watch returned an error"
    } else {
        &status.message
    };

    match status.code {
        0 => format!("Kubernetes watch error ({reason}): {message}"),
        code => format!("Kubernetes watch error {code} ({reason}): {message}"),
    }
}

fn dynamic_event_target(
    cluster_context: &str,
    kind: &str,
    event: &WatchEvent<DynamicObject>,
) -> WatchResourceTarget {
    let object = match event {
        WatchEvent::Added(object) | WatchEvent::Modified(object) | WatchEvent::Deleted(object) => {
            Some(object)
        }
        WatchEvent::Bookmark(_) | WatchEvent::Error(_) => None,
    };

    WatchResourceTarget {
        cluster: cluster_context.to_string(),
        kind: kind.to_string(),
        namespace: object.and_then(|object| object.metadata.namespace.clone()),
        name: object.and_then(|object| object.metadata.name.clone()),
    }
}

fn dynamic_event_resource_version(event: &WatchEvent<DynamicObject>) -> Option<String> {
    match event {
        WatchEvent::Added(object) | WatchEvent::Modified(object) | WatchEvent::Deleted(object) => {
            object.metadata.resource_version.clone()
        }
        WatchEvent::Bookmark(bookmark) => Some(bookmark.metadata.resource_version.clone()),
        WatchEvent::Error(_) => None,
    }
}

fn event_resource_version(event: &WatchEvent<Event>) -> Option<String> {
    match event {
        WatchEvent::Added(object) | WatchEvent::Modified(object) | WatchEvent::Deleted(object) => {
            object.metadata.resource_version.clone()
        }
        WatchEvent::Bookmark(bookmark) => Some(bookmark.metadata.resource_version.clone()),
        WatchEvent::Error(_) => None,
    }
}

fn scoped_dynamic_api(
    client: Client,
    key: &WatchResourceKey,
    namespaced: bool,
    api_resource: &kube::api::ApiResource,
) -> Api<DynamicObject> {
    if !namespaced {
        return Api::all_with(client, api_resource);
    }
    match key.namespace.as_deref() {
        Some(namespace) if !namespace.is_empty() => {
            Api::namespaced_with(client, namespace, api_resource)
        }
        _ => Api::all_with(client, api_resource),
    }
}

pub(super) async fn run_resource_watch(
    stream_id: String,
    cluster_context: String,
    key: WatchResourceKey,
    channel: Channel<StreamMessage>,
) {
    let normalized_kind = match normalize_resource_kind(&key.resource_kind) {
        Ok(kind) => kind,
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
    let api_resource = match api_resource_from_kind(&normalized_kind) {
        Ok(api_resource) => api_resource,
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
    let namespaced = normalized_kind.namespaced.unwrap_or(true);
    let kind_label = normalized_kind.kind.clone();
    let mut resource_version = "0".to_string();

    loop {
        let client = match client_for_context(&cluster_context).await {
            Ok(client) => client,
            Err(err) => {
                if !send(
                    &channel,
                    StreamMessage::Error {
                        stream_id: stream_id.clone(),
                        message: err.message,
                    },
                ) {
                    return;
                }
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let api = scoped_dynamic_api(client, &key, namespaced, &api_resource);
        let params = WatchParams::default().timeout(30);

        if !send(
            &channel,
            StreamMessage::Status {
                stream_id: stream_id.clone(),
                status: "connected".to_string(),
                message: format!("Watching {}", kind_label),
            },
        ) {
            return;
        }

        match api.watch(&params, &resource_version).await {
            Ok(stream) => {
                let mut stream = stream.boxed();
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            if let WatchEvent::Error(status) = event {
                                resource_version = "0".to_string();
                                if !send(
                                    &channel,
                                    StreamMessage::Error {
                                        stream_id: stream_id.clone(),
                                        message: watch_status_message(&status),
                                    },
                                ) {
                                    return;
                                }
                                break;
                            }
                            if let Some(next_resource_version) =
                                dynamic_event_resource_version(&event)
                            {
                                resource_version = next_resource_version;
                            }
                            if matches!(event, WatchEvent::Bookmark(_)) {
                                continue;
                            }
                            if !send(
                                &channel,
                                StreamMessage::ResourceChanged {
                                    stream_id: stream_id.clone(),
                                    target: dynamic_event_target(
                                        &cluster_context,
                                        &kind_label,
                                        &event,
                                    ),
                                    action: event_action(&event),
                                },
                            ) {
                                return;
                            }
                        }
                        Err(err) => {
                            if !send(
                                &channel,
                                StreamMessage::Error {
                                    stream_id: stream_id.clone(),
                                    message: err.to_string(),
                                },
                            ) {
                                return;
                            }
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                if !send(
                    &channel,
                    StreamMessage::Error {
                        stream_id: stream_id.clone(),
                        message: err.to_string(),
                    },
                ) {
                    return;
                }
            }
        }

        if !send(
            &channel,
            StreamMessage::Status {
                stream_id: stream_id.clone(),
                status: "reconnecting".to_string(),
                message: format!("Reconnecting {}", kind_label),
            },
        ) {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }
}

fn event_target(
    cluster_context: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> WatchResourceTarget {
    WatchResourceTarget {
        cluster: cluster_context.to_string(),
        kind: kind.to_string(),
        namespace: namespace.map(ToString::to_string),
        name: Some(name.to_string()),
    }
}

pub(super) async fn run_event_watch(
    stream_id: String,
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    channel: Channel<StreamMessage>,
) {
    let field_selector = match namespace.as_deref() {
        Some(ns) => format!(
            "involvedObject.kind={},involvedObject.name={},involvedObject.namespace={}",
            kind, name, ns,
        ),
        None => format!("involvedObject.kind={},involvedObject.name={}", kind, name),
    };
    let mut resource_version = "0".to_string();

    loop {
        let client = match client_for_context(&cluster_context).await {
            Ok(client) => client,
            Err(err) => {
                if !send(
                    &channel,
                    StreamMessage::Error {
                        stream_id: stream_id.clone(),
                        message: err.message,
                    },
                ) {
                    return;
                }
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let api: Api<Event> = if let Some(namespace) = &namespace {
            Api::namespaced(client, namespace)
        } else {
            Api::all(client)
        };
        let params = WatchParams::default().fields(&field_selector).timeout(30);

        if !send(
            &channel,
            StreamMessage::Status {
                stream_id: stream_id.clone(),
                status: "connected".to_string(),
                message: "Watching events".to_string(),
            },
        ) {
            return;
        }

        match api.watch(&params, &resource_version).await {
            Ok(stream) => {
                let mut stream = stream.boxed();
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            if let WatchEvent::Error(status) = event {
                                resource_version = "0".to_string();
                                if !send(
                                    &channel,
                                    StreamMessage::Error {
                                        stream_id: stream_id.clone(),
                                        message: watch_status_message(&status),
                                    },
                                ) {
                                    return;
                                }
                                break;
                            }
                            if let Some(next_resource_version) = event_resource_version(&event) {
                                resource_version = next_resource_version;
                            }
                            if matches!(event, WatchEvent::Bookmark(_)) {
                                continue;
                            }
                            if !send(
                                &channel,
                                StreamMessage::ResourceEventsChanged {
                                    stream_id: stream_id.clone(),
                                    target: event_target(
                                        &cluster_context,
                                        &kind,
                                        &name,
                                        namespace.as_deref(),
                                    ),
                                    action: event_action(&event),
                                },
                            ) {
                                return;
                            }
                        }
                        Err(err) => {
                            if !send(
                                &channel,
                                StreamMessage::Error {
                                    stream_id: stream_id.clone(),
                                    message: err.to_string(),
                                },
                            ) {
                                return;
                            }
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                if !send(
                    &channel,
                    StreamMessage::Error {
                        stream_id: stream_id.clone(),
                        message: err.to_string(),
                    },
                ) {
                    return;
                }
            }
        }

        if !send(
            &channel,
            StreamMessage::Status {
                stream_id: stream_id.clone(),
                status: "reconnecting".to_string(),
                message: "Reconnecting events".to_string(),
            },
        ) {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_status_message_includes_api_error_context() {
        let status = Status {
            code: 410,
            reason: "Expired".to_string(),
            message: "too old resource version".to_string(),
            ..Status::default()
        };

        assert_eq!(
            watch_status_message(&status),
            "Kubernetes watch error 410 (Expired): too old resource version",
        );
    }
}
