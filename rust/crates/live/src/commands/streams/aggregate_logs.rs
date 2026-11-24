use super::{
    log_batches::{emit_log_batch, read_log_batches},
    send,
};
use crate::commands::ClusterAccess;
use crate::models::{AggregatedLogStreamRequest, LogLineSource, StreamMessage, StreamStatus};
use crate::runtime::Channel;
use futures_util::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Client,
    api::{Api, ListParams, LogParams, WatchEvent, WatchParams},
};
use kubecove_kube::commands::SessionScope;
use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use tokio::task::{AbortHandle, Id, JoinError, JoinSet};

mod targets;
#[cfg(feature = "bench-support")]
pub use targets::benchmark_log_sources_from_pods;
use targets::{PodSources, resolve_target_selector};

const MAX_AGGREGATED_LOG_SOURCES: usize = 50;
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const LIST_RETRY_DELAY: Duration = Duration::from_secs(2);
const WATCHING_FOR_PODS: &str = "Watching for matching Pods";
const POLLING_FOR_PODS: &str = "Polling for matching Pods";

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
    /// Sources that a pod event or watch renewal asked to retry while their task still
    /// ran or cooled down. They reconnect when that task finishes.
    retry: BTreeSet<LogSourceKey>,
}

impl SourceStreams {
    /// Forgets the finished task `id` and returns its source.
    fn retire(&mut self, id: Id) -> Option<LogSourceKey> {
        let source = self
            .handles
            .iter()
            .find(|(_, handle)| handle.id() == id)
            .map(|(source, _)| source.clone())?;
        self.handles.remove(&source);
        Some(source)
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
                        if !self.sync_sources(None, WATCHING_FOR_PODS) {
                            return;
                        }
                    }
                    Err(err) => {
                        if !self.reconnecting(format!("Log pod list failed: {err}")) {
                            return;
                        }
                        self.wait_joining_sources(LIST_RETRY_DELAY).await;
                        continue;
                    }
                }
            }

            let failure = match pods
                .watch(&watch_params, cursor.as_deref().unwrap_or("0"))
                .await
            {
                Ok(stream) => {
                    if !needs_list {
                        // A renewed watch retries finished sources, as a relist does.
                        self.reconcile_sources();
                        if !self.connected(WATCHING_FOR_PODS) {
                            return;
                        }
                    }
                    let mut stream = stream.boxed();
                    loop {
                        let event = tokio::select! {
                            event = stream.next() => event,
                            finished = self.streams.tasks.join_next_with_id(), if !self.streams.tasks.is_empty() => {
                                // A finished source waits for a pod event or the watch
                                // renewal before it reconnects. Reconnecting here would
                                // request a waiting or completed container's log every second.
                                // A retry asked for while the task ran is honored now.
                                if let Some(finished) = finished {
                                    self.source_finished(finished);
                                }
                                continue;
                            }
                        };
                        match event {
                            Some(Ok(WatchEvent::Error(status))) => {
                                let message = if status.message.is_empty() {
                                    "Kubernetes pod watch returned an error".to_string()
                                } else {
                                    status.message
                                };
                                break Some((Some(status.code), message));
                            }
                            Some(Ok(event)) => {
                                cursor =
                                    usable_version(super::watch::watch_resource_version(&event));
                                let previous_total = self.pods.total();
                                let pod_event = !matches!(event, WatchEvent::Bookmark(_));
                                if self.pods.apply(event) {
                                    if !self.sync_sources(Some(previous_total), WATCHING_FOR_PODS) {
                                        return;
                                    }
                                } else if pod_event {
                                    // A status change, such as a restarted container, can
                                    // make a finished source's log readable again.
                                    self.reconcile_sources();
                                }
                            }
                            Some(Err(err)) => break Some(watch_failure(&err)),
                            None => break None,
                        }
                    }
                }
                Err(err) => Some(watch_failure(&err)),
            };
            if let Some((code, message)) = failure {
                match code {
                    Some(403) => return self.poll_pods(pods, &list_params).await,
                    Some(410) => cursor = None,
                    _ => {}
                }
                if !self.reconnecting(message) {
                    return;
                }
            }
            self.wait_joining_sources(RECONNECT_DELAY).await;
        }
    }

    async fn poll_pods(&mut self, pods: &Api<Pod>, params: &ListParams) {
        // Both clients replace the status text as logs arrive. Put the one-time warning
        // in the log output so it remains visible alongside container logs.
        if !emit_log_batch(
            &self.stream_id,
            None,
            self.options.batch_lines,
            vec![format!(
                "WARN Pod watch permission denied. {POLLING_FOR_PODS}."
            )],
            |message| send(&self.channel, message),
        ) || !self.connected(POLLING_FOR_PODS)
        {
            return;
        }

        let mut list_failed = false;
        loop {
            let delay = if list_failed {
                LIST_RETRY_DELAY
            } else {
                RECONNECT_DELAY
            };
            self.wait_joining_sources(delay).await;
            match pods.list(params).await {
                Ok(list) => {
                    let previous_total = (!list_failed).then_some(self.pods.total());
                    self.pods = PodSources::from_pods(&list.items);
                    if !self.sync_sources(previous_total, POLLING_FOR_PODS) {
                        return;
                    }
                    list_failed = false;
                }
                Err(err) => {
                    if !list_failed && !self.reconnecting(format!("Log pod list failed: {err}")) {
                        return;
                    }
                    list_failed = true;
                }
            }
        }
    }

    /// Retires tasks during retry delays and between polls, when there is no watch
    /// stream to join them. The next list can then reconnect a finished source.
    async fn wait_joining_sources(&mut self, delay: Duration) {
        let wait = tokio::time::sleep(delay);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => return,
                Some(finished) = self.streams.tasks.join_next_with_id(), if !self.streams.tasks.is_empty() => {
                    self.source_finished(finished);
                }
            }
        }
    }

    fn reconnecting(&self, message: String) -> bool {
        send_status(
            &self.channel,
            &self.stream_id,
            StreamStatus::Reconnecting,
            message,
        )
    }

    fn connected(&self, empty_message: &str) -> bool {
        send_status(
            &self.channel,
            &self.stream_id,
            StreamStatus::Connected,
            aggregate_status_message(self.pods.total(), self.streams.handles.len(), empty_message),
        )
    }

    fn sync_sources(&mut self, previous_total: Option<usize>, empty_message: &str) -> bool {
        let desired = self.pods.selected();
        let changed = desired != self.selected;
        self.selected = desired;
        self.reconcile_sources();
        (!changed && previous_total == Some(self.pods.total())) || self.connected(empty_message)
    }

    /// Retries every selected source: a source whose task still runs or cools down is marked
    /// to retry when it finishes. Only pod events, relists and watch renewals call this.
    fn reconcile_sources(&mut self) {
        let desired = std::mem::take(&mut self.selected);
        self.streams
            .attached
            .retain(|source| desired.binary_search(source).is_ok());
        self.streams
            .retry
            .retain(|source| desired.binary_search(source).is_ok());
        self.streams.handles.retain(|source, handle| {
            let retained = desired.binary_search(source).is_ok();
            if !retained {
                handle.abort();
            }
            retained
        });
        for source in &desired {
            if self.streams.handles.contains_key(source) {
                self.streams.retry.insert(source.clone());
            } else {
                self.streams.retry.remove(source);
                self.spawn_source(source.clone());
            }
        }
        self.selected = desired;
    }

    /// Forgets a finished task and reconnects only its own source, if a retry was asked for.
    /// Reconciling every source here would mark the others still cooling down for retry, and
    /// sources that finish at different times would then keep re-arming each other.
    fn source_finished(&mut self, finished: Result<(Id, ()), JoinError>) {
        let id = finished.map_or_else(|error| error.id(), |(id, ())| id);
        if let Some(source) = self.streams.retire(id)
            && self.streams.retry.remove(&source)
        {
            self.spawn_source(source);
        }
    }

    fn spawn_source(&mut self, source: LogSourceKey) {
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
        self.streams.handles.insert(source, handle);
    }
}

fn usable_version(version: Option<String>) -> Option<String> {
    version.filter(|version| !version.is_empty() && version != "0")
}

fn watch_failure(error: &kube::Error) -> (Option<u16>, String) {
    let code = match error {
        kube::Error::Api(status) => Some(status.code),
        _ => None,
    };
    (code, format!("Log pod watch failed: {error}"))
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

fn aggregate_status_message(
    total_sources: usize,
    active_sources: usize,
    empty_message: &str,
) -> String {
    match (total_sources, active_sources) {
        (0, _) => empty_message.to_string(),
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
