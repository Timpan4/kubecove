//! View-owned streams. Recovery follows resource-watch.ts; Rust owns transport backoff.
use super::{Inspector, Page};
use crate::backend::{LogStream, WatchTarget};
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{
    DiscoveredResourceKind, StreamMessage, StreamStatus, WatchResourceKey, WatchResourceKind,
};
use std::{
    collections::HashMap,
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
    time::Duration,
};

/// resource-watch.ts::RESOURCE_RECOVERY_INTERVAL.
pub(super) const RECOVERY: Duration = Duration::from_millis(15_000);
/// live-view-refresh.ts::createWatchRefreshThrottle.
pub const COALESCE: Duration = Duration::from_millis(250);

#[derive(Default)]
pub(super) struct LiveData {
    owner: String,
    pub kinds: Vec<DiscoveredResourceKind>,
    pub custom_open: bool,
    pub groups: std::collections::HashSet<String>,
    pub picker_open: bool,
    pub selected: Option<DiscoveredResourceKind>,
    pub kind_locked: bool,
    pub refreshing: bool,
    pub scope_loading: bool,
    pub refresh_error: Option<String>,
    pub error_open: bool,
    discovery: Option<Task<()>>,
    watches: HashMap<String, Registration>,
    pending: Vec<super::live_rows::Patch>,
}

struct Registration {
    seen_status: bool,
    connected: bool,
    forbidden: Option<String>,
    error: Option<String>,
    reload_error: Option<String>,
    catching_up: bool,
    recycled: Vec<kubecove_lib::models::ResourceSummary>,
    _task: Task<()>,
}

impl LiveData {
    fn take_buffer(&mut self, id: &str) -> Vec<kubecove_lib::models::ResourceSummary> {
        self.watches
            .get_mut(id)
            .map(|watch| std::mem::take(&mut watch.recycled))
            .unwrap_or_default()
    }
    pub fn reset_scope(&mut self) {
        self.watches.clear();
        self.pending.clear();
        self.owner.clear();
        self.refreshing = false;
        self.refresh_error = None;
    }
    pub fn label(&self) -> &'static str {
        if self.refreshing {
            return "Refreshing…";
        }
        if self.refresh_error.is_some() {
            return "Refresh failed";
        }
        if !self.watches.is_empty()
            && self
                .watches
                .values()
                .all(|w| !w.seen_status && w.error.is_none())
        {
            return "Connecting";
        }
        if self.watches.values().any(|w| w.reload_error.is_some()) {
            "Refresh failed"
        } else if self
            .watches
            .values()
            .any(|w| w.error.is_some() || (!w.connected && w.forbidden.is_none()))
        {
            "Reconnecting"
        } else if self.watches.values().any(|w| w.catching_up) {
            "Catching up"
        } else if !self.watches.is_empty() && self.watches.values().all(|w| w.forbidden.is_some()) {
            "Polling (limited)"
        } else if self.watches.values().any(|w| w.forbidden.is_some()) {
            "Live (limited)"
        } else if self.watches.is_empty() {
            "Not connected"
        } else {
            "Live"
        }
    }

    /// Whether row activity may animate: every watch is live and no read is pending.
    /// Mirrors `updatesCurrent` in `ResourceBrowser.svelte`.
    pub fn updates_current(&self) -> bool {
        !self.scope_loading && self.label() == "Live"
    }

    pub fn messages(&self) -> impl Iterator<Item = &String> {
        self.refresh_error
            .iter()
            .chain(self.watches.values().flat_map(|w| {
                [&w.error, &w.forbidden, &w.reload_error]
                    .into_iter()
                    .flatten()
            }))
    }
}

pub(super) fn key(kind: &str, namespace: Option<String>) -> WatchResourceKey {
    let namespaced = !crate::backend::CLUSTER_KINDS.contains(&kind);
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.into(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: Some(namespaced),
        },
        namespace: if namespaced { namespace } else { None },
    }
}

