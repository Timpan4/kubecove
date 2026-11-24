use super::{
    log_batches::{emit_log_batch, read_log_batches},
    send,
};
use crate::commands::ClusterAccess;
use crate::models::AppErrorKind;
use crate::models::{
    AggregatedLogStreamRequest, AppError, LogLineSource, StreamMessage, StreamStatus,
};
use crate::runtime::Channel;
use futures_util::StreamExt;
use k8s_openapi::{
    api::{
        apps::v1::Deployment,
        core::v1::{Pod, Service},
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{
    Client,
    api::{Api, ListParams, LogParams, WatchEvent, WatchParams},
};
use kubecove_kube::commands::SessionScope;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::Duration,
};

use tokio::task::{AbortHandle, Id, JoinSet};

const MAX_AGGREGATED_LOG_SOURCES: usize = 50;
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct LogStreamOptions {
    batch_lines: bool,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogSourceKey {
    pub pod_name: String,
    pub container: String,
}

#[derive(Default)]
struct SourceStreams {
    tasks: JoinSet<()>,
    handles: HashMap<LogSourceKey, AbortHandle>,
    /// A source only replays its tail when it first enters the selected set.
    attached: BTreeSet<LogSourceKey>,
}

impl SourceStreams {
    fn retire(&mut self, id: Id) -> bool {
        let before = self.handles.len();
        self.handles.retain(|_, handle| handle.id() != id);
        self.handles.len() != before
    }
}

struct AggregateLogs {
    client: Client,
    namespace: String,
    options: LogStreamOptions,
    channel: Channel<StreamMessage>,
    stream_id: String,
    streams: SourceStreams,
    pods: PodSources,
    selected: Vec<LogSourceKey>,
}

pub(super) async fn run_aggregated_log_stream(
    stream_id: String,
    access: ClusterAccess,
    request: AggregatedLogStreamRequest,
    channel: Channel<StreamMessage>,
) {
    let client = match access.stream_client(&request.cluster_context).await {
        Ok(client) => client,
        Err(err) => {
            send_error(&channel, stream_id, err.message);
            return;
        }
    };

    let selector = match resolve_target_selector(client.clone(), &request).await {
        Ok(selector) => selector,
        Err(err) => {
            send_error(&channel, stream_id, err.message);
            return;
        }
    };
    let pods = Api::namespaced(client.clone(), &request.namespace);
    let mut logs = AggregateLogs {
        client,
        namespace: request.namespace,
        options: LogStreamOptions {
            batch_lines: request.batch_lines,
            tail_lines: Some(super::clamp_tail_lines(request.tail_lines)),
            since_seconds: request.since_seconds,
        },
        channel,
        stream_id,
        streams: SourceStreams::default(),
        pods: PodSources::default(),
        selected: Vec::new(),
    };
    logs.watch_pods(&pods, &selector).await;
}

impl AggregateLogs {
    async fn watch_pods(&mut self, pods: &Api<Pod>, selector: &str) {
        let list_params = ListParams::default().labels(selector);
        let watch_params = WatchParams::default()
            .labels(selector)
            .timeout(super::watch::WATCH_TIMEOUT_SECS);
        let mut cursor = None;
        loop {
            let needs_list = cursor.is_none();
            if needs_list {
                match pods.list(&list_params).await {
                    Ok(list) => {
                        cursor = usable_version(list.metadata.resource_version);
                        self.pods = PodSources::from_pods(&list.items);
                        if !self.sync_sources(None) {
                            return;
                        }
                    }
                    Err(err) => {
                        if !self.reconnecting(format!("Log pod list failed: {err}")) {
                            return;
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                }
            }

            match pods
                .watch(&watch_params, cursor.as_deref().unwrap_or("0"))
                .await
            {
                Ok(stream) => {
                    if !needs_list && !self.connected() {
                        return;
                    }
                    let mut stream = stream.boxed();
                    loop {
                        let event = tokio::select! {
                            event = stream.next() => event,
                            finished = self.streams.tasks.join_next_with_id(), if !self.streams.tasks.is_empty() => {
                                if let Some(finished) = finished {
                                    let id = finished.map_or_else(|error| error.id(), |(id, ())| id);
                                    if self.streams.retire(id) {
                                        self.reconcile_sources();
                                    }
                                }
                                continue;
                            }
                        };
                        match event {
                            Some(Ok(WatchEvent::Error(status))) => {
                                if status.code == 410 {
                                    cursor = None;
                                }
                                let message = if status.message.is_empty() {
                                    "Kubernetes pod watch returned an error".to_string()
                                } else {
                                    status.message
                                };
                                if !self.reconnecting(message) {
                                    return;
                                }
                                break;
                            }
                            Some(Ok(event)) => {
                                cursor =
                                    usable_version(super::watch::watch_resource_version(&event));
                                let previous_total = self.pods.total;
                                if self.pods.apply(event)
                                    && !self.sync_sources(Some(previous_total))
                                {
                                    return;
                                }
                            }
                            Some(Err(err)) => {
                                if !self.watch_failed(&mut cursor, &err) {
                                    return;
                                }
                                break;
                            }
                            None => break,
                        }
                    }
                }
                Err(err) => {
                    if !self.watch_failed(&mut cursor, &err) {
                        return;
                    }
                }
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }

    fn watch_failed(&self, cursor: &mut Option<String>, error: &kube::Error) -> bool {
        if matches!(error, kube::Error::Api(status) if status.code == 410) {
            *cursor = None;
        }
        self.reconnecting(format!("Log pod watch failed: {error}"))
    }

    fn reconnecting(&self, message: String) -> bool {
        send_status(
            &self.channel,
            &self.stream_id,
            StreamStatus::Reconnecting,
            message,
        )
    }

    fn connected(&self) -> bool {
        send_status(
            &self.channel,
            &self.stream_id,
            StreamStatus::Connected,
            aggregate_status_message(self.pods.total, self.streams.handles.len()),
        )
    }

    fn sync_sources(&mut self, previous_total: Option<usize>) -> bool {
        let desired = self.pods.selected();
        if desired == self.selected {
            previous_total == Some(self.pods.total) || self.connected()
        } else {
            self.selected = desired;
            self.reconcile_sources();
            self.connected()
        }
    }

    fn reconcile_sources(&mut self) {
        let desired = &self.selected;
        self.streams
            .attached
            .retain(|source| desired.binary_search(source).is_ok());
        self.streams.handles.retain(|source, handle| {
            let retained = desired.binary_search(source).is_ok();
            if !retained {
                handle.abort();
            }
            retained
        });
        for source in desired {
            if self.streams.handles.contains_key(source) {
                continue;
            }
            let mut options = self.options.clone();
            if !self.streams.attached.insert(source.clone()) {
                options.tail_lines = Some(0);
            }
            let stream = stream_log_source(
                self.client.clone(),
                self.namespace.clone(),
                source.clone(),
                options,
                self.channel.clone(),
                self.stream_id.clone(),
            );
            let scope = SessionScope::current();
            let handle = self.streams.tasks.spawn(async move {
                if let Some(scope) = scope {
                    scope.run(stream).await;
                } else {
                    stream.await;
                }
                // Instant EOF or an unavailable container must not spin on log GETs.
                tokio::time::sleep(RECONNECT_DELAY).await;
            });
            self.streams.handles.insert(source.clone(), handle);
        }
    }
}

fn usable_version(version: Option<String>) -> Option<String> {
    version.filter(|version| !version.is_empty() && version != "0")
}

async fn resolve_target_selector(
    client: Client,
    request: &AggregatedLogStreamRequest,
) -> Result<String, AppError> {
    match request.target_kind.as_str() {
        "Deployment" => {
            let deployments: Api<Deployment> = Api::namespaced(client, &request.namespace);
            let deployment = deployments
                .get(&request.target_name)
                .await
                .map_err(AppError::from)?;
            selector_from_label_selector(&deployment.spec.selector)
        }
        "Service" => {
            let services: Api<Service> = Api::namespaced(client, &request.namespace);
            let service = services
                .get(&request.target_name)
                .await
                .map_err(AppError::from)?;
            service_label_selector(&service)
        }
        _ => Err(AppError::new(
            "aggregated logs support Deployment and selector-backed Service targets",
            AppErrorKind::Validation,
        )),
    }
}

fn selector_from_label_selector(selector: &LabelSelector) -> Result<String, AppError> {
    let mut parts = Vec::new();
    if let Some(labels) = &selector.match_labels {
        parts.extend(labels.iter().map(|(key, value)| format!("{key}={value}")));
    }
    if let Some(expressions) = &selector.match_expressions {
        for expression in expressions {
            let values = expression.values.clone().unwrap_or_default();
            match expression.operator.as_str() {
                "In" => {
                    if values.is_empty() {
                        return Err(AppError::new(
                            "Deployment selector In expression needs values",
                            AppErrorKind::Validation,
                        ));
                    }
                    parts.push(format!("{} in ({})", expression.key, values.join(",")));
                }
                "NotIn" => {
                    if values.is_empty() {
                        return Err(AppError::new(
                            "Deployment selector NotIn expression needs values",
                            AppErrorKind::Validation,
                        ));
                    }
                    parts.push(format!("{} notin ({})", expression.key, values.join(",")));
                }
                "Exists" => parts.push(expression.key.clone()),
                "DoesNotExist" => parts.push(format!("!{}", expression.key)),
                operator => {
                    return Err(AppError::new(
                        format!("unsupported Deployment selector operator: {operator}"),
                        AppErrorKind::Validation,
                    ));
                }
            }
        }
    }
    if parts.is_empty() {
        return Err(AppError::new(
            "Deployment selector is empty",
            AppErrorKind::Validation,
        ));
    }
    Ok(parts.join(","))
}

fn service_label_selector(service: &Service) -> Result<String, AppError> {
    let spec = service
        .spec
        .as_ref()
        .ok_or_else(|| AppError::new("service spec is unavailable", AppErrorKind::Logs))?;
    if matches!(spec.type_.as_deref(), Some("ExternalName")) {
        return Err(AppError::new(
            "ExternalName Services do not have pod logs",
            AppErrorKind::Validation,
        ));
    }
    let selector = spec
        .selector
        .as_ref()
        .filter(|selector| !selector.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "aggregated Service logs require a selector-backed Service",
                AppErrorKind::Validation,
            )
        })?;
    Ok(map_label_selector(selector))
}

fn map_label_selector(selector: &BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Default)]
struct PodSources {
    containers: BTreeMap<String, Vec<String>>,
    total: usize,
}

impl PodSources {
    fn from_pods(pods: &[Pod]) -> Self {
        let mut sources = Self::default();
        for pod in pods {
            sources.update(pod);
        }
        sources
    }

    fn apply(&mut self, event: WatchEvent<Pod>) -> bool {
        match event {
            WatchEvent::Added(pod) | WatchEvent::Modified(pod) => self.update(&pod),
            WatchEvent::Deleted(pod) => pod_name(&pod).is_some_and(|name| self.remove(name)),
            WatchEvent::Bookmark(_) | WatchEvent::Error(_) => false,
        }
    }

    fn update(&mut self, pod: &Pod) -> bool {
        let Some(name) = pod_name(pod) else {
            return false;
        };
        let Some(spec) = pod
            .spec
            .as_ref()
            .filter(|_| pod.metadata.deletion_timestamp.is_none())
        else {
            return self.remove(name);
        };
        let containers = spec
            .containers
            .iter()
            .map(|container| container.name.trim())
            .filter(|name| !name.is_empty());
        let previous = self.containers.get(name).map_or(&[][..], Vec::as_slice);
        if containers.clone().eq(previous.iter().map(String::as_str)) {
            return false;
        }
        let mut containers = containers.map(str::to_owned).collect::<Vec<_>>();
        containers.sort_unstable();
        if containers.as_slice() == previous {
            return false;
        }
        self.total = self.total - previous.len() + containers.len();
        if containers.is_empty() {
            self.containers.remove(name);
        } else {
            self.containers.insert(name.to_owned(), containers);
        }
        true
    }

    fn remove(&mut self, name: &str) -> bool {
        if let Some(previous) = self.containers.remove(name) {
            self.total -= previous.len();
            true
        } else {
            false
        }
    }

    fn selected(&self) -> Vec<LogSourceKey> {
        self.containers
            .iter()
            .flat_map(|(pod, containers)| containers.iter().map(move |container| (pod, container)))
            .take(MAX_AGGREGATED_LOG_SOURCES)
            .map(|(pod, container)| LogSourceKey {
                pod_name: pod.clone(),
                container: container.clone(),
            })
            .collect()
    }
}

fn pod_name(pod: &Pod) -> Option<&str> {
    pod.metadata
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

#[cfg(feature = "bench-support")]
pub fn benchmark_log_sources_from_pods(pods: &[Pod]) -> (Vec<LogSourceKey>, usize) {
    let sources = PodSources::from_pods(pods);
    (sources.selected(), sources.total)
}

async fn stream_log_source(
    client: Client,
    namespace: String,
    source: LogSourceKey,
    options: LogStreamOptions,
    channel: Channel<StreamMessage>,
    stream_id: String,
) {
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    let params = LogParams {
        container: Some(source.container.clone()),
        follow: true,
        tail_lines: options.tail_lines,
        since_seconds: options.since_seconds,
        timestamps: true,
        ..LogParams::default()
    };
    match pods.log_stream(&source.pod_name, &params).await {
        Ok(logs) => {
            let mut logs = logs;
            let line_source = log_line_source(&source);
            if let Err(err) = read_log_batches(&mut logs, &stream_id, Some(&line_source), |lines| {
                emit_log_batch(
                    &stream_id,
                    Some(&line_source),
                    options.batch_lines,
                    lines,
                    |message| send(&channel, message),
                )
            })
            .await
            {
                send_source_error(
                    &channel,
                    &stream_id,
                    &source,
                    options.batch_lines,
                    err.to_string(),
                );
            }
        }
        Err(err) => send_source_error(
            &channel,
            &stream_id,
            &source,
            options.batch_lines,
            err.to_string(),
        ),
    }
}

fn log_line_source(source: &LogSourceKey) -> LogLineSource {
    LogLineSource {
        pod_name: source.pod_name.clone(),
        container: Some(source.container.clone()),
    }
}

fn send_source_error(
    channel: &Channel<StreamMessage>,
    stream_id: &str,
    source: &LogSourceKey,
    batch_lines: bool,
    message: String,
) {
    emit_log_batch(
        stream_id,
        Some(&log_line_source(source)),
        batch_lines,
        vec![format!("log stream error: {message}")],
        |message| send(channel, message),
    );
}

fn send_error(channel: &Channel<StreamMessage>, stream_id: String, message: String) {
    send(channel, StreamMessage::Error { stream_id, message });
}

fn send_status(
    channel: &Channel<StreamMessage>,
    stream_id: &str,
    status: StreamStatus,
    message: String,
) -> bool {
    send(
        channel,
        StreamMessage::Status {
            stream_id: stream_id.to_string(),
            status,
            message,
        },
    )
}

fn aggregate_status_message(total_sources: usize, active_sources: usize) -> String {
    match (total_sources, active_sources) {
        (0, _) => "Watching for matching Pods".to_string(),
        (total, active) if total > active => {
            format!("Streaming first {active} of {total} pod containers")
        }
        (_, 1) => "Streaming 1 pod container".to_string(),
        (_, active) => format!("Streaming {active} pod containers"),
    }
}

#[cfg(test)]
#[path = "aggregate_logs_tests.rs"]
mod tests;
