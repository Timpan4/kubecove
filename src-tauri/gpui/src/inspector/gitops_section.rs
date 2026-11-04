//! The overview's GitOps inventory: its read and the body of the GitOps
//! `<section>` in `src/features/workspaces/WorkspaceOverview.svelte`.

use super::Inspector;
use crate::gitops::GitOpsInventory;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, FontWeight, Hsla, Task, div, prelude::*, px};

#[derive(Default)]
pub(super) struct GitOpsState {
    /// The context `load` belongs to; empty before the first read.
    context: String,
    load: GitOpsLoad,
    task: Option<Task<()>>,
}

#[derive(Default)]
enum GitOpsLoad {
    #[default]
    Detecting,
    Ready(GitOpsInventory),
    Failed(String),
}

/// `flex items-center justify-between py-2 text-xs` with a `<strong>` count.
fn count(label: &str, value: usize, tone: Option<Hsla>) -> Div {
    div()
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
        if !refresh && self.gitops.context == self.scope.context {
            return;
        }
        self.gitops.context = self.scope.context.clone();
        self.gitops.load = GitOpsLoad::Detecting;
        let read = self.backend.gitops(self.scope.context.clone());
        self.gitops.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                this.gitops.load = match result {
                    Ok(inventory) => GitOpsLoad::Ready(inventory),
                    Err(error) => GitOpsLoad::Failed(error.message),
                };
                cx.notify();
            });
        }));
    }

    pub(super) fn gitops_body(&self) -> impl IntoElement + use<> {
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
                        .child(ui::icon("git-branch", 14., Tokens::foreground()))
                        .child(ui::medium(ui::text("Open GitOps", 12., 19.5)))
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(
                                "Not available in the native client yet",
                            )
                            .build(window, cx)
                        }),
                )
                .into_any_element(),
        };
        body.child(content)
    }
}
