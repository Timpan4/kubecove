//! Native reads and guarded operations for the resource detail tabs: log
//! streams, Deployment revisions, selected-resource actions (ADR 0004 and
//! ADR 0024) and YAML apply. Each calls the shared typed command.

use super::NativeBackend;
use kubecove_lib::{
    commands::{self, StreamRegistry},
    models::{AggregatedLogStreamRequest, PodLogStreamRequest, StreamMessage},
    runtime::{Channel, ChannelClosed, State},
};
use std::sync::Arc;
use tokio::sync::mpsc;

/// The log stream the Logs tab requests: one Pod container, or every Pod
/// behind a Deployment or selector-backed Service.
#[derive(Clone, Debug)]
pub enum LogTarget {
    Pod(PodLogStreamRequest),
    Aggregate(AggregatedLogStreamRequest),
}

/// A running command stream. Dropping it stops the backend stream.
pub struct LogStream {
    pub messages: mpsc::UnboundedReceiver<StreamMessage>,
    _stop: Box<dyn Send>,
}

impl LogStream {
    /// `stop` runs its `Drop` when the stream is dropped.
    pub fn new(
        messages: mpsc::UnboundedReceiver<StreamMessage>,
        stop: impl Send + 'static,
    ) -> Self {
        Self {
            messages,
            _stop: Box::new(stop),
        }
    }

    /// A controlled stream that delivers `messages` and stays open until dropped.
    pub fn replay(messages: impl IntoIterator<Item = StreamMessage>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        for message in messages {
            let _ = sender.send(message);
        }
        Self::new(receiver, sender)
    }
}

/// Stops a native stream by id when the frontend drops it.
pub(super) struct StopStream {
    pub(super) id: String,
    pub(super) registry: Arc<StreamRegistry>,
    pub(super) runtime: tokio::runtime::Handle,
}

impl Drop for StopStream {
    fn drop(&mut self) {
        let (id, registry) = (std::mem::take(&mut self.id), self.registry.clone());
        self.runtime.spawn(async move {
            let _ = commands::stop_stream(id, State::new(&registry)).await;
        });
    }
}

pub(super) fn log_stream(backend: &NativeBackend, target: LogTarget) -> super::Read<LogStream> {
    let registry = backend.streams.clone();
    let runtime = backend
        .runtime
        .as_ref()
        .expect("runtime exists until drop")
        .handle()
        .clone();
    backend.read(async move {
        let (sender, messages) = mpsc::unbounded_channel();
        let channel = Channel::new(move |message| sender.send(message).map_err(|_| ChannelClosed));
        let id = match target {
            LogTarget::Pod(request) => {
                commands::start_pod_log_stream(request, channel, State::new(&registry)).await?
            }
            LogTarget::Aggregate(request) => {
                commands::start_aggregated_log_stream(request, channel, State::new(&registry))
                    .await?
            }
        };
        Ok(LogStream::new(
            messages,
            StopStream {
                id,
                registry,
                runtime,
            },
        ))
    })
}

pub(super) fn resource_yaml(
    backend: &NativeBackend,
    resource: kubecove_lib::models::ResourceSummary,
    mode: kubecove_lib::models::YamlViewMode,
    encoding: kubecove_lib::models::YamlEncoding,
) -> super::Read<String> {
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        if let Some(kind) =
            super::delivery::dynamic_kind(&resource).filter(|_| resource.dynamic == Some(true))
        {
            return commands::get_dynamic_resource_details(
                resource.cluster,
                kind,
                resource.name,
                resource.namespace,
                None,
                Some(mode),
                Some(encoding),
                None,
                None,
                State::new(&cancellations),
            )
            .await
            .map(|details| details.yaml);
        }
        super::supported(&resource.kind)?;
        commands::get_resource_yaml(
            resource.cluster,
            resource.kind,
            resource.name,
            resource.namespace,
            None,
            Some(mode),
            Some(encoding),
            None,
            None,
            State::new(&cancellations),
        )
        .await
    })
}
