//! Scripted messages and reads shared with tauri-dev-mock-live.ts.
use super::*;
use kubecove_gpui::backend::WatchTarget;
use kubecove_lib::models::{StreamMessage, StreamStatus, WatchResourceTarget};
use tokio::sync::mpsc;

#[derive(Default)]
pub struct LiveData {
    state: Mutex<ScriptState>,
}
#[derive(Default)]
struct ScriptState {
    failed: bool,
    burst: bool,
    changed: bool,
    deleted: bool,
    events: bool,
    watches: Vec<(WatchTarget, mpsc::UnboundedSender<StreamMessage>)>,
}
impl Fixture {
    pub(super) fn live_failed(&self) -> bool {
        self.live.state.lock().unwrap().failed
    }
    pub fn replay_live(&self, script: &str) {
        let mut state = self.live.state.lock().unwrap();
        match script {
            "burst" => state.burst = true,
            "failed" => state.failed = true,
            "connected" => state.failed = false,
            "update" => state.changed = true,
            "delete" => state.deleted = true,
            "events" => state.events = true,
            _ => {}
        }
        state.watches.retain(|(_, sender)| !sender.is_closed());
        for (watch, sender) in &state.watches {
            if script == "burst" {
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
                "events" if matches!(watch, WatchTarget::Events(r) if r.kind == "Deployment" && r.name == "payments-api") => {
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
        let (sender, receiver) = mpsc::unbounded_channel();
        sender
            .send(StreamMessage::Status {
                stream_id: "fixture".into(),
                status: StreamStatus::Connected,
                message: "Live updates connected".into(),
            })
            .unwrap();
        self.live
            .state
            .lock()
            .unwrap()
            .watches
            .push((target, sender.clone()));
        Box::pin(async move { Ok(LogStream::new(receiver, sender)) })
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
        (self.live.state.lock().unwrap().events
            && resource.kind == "Deployment"
            && resource.name == "payments-api")
            .then(|| self.live_events.clone())
    }
}
