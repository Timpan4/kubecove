use crate::models::AppErrorKind;
mod aggregate_logs;
mod kinds;
mod log_batches;
mod logs;
mod registry;
mod watch;

#[cfg(feature = "bench-support")]
pub use aggregate_logs::benchmark_log_sources_from_pods;
#[cfg(feature = "bench-support")]
pub use logs::benchmark_read_log_line;
#[cfg(feature = "bench-support")]
pub use logs::MAX_LOG_LINE_BYTES;

use crate::commands::{ClusterAccess, ClusterLiveStore};
use crate::models::{
    AggregatedLogStreamRequest, AppError, PodLogStreamRequest, StreamMessage, WatchResourceKey,
};
use crate::runtime::{Channel, State};
pub use registry::StreamRegistry;

fn send(channel: &Channel<StreamMessage>, message: StreamMessage) -> bool {
    channel.send(message).is_ok()
}

fn validate_resource_watch_scope(
    cluster_context: &str,
    keys: &[WatchResourceKey],
) -> Result<(), AppError> {
    for key in keys {
        crate::commands::helpers::validate_namespace(key.namespace.as_deref())?;
        kinds::api_resource_from_kind(&key.resource_kind)?;
    }
    if cluster_context.trim().is_empty() || keys.is_empty() {
        return Err(AppError::new(
            "resource watch scope is required",
            AppErrorKind::Validation,
        ));
    }

    Ok(())
}

fn validate_event_watch_target(
    cluster_context: &str,
    kind: &str,
    name: &str,
) -> Result<(), AppError> {
    if cluster_context.trim().is_empty() || kind.trim().is_empty() || name.trim().is_empty() {
        return Err(AppError::new(
            "event watch target is required",
            AppErrorKind::Validation,
        ));
    }

    Ok(())
}

/// Mirrors `MAX_RETAINED_LOG_LINES` in `src/features/resource-detail/log-helpers.ts`;
/// the frontend never keeps more lines than this, so the backend never requests more.
const MAX_LOG_TAIL_LINES: i64 = 1_000;

fn clamp_tail_lines(tail_lines: Option<i64>) -> i64 {
    tail_lines.unwrap_or(200).min(MAX_LOG_TAIL_LINES)
}

fn validate_pod_log_stream_request(request: &PodLogStreamRequest) -> Result<(), AppError> {
    if request.cluster_context.trim().is_empty()
        || request.namespace.trim().is_empty()
        || request.pod_name.trim().is_empty()
    {
        return Err(AppError::new(
            "pod log stream target is required",
            AppErrorKind::Validation,
        ));
    }
    crate::commands::helpers::validate_namespace(Some(&request.namespace))?;
    crate::commands::helpers::validate_path_segment(&request.pod_name, "Pod name")?;
    validate_log_window(request.tail_lines, request.since_seconds)
}

