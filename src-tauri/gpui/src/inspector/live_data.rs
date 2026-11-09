//! View-owned streams. Recovery follows resource-watch.ts; Rust owns transport backoff.
use super::{
    Inspector, Page,
    request::InFlight,
    watch_status::{self, Link, LiveStatus, WatchLink, WatchSummary},
};
use crate::backend::{KindRows, LogStream, WatchTarget};
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
    /// Custom kinds present in the selected namespace, for the sidebar.
    pub kinds: Vec<DiscoveredResourceKind>,
    /// Every discovered kind in the cluster, for the kind picker as the web's `resourceKindsQuery` provides.
    pub catalog: Vec<DiscoveredResourceKind>,
    pub custom_open: bool,
    pub groups: std::collections::HashSet<String>,
    pub picker_open: bool,
    pub selected: Option<DiscoveredResourceKind>,
    pub kind_locked: bool,
    pub refresh_error: Option<String>,
    pub error_open: bool,
    discovery: Option<Task<()>>,
    catalog_task: Option<Task<()>>,
    watches: HashMap<String, Registration>,
    pending: Vec<super::live_rows::Patch>,
    /// The list version of each kind in the scope rows, by watch signature.
    pub versions: crate::backend::ListVersions,
}

#[derive(Default)]
struct Registration {
    phase: WatchPhase,
    reload_error: Option<String>,
    catching_up: bool,
    recycled: Vec<kubecove_lib::models::ResourceSummary>,
}

/// A watch's lifecycle, as `WatchPhase` in resource-watch.ts.
#[derive(Default)]
enum WatchPhase {
    /// The rows for the key have not settled, so the watch waits to start at their version.
    #[default]
    Waiting,
    /// The stream task runs; dropping the phase cancels it.
    Running { _task: InFlight, link: Link },
}

impl Registration {
    fn link(&mut self) -> Option<&mut Link> {
        match &mut self.phase {
            WatchPhase::Waiting => None,
            WatchPhase::Running { link, .. } => Some(link),
        }
    }

    fn summary(&self) -> WatchSummary {
        WatchSummary {
            link: match &self.phase {
                WatchPhase::Waiting => WatchLink::Opening,
                WatchPhase::Running { link, .. } => link.summary(),
            },
            refresh_failed: self.reload_error.is_some(),
            catching_up: self.catching_up,
        }
    }

    fn messages(&self) -> impl Iterator<Item = &String> {
        let [error, denied] = match &self.phase {
            WatchPhase::Waiting => [None, None],
            WatchPhase::Running { link, .. } => link.messages(),
        };
        [error, denied, self.reload_error.as_ref()]
            .into_iter()
            .flatten()
    }
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
        self.versions.clear();
        self.owner.clear();
        self.refresh_error = None;
    }
    /// The status without a pending refresh, which `ScopeLoad` owns.
    pub fn settled_status(&self) -> LiveStatus {
        watch_status::aggregate(
            self.refresh_error.is_some(),
            self.watches.values().map(Registration::summary),
        )
    }

    pub fn messages(&self) -> impl Iterator<Item = &String> {
        self.refresh_error
            .iter()
            .chain(self.watches.values().flat_map(Registration::messages))
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
        self.live.catalog.clear();
        let scope = self.scope.clone();
        let context = scope.context.clone();
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        // The picker lists every discovered kind whatever the namespace, as the web's kind picker does.
        let catalog_backend = self.backend.clone();
        let catalog_context = context.clone();
        self.live.catalog_task = Some(cx.spawn(async move |this, cx| {
            let result = crate::read_retry::cancelled_read(
                || catalog_backend.resource_kinds(catalog_context.clone()),
                |delay| cx.background_executor().timer(delay),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.scope.context != catalog_context {
                    return;
                }
                match result {
                    Ok(kinds) => this.live.catalog = kinds,
                    Err(error) => this
                        .warnings
                        .push(format!("Custom Resources: {}", error.message)),
                }
                cx.notify();
            });
        }));
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
                    Ok(kinds) => {
                        this.live.kinds = kinds;
                        if this.fill_custom_selection() {
                            this.filter(cx);
                        }
                    }
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
        let _profile = crate::frame_profile::scope("live", "sync");
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
            keys = vec![crate::backend::discovered_key(
                kind,
                self.scope.namespace.clone(),
            )];
        }
        for key in keys {
            let signature = crate::backend::watch_signature(&key);
            if self
                .live
                .watches
                .get(&signature)
                .is_some_and(|watch| matches!(watch.phase, WatchPhase::Running { .. }))
            {
                continue;
            }
            // A watch starts at the version of the rows the view shows, so it
            // waits until they settle. Until then it reports as connecting.
            if !self.rows_settled(&key) {
                self.live.watches.entry(signature).or_default();
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
                    let resource_version = this
                        .update(cx, |this, _| this.shown_version(&key))
                        .ok()
                        .flatten();
                    let read = {
                        let _span = crate::frame_profile::scope("watch", "open_factory");
                        backend.watch(WatchTarget::Resource {
                            scope: scope.clone(),
                            key: Box::new(key.clone()),
                            resource_version,
                        })
                    };
                    let mut stream = match read.await {
                        Ok(stream) => stream,
                        Err(error) => {
                            lost = true;
                            let _ = this.update(cx, |this, cx| {
                                if this.live.owner == owner
                                    && let Some(link) =
                                        this.live.watches.get_mut(&id).and_then(Registration::link)
                                {
                                    link.failed(error.message);
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
                                        Ok(KindRows {
                                            rows,
                                            resource_version,
                                        }) => {
                                            this.queue_live_patch(
                                                super::live_rows::Patch {
                                                    scope: scope.clone(),
                                                    key: key.clone(),
                                                    rows,
                                                    resource_version,
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
                        // A recovery tick with nothing to recover leaves the view as it is.
                        if tick && batch.is_empty() && !reload {
                            continue;
                        }
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
                                if let Some(link) = w.link() {
                                    if closed {
                                        link.stopped();
                                    }
                                    for event in transitions {
                                        match event {
                                            StreamMessage::Status {
                                                status, message, ..
                                            } => link.status(status, message),
                                            StreamMessage::Error { message, .. } => {
                                                link.failed(message)
                                            }
                                            StreamMessage::Stopped { .. } => link.stopped(),
                                            _ => {}
                                        }
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
                            // Notices that name every change read only those rows.
                            // A tick, a reconnect or an unnamed notice lists the kind.
                            let named = names.as_ref().map(crate::model::ObservedNames::shared);
                            let rows = crate::read_retry::read(
                                cx.background_executor().clone(),
                                move || {
                                    let recycled = recycled.take().unwrap_or_default();
                                    match &named {
                                        Some(named) => retry_backend.named_kind_resources(
                                            retry_scope.clone(),
                                            retry_key.clone(),
                                            named.clone(),
                                            recycled,
                                        ),
                                        None => retry_backend.kind_resources(
                                            retry_scope.clone(),
                                            retry_key.clone(),
                                            recycled,
                                        ),
                                    }
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
                                        Ok(KindRows {
                                            rows,
                                            resource_version,
                                        }) => {
                                            this.queue_live_patch(
                                                super::live_rows::Patch {
                                                    scope: scope.clone(),
                                                    key: key.clone(),
                                                    rows,
                                                    resource_version,
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
            self.live.watches.entry(signature).or_default().phase = WatchPhase::Running {
                _task: task.into(),
                link: Link::Opening,
            };
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
        if self.scope_load.loading() {
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
            let rows = self.patch_kind(
                &patch.scope,
                &patch.key,
                patch.rows,
                patch.resource_version,
                patch.names,
            );
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
