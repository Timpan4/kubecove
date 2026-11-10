//! Page-owned finite reads and GitOps watch invalidation.
use super::{
    Inspector, Page,
    live_data::{self, COALESCE, RECOVERY, WatchInput},
    watch_status::{self, LiveStatus, WatchLink},
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
    statuses: std::collections::HashMap<String, WatchLink>,
}

impl PageRefresh {
    /// Each GitOps watch's connection, for the status menu.
    pub(super) fn links(&self) -> impl Iterator<Item = WatchLink> + '_ {
        self.statuses.values().copied()
    }
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
    pub(super) fn live_status(&self) -> LiveStatus {
        if self.page != Page::GitOps {
            // ResourceLiveStatusMenu.svelte delays and holds "Refreshing…" like other indicators.
            let refreshing = self
                .loading
                .phase("live-refresh", self.scope_load.refreshing());
            return if refreshing.shown() {
                LiveStatus::Refreshing
            } else {
                self.live.settled_status()
            };
        }
        watch_status::aggregate(
            self.gitops.error().is_some(),
            self.page_refresh.statuses.values().map(|&link| link.into()),
        )
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
                Page::GitOps if !self.gitops.fetching() => self.load_gitops(true, cx),
                Page::Helm => self.load_helm(cx),
                Page::Rbac => self.refresh_rbac(cx),
                Page::Incidents => self.load_incidents(cx),
                _ => {}
            }
        }
    }

    pub(super) fn sync_page_reads(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Settings {
            self.settings.diagnostics_task.take();
        }
        // Without an open workspace no workspace page is active, so its reads stop with it.
        let page = if self.workspace_open() {
            self.page
        } else {
            Page::Settings
        };
        let owner = (page, self.scope.context.clone());
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
        if self.page != Page::GitOps || !self.workspace_open() {
            return;
        }
        let Some(data) = self.gitops.inventory() else {
            return;
        };
        let mut keys: Vec<WatchResourceKey> = Vec::new();
        if data.argo.is_some() {
            keys.extend(
                crate::gitops::ARGO_KINDS
                    .map(|(kind, plural)| crate::gitops::argo_watch_key(kind, plural)),
            );
        }
        keys.extend(
            self.live
                .kinds
                .iter()
                .filter(|kind| kind.group.ends_with("toolkit.fluxcd.io"))
                .map(|kind| crate::backend::discovered_key(kind, None)),
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
                .insert(signature.clone(), WatchLink::Opening);
            let backend = self.backend.clone();
            let scope = self.scope.clone();
            let context = scope.context.clone();
            self.page_refresh
                .watches
                .push(cx.spawn_in(window, async move |this, cx| {
                    loop {
                        // Each open, reconnects included, starts at the version of
                        // the inventory rows the page shows for this kind.
                        let resource_version = this
                            .update(cx, |this, _| {
                                this.gitops
                                    .inventory()
                                    .and_then(|inventory| inventory.version(&key))
                            })
                            .ok()
                            .flatten();
                        let opened = backend
                            .watch(WatchTarget::Resource {
                                scope: scope.clone(),
                                key: Box::new(key.clone()),
                                resource_version,
                            })
                            .await;
                        let mut recovery = Box::pin(cx.background_executor().timer(RECOVERY));
                        let mut stream = match opened {
                            Ok(stream) => stream,
                            Err(_) => {
                                let _ = this.update(cx, |this, cx| {
                                    this.page_refresh
                                        .statuses
                                        .insert(signature.clone(), WatchLink::Lost);
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
                                    status = Some(WatchLink::Connected);
                                }
                                WatchInput::Message(StreamMessage::Status {
                                    status: StreamStatus::Forbidden,
                                    ..
                                }) => {
                                    lost = true;
                                    status = Some(WatchLink::Denied);
                                }
                                WatchInput::Message(
                                    StreamMessage::Status { .. } | StreamMessage::Error { .. },
                                ) => {
                                    lost = true;
                                    status = Some(WatchLink::Lost);
                                }
                                WatchInput::Message(StreamMessage::Stopped { .. })
                                | WatchInput::Closed => {
                                    ended = true;
                                    status = Some(WatchLink::Lost);
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