fn validate_log_window(
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
) -> Result<(), AppError> {
    if matches!(tail_lines, Some(tail_lines) if tail_lines < 0) {
        return Err(AppError::new(
            "tail_lines must be non-negative",
            AppErrorKind::Validation,
        ));
    }
    if matches!(since_seconds, Some(since_seconds) if since_seconds < 0) {
        return Err(AppError::new(
            "since_seconds must be non-negative",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

fn validate_aggregated_log_stream_request(
    request: &AggregatedLogStreamRequest,
) -> Result<(), AppError> {
    if request.cluster_context.trim().is_empty()
        || request.namespace.trim().is_empty()
        || request.target_kind.trim().is_empty()
        || request.target_name.trim().is_empty()
    {
        return Err(AppError::new(
            "aggregated log stream target is required",
            AppErrorKind::Validation,
        ));
    }
    crate::commands::helpers::validate_namespace(Some(&request.namespace))?;
    crate::commands::helpers::validate_path_segment(&request.target_name, "log target")?;
    if !matches!(request.target_kind.as_str(), "Deployment" | "Service") {
        return Err(AppError::new(
            "aggregated logs support Deployment and selector-backed Service targets",
            AppErrorKind::Validation,
        ));
    }
    validate_log_window(request.tail_lines, request.since_seconds)
}

pub async fn refresh_resource_cache(
    cluster_context: String,
    mut keys: Vec<WatchResourceKey>,
    namespaces: Vec<String>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<crate::models::ResourceCacheRefreshResult, AppError> {
    if cluster_context.trim().is_empty() {
        return Err(AppError::new(
            "resource context is required",
            AppErrorKind::Validation,
        ));
    }
    for key in &mut keys {
        crate::commands::helpers::validate_namespace(key.namespace.as_deref())?;
        kinds::api_resource_from_kind(&key.resource_kind)?;
        key.resource_kind = kinds::normalize_resource_kind(&key.resource_kind)?;
    }
    for namespace in &namespaces {
        crate::commands::helpers::validate_namespace(Some(namespace))?;
    }
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    Ok(crate::models::ResourceCacheRefreshResult {
        cleared_entries: live_store.refresh_view(
            access.source_key(),
            &cluster_context,
            &keys,
            &namespaces,
        ),
    })
}

pub async fn start_resource_watch(
    cluster_context: String,
    keys: Vec<WatchResourceKey>,
    kubeconfig_env_var: Option<String>,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<String, AppError> {
    validate_resource_watch_scope(&cluster_context, &keys)?;
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    let stream_id = registry.stream_id("resources");
    for key in keys {
        let (watch_key, broadcaster, should_start) = registry.subscribe_resource(
            &stream_id,
            access.source_key(),
            &cluster_context,
            &key,
            channel.clone(),
        );
        if should_start {
            let handle = crate::runtime::spawn(watch::run_resource_watch(
                access.clone(),
                cluster_context.clone(),
                key,
                broadcaster.clone(),
                live_store.inner().clone(),
            ));
            registry.set_resource_handle(&watch_key, &broadcaster, handle);
        }
    }
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "resources".to_string(),
        },
    );
    Ok(stream_id)
}

pub async fn start_resource_event_watch(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    crate::commands::helpers::validate_namespace(namespace.as_deref())?;
    validate_event_watch_target(&cluster_context, &kind, &name)?;
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    let stream_id = registry.stream_id("events");
    let (watch_key, broadcaster, should_start) = registry.subscribe_event(
        &stream_id,
        access.source_key(),
        &cluster_context,
        &kind,
        &name,
        namespace.as_deref(),
        channel.clone(),
    );
    if should_start {
        let handle = crate::runtime::spawn(watch::run_event_watch(
            access,
            cluster_context,
            kind,
            name,
            namespace,
            broadcaster.clone(),
        ));
        registry.set_event_handle(&watch_key, &broadcaster, handle);
    }
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "events".to_string(),
        },
    );
    Ok(stream_id)
}

pub async fn start_pod_log_stream(
    request: PodLogStreamRequest,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    validate_pod_log_stream_request(&request)?;
    let access = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?;
    let stream_id = registry.stream_id("logs");
    let handle = crate::runtime::spawn(logs::run_pod_log_stream(
        stream_id.clone(),
        access,
        request,
        channel.clone(),
    ));
    registry.insert_handles(stream_id.clone(), vec![handle]);
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "logs".to_string(),
        },
    );
    Ok(stream_id)
}

pub async fn start_aggregated_log_stream(
    request: AggregatedLogStreamRequest,
    channel: Channel<StreamMessage>,
    registry: State<'_, StreamRegistry>,
) -> Result<String, AppError> {
    validate_aggregated_log_stream_request(&request)?;
    let access = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?;
    let stream_id = registry.stream_id("logs");
    let handle = crate::runtime::spawn(aggregate_logs::run_aggregated_log_stream(
        stream_id.clone(),
        access,
        request,
        channel.clone(),
    ));
    registry.insert_handles(stream_id.clone(), vec![handle]);
    send(
        &channel,
        StreamMessage::Started {
            stream_id: stream_id.clone(),
            label: "logs".to_string(),
        },
    );
    Ok(stream_id)
}

