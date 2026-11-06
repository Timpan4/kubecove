//! Scripted messages and reads shared with tauri-dev-mock-live.ts.
use super::*;
use kubecove_gpui::backend::WatchTarget;
use kubecove_lib::models::{StreamMessage, StreamStatus, WatchResourceTarget};
use tokio::sync::mpsc;

#[derive(Default)]
pub struct LiveData {
    ready: Mutex<Vec<ReadyRead>>,
    returned: Mutex<Vec<Vec<ResourceSummary>>>,
    notifications: Mutex<Option<Vec<ReadyNotifications>>>,
    pub(super) state: std::sync::Arc<Mutex<ScriptState>>,
}
struct ReadyRead {
    scope: Scope,
    key: serde_json::Value,
    rows: Vec<ResourceSummary>,
}

struct ReadyNotifications {
    sender: mpsc::UnboundedSender<StreamMessage>,
    messages: Vec<StreamMessage>,
}

#[derive(Default)]
pub(super) struct ScriptState {
    failed: bool,
    burst: bool,
    changed: bool,
    deleted: bool,
    events: Option<EventSet>,
    pub(super) watches: Vec<(WatchTarget, mpsc::UnboundedSender<StreamMessage>)>,
}
#[derive(Clone, Copy)]
enum EventSet {
    Live,
    Overflow,
}
impl Fixture {
    /// Fixture construction precedes timed notification delivery. NativeBackend
    /// prepares these records on its Tokio runtime, outside the UI thread.
    #[allow(dead_code)] // Only the frame harness stages backend-side fixture work.
    pub fn prepare_live_burst(&self, index: usize) {
        // Ready reads have already run their producer work. Release the returned
        // buffers in that same unmeasured fixture-preparation phase.
        self.live.returned.lock().unwrap().clear();
        let keys = {
            let mut state = self.live.state.lock().unwrap();
            state.burst = index.is_multiple_of(2);
            state.watches.retain(|(_, sender)| !sender.is_closed());
            state
                .watches
                .iter()
                .filter_map(|(watch, sender)| match watch {
                    WatchTarget::Resource { scope, key }
                        if self
                            .resources
                            .iter()
                            .any(|row| row.kind == key.resource_kind.kind) =>
                    {
                        Some((scope.clone(), key.clone(), sender.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let mut ready = Vec::new();
        let mut notifications = Vec::new();
        for (scope, key, sender) in keys {
            let messages = self
                .resources
                .iter()
                .filter(|row| row.kind == key.resource_kind.kind)
                .map(|row| StreamMessage::ResourceChanged {
                    stream_id: "fixture".into(),
                    target: WatchResourceTarget {
                        cluster: scope.context.clone(),
                        kind: row.kind.clone(),
                        namespace: row.namespace.clone(),
                        name: Some(row.name.clone()),
                    },
                    action: "applied".into(),
                })
                .collect();
            notifications.push(ReadyNotifications { sender, messages });
            ready.push(ReadyRead {
                rows: self.kind_rows(&scope, &key),
                scope,
                key: serde_json::to_value(key).unwrap(),
            });
        }
        assert_eq!(
            notifications
                .iter()
                .map(|batch| batch.messages.len())
                .sum::<usize>(),
            self.resources.len()
        );
        let mut previous = self.live.notifications.lock().unwrap();
        assert!(
            previous.is_none(),
            "previous burst left undelivered notifications"
        );
        *previous = Some(notifications);
        let mut current = self.live.ready.lock().unwrap();
        assert!(
            current.is_empty(),
            "previous burst left unconsumed kind reads"
        );
        *current = ready;
    }

    pub(super) fn defer_kind_buffer(&self, rows: Vec<ResourceSummary>) {
        if !rows.is_empty() {
            self.live.returned.lock().unwrap().push(rows);
        }
    }

    pub(super) fn ready_kind_read(
        &self,
        scope: &Scope,
        key: &kubecove_lib::models::WatchResourceKey,
    ) -> Option<Vec<ResourceSummary>> {
        let key = serde_json::to_value(key).unwrap();
        let mut ready = self.live.ready.lock().unwrap();
        let index = ready
            .iter()
            .position(|read| read.scope == *scope && read.key == key)?;
        Some(ready.swap_remove(index).rows)
    }

    pub(super) fn kind_rows(
        &self,
        scope: &Scope,
        key: &kubecove_lib::models::WatchResourceKey,
    ) -> Vec<ResourceSummary> {
        let rows = self
            .resources
            .iter()
            .filter(|row| row.kind == key.resource_kind.kind)
            .filter(|row| {
                key.resource_kind
                    .api_version
                    .as_ref()
                    .is_none_or(|api| row.api_version.as_ref() == Some(api))
            })
            .filter(|row| {
                key.namespace
                    .as_ref()
                    .is_none_or(|ns| row.namespace.as_ref() == Some(ns))
            })
            .map(|row| ResourceSummary {
                cluster: scope.context.clone(),
                ..row.clone()
            })
            .collect();
        self.live_rows(rows)
    }

    pub(super) fn live_failed(&self) -> bool {
        self.live.state.lock().unwrap().failed
    }
    pub fn replay_live(&self, script: &str) {
        if matches!(script, "update" | "delete") {
            self.timing.invalidate("mock-dev");
        }
        let _span = kubecove_gpui::frame_profile::scope("fixture", "send");
        let mut state = self.live.state.lock().unwrap();
        match script {
            "burst" | "burst-reset" => state.burst = script == "burst",
            "failed" => state.failed = true,
            "connected" => state.failed = false,
            "update" => state.changed = true,
            "delete" => state.deleted = true,
            "events" => state.events = Some(EventSet::Live),
            "events-overflow" => state.events = Some(EventSet::Overflow),
            _ => {}
        }
        if let Some(notifications) = self.live.notifications.lock().unwrap().take() {
            assert!(matches!(script, "burst" | "burst-reset"));
            drop(state);
            for batch in notifications {
                for message in batch.messages {
                    batch.sender.send(message).unwrap();
                }
            }
            return;
        }
        state.watches.retain(|(_, sender)| !sender.is_closed());
        for (watch, sender) in &state.watches {
            if matches!(script, "burst" | "burst-reset") {
                if let WatchTarget::Resource { scope, key } = watch {
                    for row in self
                        .resources
                        .iter()
                        .filter(|row| row.kind == key.resource_kind.kind)
                    {
                        let _ = sender.send(StreamMessage::ResourceChanged {
                            stream_id: "fixture".into(),
                            target: WatchResourceTarget {
                                cluster: scope.context.clone(),
                                kind: row.kind.clone(),
                                namespace: row.namespace.clone(),
                                name: Some(row.name.clone()),
                            },
                            action: "applied".into(),
                        });
                    }
                }
                continue;
            }
            let target = WatchResourceTarget {
                cluster: "mock-dev".into(),
                kind: "Deployment".into(),
                namespace: Some("payments".into()),
                name: Some("payments-api".into()),
            };
            let message = match script {
                "connected" | "reconnecting" | "forbidden" => StreamMessage::Status {
                    stream_id: "fixture".into(),
                    status: if script == "forbidden" {
                        StreamStatus::Forbidden
                    } else if script == "connected" {
                        StreamStatus::Connected
                    } else {
                        StreamStatus::Reconnecting
                    },
                    message: script.into(),
                },
                "events" | "events-overflow" if matches!(watch, WatchTarget::Events(r) if r.kind == "Deployment" && r.name == "payments-api") => {
                    StreamMessage::ResourceEventsChanged {
                        stream_id: "fixture".into(),
                        target,
                        action: "applied".into(),
                    }
                }
                "update" | "delete" | "failed" if matches!(watch, WatchTarget::Resource { key, .. } if key.resource_kind.kind == "Deployment") => {
                    StreamMessage::ResourceChanged {
                        stream_id: "fixture".into(),
                        target,
                        action: script.into(),
                    }
                }
                _ => continue,
            };
            let _ = sender.send(message);
            if script == "failed" {
                let _ = sender.send(StreamMessage::Status {
                    stream_id: "fixture".into(),
                    status: StreamStatus::Reconnecting,
                    message: "Scripted connection loss".into(),
                });
            }
        }
    }
    pub(super) fn live_watch(&self, target: WatchTarget) -> Read<LogStream> {
        let context = match &target {
            WatchTarget::Resource { scope, .. } => scope.context.clone(),
            WatchTarget::Events(resource) => resource.cluster.clone(),
        };
        let opening = self.timing.read("start_resource_watch", &context, "", ());
        let live = self.live.state.clone();
        Box::pin(async move {
            opening.await?;
            let (sender, receiver) = mpsc::unbounded_channel();
            sender
                .send(StreamMessage::Status {
                    stream_id: "fixture".into(),
                    status: StreamStatus::Connected,
                    message: "Live updates connected".into(),
                })
                .unwrap();
            let mut state = live.lock().unwrap();
            // The real stream opens at resourceVersion "0" in streams/watch.rs.
            if (state.changed || state.deleted)
                && matches!(&target, WatchTarget::Resource { key, .. } if key.resource_kind.kind == "Deployment")
            {
                let _ = sender.send(StreamMessage::ResourceChanged {
                    stream_id: "fixture".into(),
                    target: WatchResourceTarget {
                        cluster: context,
                        kind: "Deployment".into(),
                        namespace: Some("payments".into()),
                        name: Some("payments-api".into()),
                    },
                    action: if state.deleted { "deleted" } else { "applied" }.into(),
                });
            }
            state.watches.push((target, sender.clone()));
            Ok(LogStream::new(receiver, sender))
        })
    }
    pub(super) fn live_rows(&self, rows: Vec<ResourceSummary>) -> Vec<ResourceSummary> {
        let state = self.live.state.lock().unwrap();
        rows.into_iter()
            .filter_map(|mut row| {
                if state.burst {
                    row.health_assessment.state =
                        kubecove_lib::models::HealthAssessmentState::Degraded;
                }
                if row.kind == "Deployment"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == "payments-api"
                {
                    if state.deleted {
                        return None;
                    }
                    if state.changed {
                        row.status = Some("Progressing".into());
                        row.health_assessment.state =
                            kubecove_lib::models::HealthAssessmentState::Degraded;
                    }
                }
                Some(row)
            })
            .collect()
    }
    pub(super) fn scripted_events(
        &self,
        resource: &ResourceSummary,
    ) -> Option<Vec<ResourceEventSummary>> {
        if resource.kind != "Deployment" || resource.name != "payments-api" {
            return None;
        }
        match self.live.state.lock().unwrap().events {
            Some(EventSet::Live) => Some(self.live_events.clone()),
            Some(EventSet::Overflow) => Some(self.overflow_events.clone()),
            None => None,
        }
    }
}
