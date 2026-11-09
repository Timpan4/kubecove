//! The overview's GitOps inventory: its read and the body of the GitOps
//! `<section>` in `src/features/workspaces/WorkspaceOverview.svelte`.

use super::{Inspector, request::Query};
use crate::gitops::GitOpsInventory;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, FontWeight, Hsla, SharedString, div, prelude::*, px};
use kubecove_lib::models::AppError;
use std::time::Duration;

#[derive(Default)]
pub(super) struct GitOpsState {
    /// The context `load` belongs to; empty before the first read.
    context: String,
    load: Query<GitOpsInventory>,
    /// Bumped whenever `load` changes, so the GitOps page knows to rebuild its rows.
    pub(super) revision: u64,
    /// A refresh asked for while a read ran; it reads again once that read settles.
    trailing: bool,
}

impl GitOpsState {
    pub(super) fn inventory(&self) -> Option<&GitOpsInventory> {
        self.load.data()
    }

    pub(super) fn error(&self) -> Option<&AppError> {
        self.load.refresh_error().or(self.load.failure())
    }

    pub(super) fn fetching(&self) -> bool {
        self.load.fetching()
    }
}

/// `flex items-center justify-between py-2 text-xs` with a `<strong>` count.
fn count(label: &str, value: usize, tone: Option<Hsla>) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(format!("gitops-count:{label}")))
        .test_support()
        .aria_label(value.to_string())
        .flex()
        .items_center()
        .justify_between()
        .py(px(8.))
        .child(ui::muted(ui::xs(label.to_owned())))
        .child(
            ui::xs(value.to_string())
                .font_weight(FontWeight::BOLD)
                .text_color(tone.unwrap_or(Tokens::foreground())),
        )
}

/// `Separator`: `h-px w-full bg-border`.
fn separator() -> Div {
    div().h(px(1.)).w_full().flex_none().bg(Tokens::border())
}

/// `inline-flex items-center gap-2 text-xs text-muted-foreground` with a `Spinner`.
fn pending(label: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
        .child(ui::muted(ui::xs(label.to_owned())))
}

impl Inspector {
    /// Reads the inventory once per context, and again on refresh. Replacing
    /// the read drops, and so cancels, a read for the previous context.
    pub(super) fn load_gitops(&mut self, refresh: bool, cx: &mut Context<Self>) {
        if self.scope.context.is_empty() {
            return;
        }
        if self.gitops.context == self.scope.context && self.gitops.load.fetching() {
            self.gitops.trailing |= refresh;
            return;
        }
        if !refresh && self.gitops.context == self.scope.context {
            return;
        }
        if self.gitops.context != self.scope.context {
            self.gitops.load = Query::default();
            self.gitops.trailing = false;
        }
        self.gitops.context = self.scope.context.clone();
        // A watch event retries while an error shows, so the error hides during the read.
        self.gitops.load.clear_refresh_error();
        self.gitops.revision += 1;
        self.start_gitops_read(None, cx);
    }

    /// Starts one inventory read, after `delay` when a trailing refresh coalesces.
    fn start_gitops_read(&mut self, delay: Option<Duration>, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let context = self.scope.context.clone();
        self.gitops.load.start(cx.spawn(async move |this, cx| {
            if let Some(delay) = delay {
                cx.background_executor().timer(delay).await;
            }
            let result = crate::read_retry::read(cx.background_executor().clone(), move || {
                backend.gitops(context.clone())
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.scope.context != this.gitops.context {
                    return;
                }
                // A partial inventory after a full one keeps the full one and reports the list error.
                let shown = this.gitops.inventory().is_some();
                let result = result.and_then(|inventory| {
                    match inventory.list_error.clone().filter(|_| shown) {
                        Some(error) => Err(error),
                        None => Ok(inventory),
                    }
                });
                let finished = result.is_ok();
                this.gitops.load.settle(result);
                if finished {
                    this.page_read_finished(super::Page::GitOps, cx);
                }
                this.gitops.revision += 1;
                if std::mem::take(&mut this.gitops.trailing) {
                    this.start_gitops_read(Some(super::live_data::COALESCE), cx);
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn cancel_gitops_read(&mut self) {
        self.gitops.load.cancel();
        self.gitops.trailing = false;
    }

    pub(super) fn gitops_body(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let body = div().id("gitops-inventory").test_support().p(px(12.));
        let phase = self
            .loading
            .phase("overview-gitops", self.gitops.load.pending());
        let content: AnyElement = match self.gitops.load.settled() {
            // WorkspaceOverview.svelte checks detection first.
            _ if phase.shown() => pending("Detecting...").into_any_element(),
            None => div().into_any_element(),
            _ if !phase.ready() => div().into_any_element(),
            Some(Err(error)) => div()
                .id("gitops-error")
                .role(gpui_kit::Role::Alert)
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::destructive().opacity(0.4))
                .child(
                    ui::semibold(ui::xs("Failed to load GitOps inventory"))
                        .text_color(Tokens::destructive()),
                )
                .child(ui::muted(ui::xs(error.message.clone())))
                .into_any_element(),
            Some(Ok(inventory)) if !inventory.detected() => {
                ui::muted(ui::xs("Not detected")).into_any_element()
            }
            Some(Ok(inventory)) => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .when_some(inventory.argo.as_ref(), |list, apps| {
                    list.child(count("Applications", apps.len(), None))
                        .child(separator())
                        .child(count(
                            "Out of sync",
                            inventory.out_of_sync(),
                            Some(Palette::amber300()),
                        ))
                        .child(separator())
                })
                .when_some(inventory.flux.as_ref(), |list, rows| {
                    list.child(count("Flux resources", rows.len(), None))
                        .child(separator())
                })
                // `Button variant="outline"` at the default size (`h-7`, inline-start icon).
                .child(
                    ui::outline_button()
                        .id("open-gitops")
                        .justify_center()
                        .h(px(28.))
                        .pl(px(6.))
                        .pr(px(8.))
                        .test_support()
                        .cursor_pointer()
                        .child(ui::icon("git-branch", 14., Tokens::foreground()))
                        .child(ui::medium(ui::text("Open GitOps", 12., 19.5)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_gitops(crate::gitops_surface::Focus::Section, cx)
                        })),
                )
                .into_any_element(),
        };
        body.child(content)
    }
}
