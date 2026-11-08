//! `StopExecSessionsDialog.svelte` and `openExecSessions.svelte.ts`.
use super::{
    Inspector, Page,
    controls::{Variant, button},
};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, div, prelude::*, px};
use kubecove_lib::models::ResourceSummary;

pub(super) enum LeaveIntent {
    Close,
    Page(Page),
    Resource(Box<ResourceSummary>),
    Namespace(Option<String>),
    IncidentResources(crate::incidents::Filter),
    Context(String),
    Workspace(u64),
}
impl Inspector {
    /// Returns true when the caller must not leave now: an unsaved YAML draft refuses first,
    /// so exec sessions are never stopped for a navigation the draft then blocks.
    pub(super) fn defer_exec_leave(&mut self, intent: LeaveIntent, cx: &mut Context<Self>) -> bool {
        if !self.can_leave(cx) {
            return true;
        }
        if self
            .exec
            .tabs
            .iter()
            .any(|t| t.id.is_some() || t.status == "starting" || t.status == "connecting")
        {
            self.exec.leave = Some(intent);
            cx.notify();
            true
        } else {
            false
        }
    }
    pub(super) fn exec_leave_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.exec.leave.as_ref()?;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::black().opacity(0.5))
                .child(
                    ui::card()
                        .w(px(448.))
                        .p(px(24.))
                        .gap(px(16.))
                        .flex()
                        .flex_col()
                        .child(ui::heading(
                            format!(
                                "Stop {} exec session{}?",
                                self.exec.tabs.len(),
                                if self.exec.tabs.len() == 1 { "" } else { "s" }
                            ),
                            18.,
                            28.,
                        ))
                        .child(ui::muted(ui::sm(
                            "Leaving this view stops the open exec sessions.",
                        )))
                        .children(
                            self.exec.tabs.iter().map(|t| {
                                ui::xs(format!("{} in {}", t.command.join(" "), t.container))
                            }),
                        )
                        .when_some(self.exec.leave_error.clone(), |card, error| {
                            card.child(ui::xs(error).text_color(Tokens::destructive()))
                        })
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(
                                    button(Variant::Outline, 28., false)
                                        .id("exec-leave-cancel")
                                        .test_support()
                                        .child(ui::xs("Stay"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.exec.leave = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button(Variant::Destructive, 28., self.exec.stopping)
                                        .id("exec-leave-confirm")
                                        .test_support()
                                        .child(ui::xs("Stop and leave"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            if this.exec.stopping {
                                                return;
                                            }
                                            this.exec.stopping = true;
                                            let backend = this.backend.clone();
                                            let ids: Vec<_> = this
                                                .exec
                                                .tabs
                                                .iter()
                                                .filter_map(|t| t.id.clone())
                                                .collect();
                                            cx.spawn_in(window, async move |this, cx| {
                                                let mut error = None;
                                                for id in ids {
                                                    match backend.stop_exec(id).await {
                                                        Ok(true) => {}
                                                        Ok(false) => {
                                                            error = Some(
                                                                "A session refused to stop".into(),
                                                            );
                                                            break;
                                                        }
                                                        Err(e) => {
                                                            error = Some(e.message);
                                                            break;
                                                        }
                                                    }
                                                }
                                                let _ = this.update_in(cx, |this, window, cx| {
                                                    this.exec.stopping = false;
                                                    this.exec.leave_error = error;
                                                    if this.exec.leave_error.is_some() {
                                                        cx.notify();
                                                        return;
                                                    }
                                                    let intent = this.exec.leave.take();
                                                    this.exec.clear();
                                                    match intent {
                                                        Some(LeaveIntent::Close) => {
                                                            this.close_detail(cx)
                                                        }
                                                        Some(LeaveIntent::Page(page)) => {
                                                            this.open_page(page, cx)
                                                        }
                                                        Some(LeaveIntent::Resource(resource)) => {
                                                            this.open_resource(
                                                                *resource, window, cx,
                                                            )
                                                        }
                                                        Some(LeaveIntent::Namespace(namespace)) => {
                                                            this.select_namespace(
                                                                namespace, window, cx,
                                                            )
                                                        }
                                                        Some(LeaveIntent::IncidentResources(
                                                            filter,
                                                        )) => this.open_incident_resources(
                                                            filter, window, cx,
                                                        ),
                                                        Some(LeaveIntent::Context(context)) => {
                                                            this.scope.context = context;
                                                            this.scope.namespace = None;
                                                            this.load_scope(false, window, cx);
                                                        }
                                                        Some(LeaveIntent::Workspace(id)) => {
                                                            this.open_workspace(id, window, cx)
                                                        }
                                                        None => {}
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                            .detach();
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