pub async fn stop_stream(
    stream_id: String,
    registry: State<'_, StreamRegistry>,
) -> Result<bool, AppError> {
    Ok(registry.stop(&stream_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_log_request() -> PodLogStreamRequest {
        PodLogStreamRequest {
            batch_lines: false,
            cluster_context: "kind-dev".to_string(),
            kubeconfig_env_var: None,
            namespace: "default".to_string(),
            pod_name: "api-0".to_string(),
            container: Some("api".to_string()),
            tail_lines: Some(200),
            since_seconds: None,
        }
    }

    fn valid_aggregated_log_request() -> AggregatedLogStreamRequest {
        AggregatedLogStreamRequest {
            batch_lines: false,
            cluster_context: "kind-dev".to_string(),
            kubeconfig_env_var: None,
            namespace: "default".to_string(),
            target_kind: "Deployment".to_string(),
            target_name: "api".to_string(),
            tail_lines: Some(200),
            since_seconds: None,
        }
    }

    #[test]
    fn pod_log_validation_requires_target_and_non_negative_tail() {
        assert!(validate_pod_log_stream_request(&valid_log_request()).is_ok());

        assert_eq!(
            validate_pod_log_stream_request(&PodLogStreamRequest {
                cluster_context: String::new(),
                ..valid_log_request()
            })
            .expect_err("empty context")
            .message,
            "pod log stream target is required",
        );
        assert_eq!(
            validate_pod_log_stream_request(&PodLogStreamRequest {
                namespace: " ".to_string(),
                ..valid_log_request()
            })
            .expect_err("empty namespace")
            .kind,
            AppErrorKind::Validation,
        );
        assert_eq!(
            validate_pod_log_stream_request(&PodLogStreamRequest {
                pod_name: String::new(),
                ..valid_log_request()
            })
            .expect_err("empty pod")
            .kind,
            AppErrorKind::Validation,
        );
        assert_eq!(
            validate_pod_log_stream_request(&PodLogStreamRequest {
                tail_lines: Some(-1),
                ..valid_log_request()
            })
            .expect_err("negative tail")
            .message,
            "tail_lines must be non-negative",
        );
        assert_eq!(
            validate_pod_log_stream_request(&PodLogStreamRequest {
                since_seconds: Some(-1),
                ..valid_log_request()
            })
            .expect_err("negative since")
            .message,
            "since_seconds must be non-negative",
        );
    }

    #[test]
    fn aggregated_log_validation_requires_supported_target_and_non_negative_limits() {
        assert!(validate_aggregated_log_stream_request(&valid_aggregated_log_request()).is_ok());

        assert_eq!(
            validate_aggregated_log_stream_request(&AggregatedLogStreamRequest {
                target_name: String::new(),
                ..valid_aggregated_log_request()
            })
            .expect_err("empty target")
            .message,
            "aggregated log stream target is required",
        );
        assert_eq!(
            validate_aggregated_log_stream_request(&AggregatedLogStreamRequest {
                target_kind: "StatefulSet".to_string(),
                ..valid_aggregated_log_request()
            })
            .expect_err("unsupported kind")
            .message,
            "aggregated logs support Deployment and selector-backed Service targets",
        );
        assert_eq!(
            validate_aggregated_log_stream_request(&AggregatedLogStreamRequest {
                tail_lines: Some(-1),
                ..valid_aggregated_log_request()
            })
            .expect_err("negative tail")
            .message,
            "tail_lines must be non-negative",
        );
        assert_eq!(
            validate_aggregated_log_stream_request(&AggregatedLogStreamRequest {
                since_seconds: Some(-1),
                ..valid_aggregated_log_request()
            })
            .expect_err("negative since")
            .message,
            "since_seconds must be non-negative",
        );
    }
}
