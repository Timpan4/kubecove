//! `StopExecSessionsDialog.svelte` and `openExecSessions.svelte.ts`.
use super::{
    Inspector, Page,
    controls::{Variant, button},
    request::InFlight,
};
use crate::targets::TestSupportExt;
use crate::ui;
use gpui_kit::{AnyElement, Context, Window, div, prelude::*, px};
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

pub(super) enum LeaveDialog {
    Closed,
    /// The dialog asks before this navigation stops the open sessions.
    Confirming(LeaveIntent),
    /// "Stop and leave" closed the dialog, as `answerStopPrompt` does, and the stops run.
    /// The intent runs once every session stopped; a failure keeps the user on the Pod,
    /// where the failed terminal shows the error.
    Stopping {
        intent: LeaveIntent,
        _stop: InFlight,
    },
}

impl Inspector {
    /// Returns true when the caller must not leave now: an unsaved YAML draft refuses first,
    /// so exec sessions are never stopped for a navigation the draft then blocks.
    pub(super) fn defer_exec_leave(&mut self, intent: LeaveIntent, cx: &mut Context<Self>) -> bool {
        if !self.can_leave(cx) {
            return true;
        }
        // A navigation during the stops replaces the one they run for.
        if let LeaveDialog::Stopping {
            intent: pending, ..
        } = &mut self.exec.leave
        {
            *pending = intent;
            return true;
        }
        if self.exec.tabs.iter().any(|tab| tab.blocks_leave()) {
            self.exec.leave = LeaveDialog::Confirming(intent);
            cx.notify();
            true
        } else {
            false
        }
    }

    fn stop_exec_and_leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let LeaveDialog::Confirming(intent) =
            std::mem::replace(&mut self.exec.leave, LeaveDialog::Closed)
        else {
            return;
        };
        let backend = self.backend.clone();
        let mut ids = Vec::new();
        for tab in &mut self.exec.tabs {
            if let Some(id) = tab.session_id().map(str::to_owned) {
                tab.stopping();
                ids.push(id);
            }
        }
        let stop = cx.spawn_in(window, async move |this, cx| {
            // Every session gets its stop, as `confirmStoppingExecSessions` stops every tab.
            let mut results = Vec::with_capacity(ids.len());
            for id in ids {
                let result = backend.stop_exec(id.clone()).await;
                results.push((id, result));
            }
            let _ = this.update_in(cx, |this, window, cx| {
                let LeaveDialog::Stopping { intent, .. } =
                    std::mem::replace(&mut this.exec.leave, LeaveDialog::Closed)
                else {
                    return;
                };
                let mut stopped_all = true;
                for (id, result) in results {
                    let Some(tab) = this
                        .exec
                        .tabs
                        .iter_mut()
                        .find(|tab| tab.session_id() == Some(id.as_str()))
                    else {
                        continue;
                    };
                    match result {
                        Ok(true) => tab.stopped(),
                        Ok(false) => {
                            stopped_all = false;
                            tab.stop_failed("Stop failed; the session may still be running".into());
                        }
                        Err(error) => {
                            stopped_all = false;
                            tab.stop_failed(error.into());
                        }
                    }
                }
                this.load_sessions(cx);
                if stopped_all {
                    this.exec.clear();
                    this.leave_for(intent, window, cx);
                }
                cx.notify();
            });
        });
        self.exec.leave = LeaveDialog::Stopping {
            intent,
            _stop: stop.into(),
        };
        cx.notify();
    }

    fn leave_for(&mut self, intent: LeaveIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            LeaveIntent::Close => self.close_detail(cx),
            LeaveIntent::Page(page) => self.open_page(page, cx),
            LeaveIntent::Resource(resource) => self.open_resource(*resource, window, cx),
            LeaveIntent::Namespace(namespace) => self.select_namespace(namespace, window, cx),
            LeaveIntent::IncidentResources(filter) => {
                self.open_incident_resources(filter, window, cx)
            }
            LeaveIntent::Context(context) => {
                self.scope.context = context;
                self.scope.namespace = None;
                self.load_scope(false, window, cx);
            }
            LeaveIntent::Workspace(id) => self.open_workspace(id, window, cx),
        }
    }

    pub(super) fn exec_leave_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let LeaveDialog::Confirming(_) = &self.exec.leave else {
            return None;
        };
        let title = format!(
            "Stop {} exec session{}?",
            self.exec.tabs.len(),
            if self.exec.tabs.len() == 1 { "" } else { "s" }
        );
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
                        // `DialogContent` takes its accessible name from the title.
                        .id("exec-leave-dialog")
                        .test_support()
                        .role(gpui_kit::Role::Dialog)
                        .aria_label(title.clone())
                        .w(px(448.))
                        .p(px(24.))
                        .gap(px(16.))
                        .flex()
                        .flex_col()
                        .child(ui::heading(title, 18., 28.))
                        .child(ui::muted(ui::sm(
                            "Leaving this view stops the open exec sessions.",
                        )))
                        .children(
                            self.exec.tabs.iter().map(|t| {
                                ui::xs(format!("{} in {}", t.command.join(" "), t.container))
                            }),
                        )
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
                                            this.exec.leave = LeaveDialog::Closed;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button(Variant::Destructive, 28., false)
                                        .id("exec-leave-confirm")
                                        .test_support()
                                        .child(ui::xs("Stop and leave"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.stop_exec_and_leave(window, cx)
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
