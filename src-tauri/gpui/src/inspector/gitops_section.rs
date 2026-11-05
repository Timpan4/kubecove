//! The overview's GitOps inventory: its read and the body of the GitOps
//! `<section>` in `src/features/workspaces/WorkspaceOverview.svelte`.

use super::Inspector;
use crate::gitops::GitOpsInventory;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, Task, div, prelude::*, px,
};

#[derive(Default)]
pub(super) struct GitOpsState {
    /// The context `load` belongs to; empty before the first read.
    context: String,
    load: GitOpsLoad,
    /// Bumped whenever `load` changes, so the GitOps page knows to rebuild its rows.
    pub(super) revision: u64,
    task: Option<Task<()>>,
    request: std::rc::Rc<()>,
    pub(super) fetching: bool,
    trailing: bool,
    refresh_error: Option<String>,
}

impl GitOpsState {
    pub(super) fn inventory(&self) -> Option<&GitOpsInventory> {
        match &self.load {
            GitOpsLoad::Ready(inventory) => Some(inventory),
            _ => None,
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        if let Some(error) = &self.refresh_error {
            return Some(error);
        }
        match &self.load {
            GitOpsLoad::Failed(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Default)]
enum GitOpsLoad {
    #[default]
    Detecting,
    Ready(GitOpsInventory),
    Failed(String),
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
    /// the task drops, and so cancels, a read for the previous context.
    pub(super) fn load_gitops(&mut self, refresh: bool, cx: &mut Context<Self>) {
        if self.scope.context.is_empty() {
            return;
        }
        if self.gitops.context == self.scope.context && self.gitops.fetching {
            self.gitops.trailing |= refresh;
            return;
        }
        if !refresh && self.gitops.context == self.scope.context {
            return;
        }
        if self.gitops.context != self.scope.context {
            self.cancel_gitops_read();
            self.gitops.load = GitOpsLoad::Detecting;
        }
        self.gitops.context = self.scope.context.clone();
        self.gitops.fetching = true;
        self.gitops.refresh_error = None;
        self.gitops.revision += 1;
        let backend = self.backend.clone();
        let context = self.scope.context.clone();
        let request = self.gitops.request.clone();
        self.gitops.task = Some(cx.spawn(async move |this, cx| {
            loop {
                let retry_backend = backend.clone();
                let retry_context = context.clone();
                let result = crate::read_retry::read(cx.background_executor().clone(), move || {
                    retry_backend.gitops(retry_context.clone())
                })
                .await;
                let again = this
                    .update(cx, |this, cx| {
                        if !std::rc::Rc::ptr_eq(&this.gitops.request, &request)
                            || this.scope.context != context
                        {
                            return false;
                        }
                        match result {
                            Ok(inventory)
                                if inventory.list_error.is_some()
                                    && this.gitops.inventory().is_some() =>
                            {
                                this.gitops.refresh_error = inventory.list_error;
                            }
                            Ok(inventory) => {
                                this.gitops.load = GitOpsLoad::Ready(inventory);
                                this.gitops.refresh_error = None;
                                this.page_read_finished(super::Page::GitOps, cx);
                            }
                            Err(error) => {
                                if this.gitops.inventory().is_some() {
                                    this.gitops.refresh_error = Some(error.message);
                                } else {
                                    this.gitops.load = GitOpsLoad::Failed(error.message);
                                }
                            }
                        }
                        this.gitops.revision += 1;
                        let again = std::mem::take(&mut this.gitops.trailing);
                        this.gitops.fetching = again;
                        cx.notify();
                        again
                    })
                    .unwrap_or(false);
                if !again {
                    break;
                }
                cx.background_executor()
                    .timer(super::live_data::COALESCE)
                    .await;
            }
        }));
    }

    pub(super) fn cancel_gitops_read(&mut self) {
        self.gitops.task.take();
        self.gitops.request = std::rc::Rc::new(());
        self.gitops.fetching = false;
        self.gitops.trailing = false;
    }

    pub(super) fn gitops_body(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let body = div().id("gitops-inventory").test_support().p(px(12.));
        let content: AnyElement = match &self.gitops.load {
            GitOpsLoad::Detecting => pending("Detecting...").into_any_element(),
            GitOpsLoad::Failed(error) => div()
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
                .child(ui::muted(ui::xs(error.clone())))
                .into_any_element(),
            GitOpsLoad::Ready(inventory) if !inventory.detected() => {
                ui::muted(ui::xs("Not detected")).into_any_element()
            }
            GitOpsLoad::Ready(inventory) => div()
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