pub(super) fn dynamic_key(
    kind: &DiscoveredResourceKind,
    namespace: Option<String>,
) -> WatchResourceKey {
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.kind.clone(),
            group: Some(kind.group.clone()),
            version: Some(kind.version.clone()),
            api_version: Some(kind.api_version.clone()),
            plural: Some(kind.plural.clone()),
            namespaced: Some(kind.namespaced),
        },
        namespace: if kind.namespaced { namespace } else { None },
    }
}

pub(super) enum WatchInput {
    Message(StreamMessage),
    Tick,
    Closed,
}

pub(super) async fn receive_or_tick(
    stream: &mut LogStream,
    timer: &mut Pin<Box<impl Future<Output = ()>>>,
) -> WatchInput {
    let mut receive = Box::pin(stream.messages.recv());
    poll_fn(|cx| {
        if let Poll::Ready(message) = receive.as_mut().poll(cx) {
            return Poll::Ready(message.map_or(WatchInput::Closed, WatchInput::Message));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(WatchInput::Tick);
        }
        Poll::Pending
    })
    .await
}

impl Inspector {
    /// Custom kinds present in the selected namespace, or cluster-wide with no
    /// namespace, as the web's `listPresentCustomResourceKinds` returns them.
    pub(super) fn discover_kinds(&mut self, cx: &mut Context<Self>) {
        self.live.kinds.clear();
        let scope = self.scope.clone();
        let context = scope.context.clone();
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        self.live.discovery = Some(cx.spawn(async move |this, cx| {
            let result = crate::read_retry::cancelled_read(
                || {
                    timings.measure(
                        "list_present_custom_resource_kinds",
                        backend.namespace_custom_kinds(scope.clone()),
                    )
                },
                |delay| cx.background_executor().timer(delay),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.scope.context != context {
                    return;
                }
                match result {
                    Ok(kinds) => this.live.kinds = kinds,
                    Err(error) => this
                        .warnings
                        .push(format!("Custom Resources: {}", error.message)),
                }
                cx.notify();
            });
        }));
    }

    /// Reconciles before rendering: changed scopes drop their tasks and stream guards.
    pub(super) fn sync_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_gitops_watches(window, cx);
        let owner = format!(
            "{:?}:{:?}:{}:{:?}:{:?}:{:?}",
            self.page as u8,
            self.launcher.active,
            self.scope.context,
            self.scope.namespace,
            self.live
                .selected
                .as_ref()
                .map(|kind| (&kind.api_version, &kind.kind)),
            self.kind_selection.chosen.as_ref().map(|kinds| {
                let mut kinds: Vec<_> = kinds.iter().collect();
                kinds.sort();
                kinds
            })
        );
        if owner != self.live.owner {
            self.live.watches.clear();
            self.live.pending.clear();
            self.live.owner = owner;
        }
        if !self.workspace_open()
            || self.scope.context.is_empty()
            || !matches!(
                self.page,
                Page::Resources | Page::ClusterOverview | Page::Overview
            )
        {
            return;
        }
        let mut keys: Vec<_> = self
            .available_kind_keys()
            .into_iter()
            .filter(|(id, _, _)| self.kind_selected(id))
            .map(|(_, _, key)| key)
            .collect();
        if let Some(kind) = &self.live.selected {
            keys = vec![dynamic_key(kind, self.scope.namespace.clone())];
        }
        for key in keys {
            let signature = format!(
                "{}:{}:{}:{}",
                key.resource_kind.kind,
                key.resource_kind.api_version.as_deref().unwrap_or(""),
                key.resource_kind.plural.as_deref().unwrap_or(""),
                key.namespace.as_deref().unwrap_or("")
            );
            if self.live.watches.contains_key(&signature) {
                continue;
            }
            let scope = self.scope.clone();
            let backend = self.backend.clone();
            let owner = self.live.owner.clone();
            let id = signature.clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let mut lost = false;
                let mut timer = Box::pin(cx.background_executor().timer(RECOVERY));
                loop {
                    let mut stream = match backend
                        .watch(WatchTarget::Resource {
                            scope: scope.clone(),
                            key: key.clone(),
                        })
                        .await
                    {
                        Ok(stream) => stream,
                        Err(error) => {
                            lost = true;
                            let _ = this.update(cx, |this, cx| {
                                if this.live.owner == owner
                                    && let Some(w) = this.live.watches.get_mut(&id)
                                {
                                    w.error = Some(error.message);
                                    w.connected = false;
                                }
                                cx.notify();
                            });
                            timer.as_mut().await;
                            timer = Box::pin(cx.background_executor().timer(RECOVERY));
                            let recycled = this
                                .update(cx, |this, _| this.live.take_buffer(&id))
                                .unwrap_or_default();
                            let mut recycled = Some(recycled);
                            let retry_backend = backend.clone();
                            let retry_scope = scope.clone();
                            let retry_key = key.clone();
                            let rows = crate::read_retry::read(
                                cx.background_executor().clone(),
                                move || {
                                    retry_backend.kind_resources(
                                        retry_scope.clone(),
                                        retry_key.clone(),
                                        recycled.take().unwrap_or_default(),
                                    )
                                },
                            )
                            .await;
                            let valid = this
                                .update(cx, |this, cx| {
                                    if this.live.owner != owner {
                                        return false;
                                    }
                                    match rows {
                                        Ok(rows) => {
                                            this.supersede_kind_read(&key);
                                            this.queue_live_patch(
                                                super::live_rows::Patch {
                                                    scope: scope.clone(),
                                                    key: key.clone(),
                                                    rows,
                                                    names: None,
                                                    watch: Some(id.clone()),
                                                },
                                                cx,
                                            );
                                        }
                                        Err(error) => {
                                            if let Some(w) = this.live.watches.get_mut(&id) {
                                                w.reload_error = Some(error.message);
                                            }
                                        }
                                    }
                                    true
                                })
                                .unwrap_or(false);
                            if !valid {
                                return;
                            }
                            continue;
                        }
                    };
                    let mut forbidden = false;
                    let mut read_failed = false;
                    loop {
                        let input = receive_or_tick(&mut stream, &mut timer).await;
                        let tick = matches!(input, WatchInput::Tick);
                        let batch_profile = crate::frame_profile::scope("watch", "batch");
                        let closed = matches!(input, WatchInput::Closed);
                        if tick {
                            timer = Box::pin(cx.background_executor().timer(RECOVERY));
                        }
                        let mut batch = match input {
                            WatchInput::Message(message) => vec![message],
                            WatchInput::Closed | WatchInput::Tick => Vec::new(),
                        };
                        while let Ok(message) = stream.messages.try_recv() {
                            batch.push(message);
                        }
                        let ended = batch
                            .iter()
                            .any(|event| matches!(event, StreamMessage::Stopped { .. }))
                            || closed;
                        if closed {
                            lost = true;
                        }
                        let mut reload = tick && (lost || forbidden || read_failed);
                        let mut names = Some(crate::model::ObservedNames::default());
                        let mut transitions = Vec::new();
                        for event in batch {
                            match event {
                                StreamMessage::Status { status, .. } => {
                                    forbidden = status == StreamStatus::Forbidden;
                                    if status != StreamStatus::Connected {
                                        lost = true;
                                    } else if lost {
                                        reload = true;
                                        names = None;
                                        lost = false;
                                    }
                                    transitions.push(event);
                                }
                                StreamMessage::Error { .. } | StreamMessage::Stopped { .. } => {
                                    lost = true;
                                    transitions.push(event);
                                }
                                StreamMessage::ResourceChanged { target, .. } => {
                                    reload = true;
                                    if let (Some(names), Some(name)) = (&mut names, target.name) {
                                        names.insert((target.namespace, name));
                                    } else {
                                        names = None;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if tick {
                            names = None;
                        }
                        drop(batch_profile);
                        let valid = this
                            .update_in(cx, |this, _window, cx| {
                                if this.live.owner != owner {
                                    return false;
                                }
                                let Some(w) = this.live.watches.get_mut(&id) else {
                                    return false;
                                };
                                if closed {
                                    w.connected = false;
                                }
                                for event in transitions {
                                    match event {
                                        StreamMessage::Status {
                                            status, message, ..
                                        } => {
                                            w.seen_status = true;
                                            w.connected = status == StreamStatus::Connected;
                                            w.forbidden = (status == StreamStatus::Forbidden)
                                                .then_some(message);
                                            if w.connected || w.forbidden.is_some() {
                                                w.error = None;
                                            }
                                        }
                                        StreamMessage::Error { message, .. } => {
                                            w.error = Some(message);
                                            w.connected = false;
                                        }
                                        StreamMessage::Stopped { .. } => w.connected = false,
                                        _ => {}
                                    }
                                }
                                w.catching_up = reload && names.is_none();
                                if names.is_none() || !reload {
                                    cx.notify();
                                }
                                true
                            })
                            .unwrap_or(false);
                        if !valid {
                            return;
                        }
                        if reload {
                            let started = std::time::Instant::now();
                            let recycled = this
                                .update(cx, |this, _| this.live.take_buffer(&id))
                                .unwrap_or_default();
                            let mut recycled = Some(recycled);
                            let retry_backend = backend.clone();
                            let retry_scope = scope.clone();
                            let retry_key = key.clone();
                            let rows = crate::read_retry::read(
                                cx.background_executor().clone(),
                                move || {
                                    retry_backend.kind_resources(
                                        retry_scope.clone(),
                                        retry_key.clone(),
                                        recycled.take().unwrap_or_default(),
                                    )
                                },
                            )
                            .await;
                            read_failed = rows.is_err();
                            let valid = this
                                .update_in(cx, |this, _window, cx| {
                                    if this.live.owner != owner {
                                        return false;
                                    }
                                    match rows {
                                        Ok(rows) => {
                                            this.supersede_kind_read(&key);
                                            this.queue_live_patch(
                                                super::live_rows::Patch {
                                                    scope: scope.clone(),
                                                    key: key.clone(),
                                                    rows,
                                                    names,
                                                    watch: Some(id.clone()),
                                                },
                                                cx,
                                            );
                                            if let Some(w) = this.live.watches.get_mut(&id) {
                                                w.reload_error = None;
                                                w.catching_up = false;
                                            }
                                        }
                                        Err(error) => {
                                            if let Some(w) = this.live.watches.get_mut(&id) {
                                                w.reload_error = Some(error.message);
                                                w.catching_up = false;
                                            }
                                        }
                                    }
                                    cx.notify();
                                    true
                                })
                                .unwrap_or(false);
                            if !valid {
                                return;
                            }
                            if !tick {
                                cx.background_executor()
                                    .timer(COALESCE.saturating_sub(started.elapsed()))
                                    .await;
                            }
                        }
                        if ended {
                            if forbidden {
                                stream = LogStream::replay([]);
                            } else {
                                break;
                            }
                        }
                    }
                    drop(stream);
                    timer.as_mut().await;
                    timer = Box::pin(cx.background_executor().timer(RECOVERY));
                }
            });
            self.live.watches.insert(
                signature,
                Registration {
                    seen_status: false,
                    connected: false,
                    forbidden: None,
                    error: None,
                    reload_error: None,
                    catching_up: false,
                    recycled: Vec::new(),
                    _task: task,
                },
            );
        }
    }

    pub(super) fn queue_live_patch(
        &mut self,
        patch: super::live_rows::Patch,
        cx: &mut Context<Self>,
    ) {
        self.live.pending.push(patch);
        cx.notify();
    }

    pub(super) fn flush_live_patches(&mut self, cx: &mut Context<Self>) {
        let _span = crate::frame_profile::scope("watch", "flush");
        if self.live.scope_loading {
            return;
        }
        let mut pending = std::mem::take(&mut self.live.pending);
        pending.retain(|patch| patch.scope == self.scope);
        if pending.is_empty() {
            return;
        }
        self.visible.release_source();
        self.row_index.invalidate();
        for patch in pending {
            let rows = self.patch_kind(&patch.scope, &patch.key, patch.rows, patch.names);
            if let Some(watch) = patch
                .watch
                .as_deref()
                .and_then(|id| self.live.watches.get_mut(id))
            {
                // The sequential read consumes this buffer on its next producer turn.
                watch.recycled = rows;
            }
        }
        self.prepare_rows();
        self.filter(cx);
        cx.notify();
    }
}
