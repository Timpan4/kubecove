//! Scripted messages and reads shared with tauri-dev-mock-live.ts.
use super::*;
use kubecove_gpui::backend::{ListVersions, WatchTarget, discovered_key, watch_signature};
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
    /// A scripted rollout gives payments-api the "Rolling out" lifecycle, which shows activity.
    rolling: bool,
    deleted: bool,
    /// The "add" script lists a copy of payments-api named `ADDED`.
    added: bool,
    /// The "ownership" script moves checkout-worker to a Flux owner. Its ownerRef-only Pod follows it.
    reowned: bool,
    events: Option<EventSet>,
    /// Scripted changes so far. The mock list version is one higher.
    revision: u64,
    /// The list version that first shows the scripted change.
    changed_at: Option<u64>,
    /// The kind and start version of each resource watch, in opening order.
    starts: Vec<(String, Option<String>)>,
    pub(super) watches: Vec<(WatchTarget, mpsc::UnboundedSender<StreamMessage>)>,
}
/// The Deployment the "ownership" script moves to another GitOps owner.
const REOWNED: &str = "checkout-worker";
/// The copy of payments-api the "add" script creates. Its name sorts between
/// the payments Deployments.
const ADDED: &str = "orders-api";

/// The Deployment the update, rollout and delete scripts change.
fn scripted_target(row: &ResourceSummary) -> bool {
    row.kind == "Deployment"
        && row.namespace.as_deref() == Some("payments")
        && row.name == "payments-api"
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
                    WatchTarget::Resource { scope, key, .. }
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

    /// `kind_resources`: a burst's prepared read, or the kind's rows now.
    pub(super) fn kind_read(
        &self,
        scope: Scope,
        key: kubecove_lib::models::WatchResourceKey,
        mut recycled: Vec<ResourceSummary>,
    ) -> Read<kubecove_gpui::backend::KindRows> {
        let _span = kubecove_gpui::frame_profile::scope("fixture", "kind_read");
        let resource_version = Some(self.list_version());
        if self.live_failed() {
            return Box::pin(async move {
                drop(recycled);
                Err(AppError::new(
                    "Scripted reload failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        let rows = if let Some(rows) = self.ready_kind_read(&scope, &key) {
            self.defer_kind_buffer(recycled);
            rows
        } else {
            let rows = self.kind_rows(&scope, &key);
            recycled.clear();
            recycled.extend(rows);
            recycled
        };
        Box::pin(async move {
            Ok(kubecove_gpui::backend::KindRows {
                rows,
                resource_version,
            })
        })
    }

    /// `named_kind_resources`. A prepared burst names every row of its ready
    /// read, so that read is the named read, filtered outside the measured
    /// frame as NativeBackend filters named reads on its runtime.
    pub(super) fn named_kind_read(
        &self,
        scope: Scope,
        key: kubecove_lib::models::WatchResourceKey,
        names: kubecove_gpui::backend::WatchNames,
        recycled: Vec<ResourceSummary>,
    ) -> Read<kubecove_gpui::backend::KindRows> {
        let wanted = serde_json::to_value(&key).unwrap();
        let prepared = (self.live.ready.lock().unwrap().iter())
            .any(|read| read.scope == scope && read.key == wanted);
        let read = self.kind_read(scope, key, recycled);
        if prepared {
            return read;
        }
        kubecove_gpui::backend::named_rows(read, names)
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

    /// The mock list version, shared with tauri-dev-mock-live.ts. A scripted
    /// change raises it, as a cluster write does.
    pub(super) fn list_version(&self) -> String {
        (self.live.state.lock().unwrap().revision + 1).to_string()
    }

    /// The current list version under the watch signature of every kind a
    /// view can watch in `namespace`.
    pub(super) fn list_versions(&self, namespace: Option<&str>) -> ListVersions {
        let version = self.list_version();
        let mut versions = ListVersions::new();
        for row in &self.resources {
            let discovered = self.kinds.iter().find(|kind| {
                kind.kind == row.kind && row.api_version.as_ref() == Some(&kind.api_version)
            });
            let key = match discovered {
                Some(kind) => discovered_key(kind, namespace.map(str::to_owned)),
                None => {
                    let namespaced = !CLUSTER_KINDS.contains(&row.kind.as_str());
                    kubecove_lib::models::WatchResourceKey {
                        resource_kind: kubecove_lib::models::WatchResourceKind {
                            kind: row.kind.clone(),
                            group: None,
                            version: None,
                            api_version: None,
                            plural: None,
                            namespaced: Some(namespaced),
                        },
                        namespace: namespace.filter(|_| namespaced).map(str::to_owned),
                    }
                }
            };
            versions.insert(watch_signature(&key), version.clone());
        }
        versions
    }

    /// Asserts the version the newest `kind` watch started at.
    #[allow(dead_code)] // Only the parity driver checks watch starts.
    pub fn expect_watch_start(&self, kind: &str, resource_version: Option<&str>) {
        let state = self.live.state.lock().unwrap();
        let start = state
            .starts
            .iter()
            .rev()
            .find(|(started, _)| started == kind)
            .unwrap_or_else(|| panic!("no {kind} watch started"));
        assert_eq!(
            start.1.as_deref(),
            resource_version,
            "{kind} watch start version"
        );
    }
    pub fn replay_live(&self, script: &str) {
        if matches!(
            script,
            "update" | "rollout" | "delete" | "add" | "ownership" | "missed-delete"
        ) {
            self.timing.invalidate("mock-dev");
        }
        let _span = kubecove_gpui::frame_profile::scope("fixture", "send");
        let mut state = self.live.state.lock().unwrap();
        match script {
            "burst" | "burst-reset" => state.burst = script == "burst",
            "failed" => state.failed = true,
            "connected" => state.failed = false,
            // Deletes payments-api without a notification, as an event a lost watch never delivered.
            "missed-delete" => {
                state.deleted = true;
                state.revision += 1;
                state.changed_at = Some(state.revision + 1);
                return;
            }
            "update" | "rollout" | "delete" | "add" | "ownership" => {
                state.changed |= script == "update";
                state.rolling |= script == "rollout";
                state.deleted |= script == "delete";
                state.added |= script == "add";
                state.reowned |= script == "ownership";
                state.revision += 1;
                state.changed_at = Some(state.revision + 1);
            }
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
                if let WatchTarget::Resource { scope, key, .. } = watch {
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
                name: Some(
                    match script {
                        "ownership" => REOWNED,
                        "add" => ADDED,
                        _ => "payments-api",
                    }
                    .into(),
                ),
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
                "update" | "rollout" | "delete" | "add" | "ownership" | "failed" if matches!(watch, WatchTarget::Resource { key, .. } if key.resource_kind.kind == "Deployment") => {
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
            let start = match &target {
                WatchTarget::Resource {
                    key,
                    resource_version,
                    ..
                } => Some((key.resource_kind.kind.clone(), resource_version.clone())),
                WatchTarget::Events(_) => None,
            };
            // streams/watch.rs starts at the given list version, or at "0",
            // which replays every object. Either way it delivers a change made
            // after that version.
            let replays = |version: &Option<String>| {
                state.changed_at.is_some_and(|changed| {
                    version
                        .as_deref()
                        .and_then(|version| version.parse::<u64>().ok())
                        .is_none_or(|version| version < changed)
                })
            };
            if let Some((kind, version)) = &start
                && kind == "Deployment"
                && replays(version)
            {
                let changed = [
                    (state.changed || state.rolling || state.deleted).then_some((
                        "payments-api",
                        if state.deleted { "deleted" } else { "applied" },
                    )),
                    state.reowned.then_some((REOWNED, "applied")),
                ];
                for (name, action) in changed.into_iter().flatten() {
                    let _ = sender.send(StreamMessage::ResourceChanged {
                        stream_id: "fixture".into(),
                        target: WatchResourceTarget {
                            cluster: context.clone(),
                            kind: "Deployment".into(),
                            namespace: Some("payments".into()),
                            name: Some(name.into()),
                        },
                        action: action.into(),
                    });
                }
            }
            state.starts.extend(start);
            state.watches.push((target, sender.clone()));
            Ok(LogStream::new(receiver, sender))
        })
    }
    pub(super) fn live_rows(&self, rows: Vec<ResourceSummary>) -> Vec<ResourceSummary> {
        let state = self.live.state.lock().unwrap();
        let added = state
            .added
            .then(|| rows.iter().find(|row| scripted_target(row)).cloned())
            .flatten()
            .map(|row| ResourceSummary {
                name: ADDED.into(),
                ..row
            });
        rows.into_iter()
            .filter_map(|mut row| {
                if state.burst {
                    row.health_assessment.state =
                        kubecove_lib::models::HealthAssessmentState::Degraded;
                }
                if state.reowned
                    && row.kind == "Deployment"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == REOWNED
                {
                    row.git_ops_owner = Some(kubecove_lib::models::GitOpsOwnerSummary {
                        provider: "flux".into(),
                        kind: "Kustomization".into(),
                        name: "platform".into(),
                        namespace: Some("flux-system".into()),
                        confidence: "inventory".into(),
                        provenance: "flux-kustomization-inventory".into(),
                        partial: false,
                    });
                }
                if scripted_target(&row) {
                    if state.deleted {
                        return None;
                    }
                    if state.rolling {
                        row.status = Some("Rolling out".into());
                    } else if state.changed {
                        row.status = Some("Progressing".into());
                        row.health_assessment.state =
                            kubecove_lib::models::HealthAssessmentState::Degraded;
                    }
                }
                Some(row)
            })
            .chain(added)
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
