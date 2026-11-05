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
pub(super) const COALESCE: Duration = Duration::from_millis(250);

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
    _task: Task<()>,
}

impl LiveData {
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
    pub(super) fn discover_kinds(&mut self, cx: &mut Context<Self>) {
        self.live.kinds.clear();
        if !self.persisted.preferences.show_custom_resources {
            return;
        }
        self.live.selected = None;
        let context = self.scope.context.clone();
        let read = self.backend.resource_kinds(context.clone());
        let read = self.timings.measure("resource_kinds", read);
        self.live.discovery = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
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
        let owner = format!(
            "{:?}:{}:{:?}:{:?}",
            self.page as u8,
            self.scope.context,
            self.scope.namespace,
            self.live
                .selected
                .as_ref()
                .map(|kind| (&kind.api_version, &kind.kind))
        );
        if owner != self.live.owner {
            self.live.watches.clear();
            self.live.pending.clear();
            self.live.owner = owner;
        }
        if self.scope.context.is_empty()
            || !matches!(
                self.page,
                Page::Resources | Page::ClusterOverview | Page::Overview
            )
        {
            return;
        }
        let mut keys: Vec<_> = self
            .base_kinds()
            .iter()
            .map(|kind| key(kind, self.scope.namespace.clone()))
            .collect();
        keys.extend(
            self.live
                .kinds
                .iter()
                .map(|kind| dynamic_key(kind, self.scope.namespace.clone())),
        );
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
                            let rows = backend.kind_resources(scope.clone(), key.clone()).await;
                            let valid = this
                                .update(cx, |this, cx| {
                                    if this.live.owner != owner {
                                        return false;
                                    }
                                    match rows {
                                        Ok(rows) => this.patch_kind(&scope, &key, rows, None, cx),
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
                        let mut names = Some(std::collections::HashSet::new());
                        for event in &batch {
                            match event {
                                StreamMessage::Status { status, .. } => {
                                    forbidden = *status == StreamStatus::Forbidden;
                                    if *status != StreamStatus::Connected {
                                        lost = true;
                                    } else if lost {
                                        reload = true;
                                        names = None;
                                        lost = false;
                                    }
                                }
                                StreamMessage::Error { .. } | StreamMessage::Stopped { .. } => {
                                    lost = true
                                }
                                StreamMessage::ResourceChanged { target, .. } => {
                                    reload = true;
                                    if let (Some(names), Some(name)) = (&mut names, &target.name) {
                                        names.insert((target.namespace.clone(), name.clone()));
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
                        let valid = this
                            .update_in(cx, |this, window, cx| {
                                if this.live.owner != owner {
                                    return false;
                                }
                                let Some(w) = this.live.watches.get_mut(&id) else {
                                    return false;
                                };
                                if closed {
                                    w.connected = false;
                                }
                                for event in batch {
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
                                if tick
                                    && this.live.refresh_error.is_some()
                                    && !this.live.refreshing
                                {
                                    this.refresh(window, cx);
                                }
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
                            let rows = backend.kind_resources(scope.clone(), key.clone()).await;
                            read_failed = rows.is_err();
                            let valid = this
                                .update_in(cx, |this, window, cx| {
                                    if this.live.owner != owner {
                                        return false;
                                    }
                                    match rows {
                                        Ok(rows) => {
                                            this.queue_live_patch(
                                                super::live_rows::Patch {
                                                    scope: scope.clone(),
                                                    key: key.clone(),
                                                    rows,
                                                    names,
                                                },
                                                window,
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
                    _task: task,
                },
            );
        }
    }

    fn queue_live_patch(
        &mut self,
        patch: super::live_rows::Patch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.live.pending.push(patch);
        cx.notify();
    }

    pub(super) fn flush_live_patches(&mut self, cx: &mut Context<Self>) {
        if self.live.scope_loading {
            return;
        }
        for patch in std::mem::take(&mut self.live.pending) {
            self.patch_kind(
                &patch.scope,
                &patch.key,
                patch.rows,
                patch.names.as_ref(),
                cx,
            );
        }
    }
}
