//! Page-owned finite reads and GitOps watch invalidation.
use super::{
    Inspector, Page,
    live_data::{self, COALESCE, RECOVERY, WatchInput},
};
use crate::backend::WatchTarget;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{StreamMessage, StreamStatus, WatchResourceKey};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct PageRefresh {
    owner: Option<(Page, String)>,
    read_at: [Option<Instant>; 4],
    watches: Vec<Task<()>>,
    watch_keys: Vec<String>,
    statuses: std::collections::HashMap<String, WatchState>,
}

#[derive(Clone, Copy, PartialEq)]
enum WatchState {
    Opening,
    Live,
    Lost,
    Forbidden,
}

fn slot(page: Page) -> Option<(usize, Duration)> {
    // These are query staleTime values, not polling intervals.
    // GitOpsSurface.svelte, HelmSurface.svelte, IncidentSurface.svelte: 15_000.
    // RbacSurface.svelte: 30_000.
    match page {
        Page::GitOps => Some((0, Duration::from_millis(15_000))),
        Page::Helm => Some((1, Duration::from_millis(15_000))),
        Page::Rbac => Some((2, Duration::from_millis(30_000))),
        Page::Incidents => Some((3, Duration::from_millis(15_000))),
        _ => None,
    }
}

impl Inspector {
    pub(super) fn live_label(&self) -> &'static str {
        if self.page != Page::GitOps {
            return self.live.label();
        }
        if self.gitops.error().is_some() {
            return "Refresh failed";
        }
        let statuses = &self.page_refresh.statuses;
        if statuses.is_empty() {
            return "Not connected";
        }
        if statuses
            .values()
            .all(|status| *status == WatchState::Opening)
        {
            return "Connecting";
        }
        if statuses.values().any(|status| *status == WatchState::Lost) {
            return "Reconnecting";
        }
        if statuses
            .values()
            .all(|status| *status == WatchState::Forbidden)
        {
            return "Polling (limited)";
        }
        if statuses
            .values()
            .any(|status| *status == WatchState::Forbidden)
        {
            return "Live (limited)";
        }
        "Live"
    }
    pub(super) fn invalidate_page_read(&mut self, page: Page) {
        if let Some((index, _)) = slot(page) {
            self.page_refresh.read_at[index] = None;
        }
    }
    pub(super) fn page_read_finished(&mut self, page: Page, cx: &Context<Self>) {
        if let Some((index, _)) = slot(page) {
            self.page_refresh.read_at[index] = Some(cx.background_executor().now());
        }
    }

    pub(super) fn page_read_stale(&self, page: Page, cx: &Context<Self>) -> bool {
        slot(page).is_some_and(|(index, stale)| {
            self.page_refresh.read_at[index]
                .is_none_or(|at| cx.background_executor().now().duration_since(at) >= stale)
        })
    }

    pub(super) fn refresh_focused_page(&mut self, cx: &mut Context<Self>) {
        if self.page == Page::Helm {
            self.refresh_helm_details(cx);
        }
        if self.page_read_stale(self.page, cx) {
            match self.page {
                Page::GitOps => self.load_gitops(true, cx),
                Page::Helm => self.load_helm(cx),
                Page::Rbac => self.refresh_rbac(cx),
                Page::Incidents => self.load_incidents(cx),
                _ => {}
            }
        }
    }

    pub(super) fn sync_page_reads(&mut self, cx: &mut Context<Self>) {
        let owner = (self.page, self.scope.context.clone());
        if self.page_refresh.owner.as_ref() == Some(&owner) {
            return;
        }
        let old = self.page_refresh.owner.replace(owner);
        if old
            .as_ref()
            .is_some_and(|(_, context)| *context != self.scope.context)
        {
            self.page_refresh.read_at = [None; 4];
        }
        if let Some((page, _)) = old {
            match page {
                Page::GitOps => self.cancel_gitops_read(),
                Page::Helm => self.cancel_helm_reads(),
                Page::Rbac => self.cancel_rbac_read(),
                Page::Incidents => self.cancel_incident_reads(),
                _ => {}
            }
        }
        self.page_refresh.watches.clear();
        self.page_refresh.watch_keys.clear();
        self.page_refresh.statuses.clear();
        cx.notify();
    }

    pub(super) fn sync_gitops_watches(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_page_reads(cx);
        if self.page != Page::GitOps {
            return;
        }
        let Some(data) = self.gitops.inventory() else {
            return;
        };
        let mut keys: Vec<WatchResourceKey> = Vec::new();
        if data.argo.is_some() {
            for (kind, plural) in [
                ("Application", "applications"),
                ("ApplicationSet", "applicationsets"),
                ("AppProject", "appprojects"),
            ] {
                let mut key = live_data::key(kind, None);
                key.resource_kind.api_version = Some("argoproj.io/v1alpha1".into());
                key.resource_kind.plural = Some(plural.into());
                keys.push(key);
            }
        }
        keys.extend(
            self.live
                .kinds
                .iter()
                .filter(|kind| kind.group.ends_with("toolkit.fluxcd.io"))
                .map(|kind| live_data::dynamic_key(kind, None)),
        );
        let signatures: Vec<_> = keys
            .iter()
            .map(|key| serde_json::to_string(key).expect("watch key"))
            .collect();
        if signatures == self.page_refresh.watch_keys {
            return;
        }
        self.page_refresh.watches.clear();
        self.page_refresh.statuses.clear();
        self.page_refresh.watch_keys = signatures;
        for key in keys {
            let signature = serde_json::to_string(&key).expect("watch key");
            self.page_refresh
                .statuses
                .insert(signature.clone(), WatchState::Opening);
            let backend = self.backend.clone();
            let scope = self.scope.clone();
            let context = scope.context.clone();
            self.page_refresh
                .watches
                .push(cx.spawn_in(window, async move |this, cx| {
                    loop {
                        let opened = backend
                            .watch(WatchTarget::Resource {
                                scope: scope.clone(),
                                key: key.clone(),
                            })
                            .await;
                        let mut recovery = Box::pin(cx.background_executor().timer(RECOVERY));
                        let mut stream = match opened {
                            Ok(stream) => stream,
                            Err(_) => {
                                let _ = this.update(cx, |this, cx| {
                                    this.page_refresh
                                        .statuses
                                        .insert(signature.clone(), WatchState::Lost);
                                    cx.notify();
                                });
                                recovery.await;
                                let _ = this.update(cx, |this, cx| this.load_gitops(true, cx));
                                continue;
                            }
                        };
                        let mut lost = false;
                        loop {
                            let event =
                                live_data::receive_or_tick(&mut stream, &mut recovery).await;
                            let mut reload = false;
                            let mut ended = false;
                            let mut status = None;
                            match event {
                                WatchInput::Message(StreamMessage::ResourceChanged { .. }) => {
                                    reload = true
                                }
                                WatchInput::Message(StreamMessage::Status {
                                    status: StreamStatus::Connected,
                                    ..
                                }) => {
                                    reload = lost;
                                    lost = false;
                                    status = Some(WatchState::Live);
                                }
                                WatchInput::Message(StreamMessage::Status {
                                    status: StreamStatus::Forbidden,
                                    ..
                                }) => {
                                    lost = true;
                                    status = Some(WatchState::Forbidden);
                                }
                                WatchInput::Message(
                                    StreamMessage::Status { .. } | StreamMessage::Error { .. },
                                ) => {
                                    lost = true;
                                    status = Some(WatchState::Lost);
                                }
                                WatchInput::Message(StreamMessage::Stopped { .. })
                                | WatchInput::Closed => {
                                    ended = true;
                                    status = Some(WatchState::Lost);
                                }
                                WatchInput::Tick => {
                                    reload = lost;
                                    recovery = Box::pin(cx.background_executor().timer(RECOVERY));
                                }
                                _ => {}
                            }
                            let valid = this
                                .update(cx, |this, cx| {
                                    if this.page != Page::GitOps || this.scope.context != context {
                                        return false;
                                    }
                                    if let Some(status) = status {
                                        this.page_refresh
                                            .statuses
                                            .insert(signature.clone(), status);
                                        cx.notify();
                                    }
                                    if reload || this.gitops.error().is_some() {
                                        this.load_gitops(true, cx);
                                    }
                                    true
                                })
                                .unwrap_or(false);
                            if !valid {
                                return;
                            }
                            if ended {
                                break;
                            }
                            if reload {
                                cx.background_executor().timer(COALESCE).await;
                            }
                        }
                        recovery.await;
                        let _ = this.update(cx, |this, cx| this.load_gitops(true, cx));
                    }
                }));
        }
    }
}
