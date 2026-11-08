//! Port Forwards page, mirroring `LiveSessionsSurface.svelte` and
//! `LiveSessionsView.svelte`. Row actions use typed ADR 0003 session commands.

use super::{
    Inspector,
    friendly_error::Operation,
    port_forward_rows::{card, card_title, stat, table},
    surface,
};
use crate::backend::LiveSessions;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{AnyElement, Context, Task, div, prelude::*, px};
use kubecove_lib::models::AppError;
use std::rc::Rc;

pub(super) struct SessionsState {
    pub(super) form: Option<super::saved_forward_form::PresetForm>,
    pub(super) load: Option<Result<LiveSessions, AppError>>,
    pub(super) action: SessionAction,
    pub(super) scope_context: Option<String>,
    /// `ActiveLiveSessionsButton`'s popover, open from the header count.
    pub(super) popover: bool,
    request: Rc<()>,
    task: Option<Task<()>>,
    poll: Option<Task<()>>,
    pub(super) rows: super::windowed::Windowed,
}

impl SessionsState {
    pub(super) fn new() -> Self {
        Self {
            form: None,
            load: None,
            action: SessionAction::Idle,
            scope_context: None,
            popover: false,
            request: Rc::new(()),
            task: None,
            poll: None,
            rows: Default::default(),
        }
    }
}

/// The latest start, stop or save of a session. One runs at a time, as the
/// web disables its controls while a start is pending.
pub(super) enum SessionAction {
    Idle,
    Running,
    Done(ActionFeedback),
}

/// What a finished action reports. `PortForwardTab.svelte` shows a saved
/// preset as a neutral `Alert` and failures as `FriendlyError`s.
pub(super) enum ActionFeedback {
    Info(String),
    Failed(String),
}

impl SessionAction {
    pub(super) fn running(&self) -> bool {
        matches!(self, SessionAction::Running)
    }

    pub(super) fn failure(&self) -> Option<&str> {
        match self {
            SessionAction::Done(ActionFeedback::Failed(message)) => Some(message),
            _ => None,
        }
    }

    pub(super) fn info(&self) -> Option<&str> {
        match self {
            SessionAction::Done(ActionFeedback::Info(message)) => Some(message),
            _ => None,
        }
    }

    /// `Idle` without errors, else the errors joined one per line.
    pub(super) fn finished(errors: Vec<String>) -> Self {
        if errors.is_empty() {
            SessionAction::Idle
        } else {
            SessionAction::Done(ActionFeedback::Failed(errors.join("\n")))
        }
    }
}

impl Inspector {
    /// `ActiveLiveSessionsButton`'s count: every port forward and exec session
    /// on the host, without the workspace filter the page applies.
    pub(super) fn live_session_count(&self) -> Option<usize> {
        match &self.sessions.load {
            Some(Ok(sessions)) => {
                let (forwards, execs) = sessions.for_context(&self.scope.context);
                Some(forwards.len() + execs.len())
            }
            _ => None,
        }
    }

    pub(super) fn ensure_sessions(&mut self, cx: &mut Context<Self>) {
        if self.sessions.scope_context.as_deref() != Some(&self.scope.context)
            || self
                .sessions
                .task
                .as_ref()
                .is_none_or(|task| task.is_ready())
        {
            self.load_sessions(cx);
        }
    }

    pub(super) fn load_sessions(&mut self, cx: &mut Context<Self>) {
        if self.sessions.poll.is_none() {
            // `portForwardLifecycle.ts` and `podExecLifecycle.ts` query default.
            self.sessions.poll = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(3_000))
                        .await;
                    if this
                        .update(cx, |this, cx| this.ensure_sessions(cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        let keep = self
            .persisted
            .preferences
            .keep_live_sessions_on_workspace_switch;
        // A failed read stays visible until the next read resolves, as the web's
        // query keeps its error during a refetch. The 3 s poll must not show Loading.
        let state = &mut self.sessions;
        state.request = Rc::new(());
        let request = state.request.clone();
        let context = self.scope.context.clone();
        let changed = state
            .scope_context
            .as_ref()
            .is_some_and(|old| *old != context);
        state.scope_context = Some(context.clone());
        let cleanup = changed && !keep;
        let previous_source = state
            .load
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|s| s.source_key.clone());
        let backend = self.backend.clone();
        let executor = cx.background_executor().clone();
        let source = backend.clone();
        let read_sessions = move || {
            let source = source.clone();
            crate::read_retry::read(executor.clone(), move || source.live_sessions())
        };
        let read = async move {
            let sessions = read_sessions().await?;
            if (cleanup
                || (!keep && previous_source.is_some() && previous_source != sessions.source_key))
                && let Some(source) = &sessions.source_key
            {
                backend
                    .cleanup_sessions(kubecove_lib::models::LiveSessionCleanupRequest {
                        allowed_cluster_contexts: vec![context],
                        kubeconfig_source_key: source.clone(),
                    })
                    .await?;
                return read_sessions().await;
            }
            Ok(sessions)
        };
        state.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if Rc::ptr_eq(&this.sessions.request, &request) {
                    // The 3 s poll usually reads the same list. Redrawing the window for it
                    // costs a full Inspector render, which at 10k rows dominates idle CPU.
                    let unchanged = match (&this.sessions.load, &result) {
                        (Some(Ok(old)), Ok(new)) => old == new,
                        (Some(Err(old)), Err(new)) => {
                            old.kind == new.kind && old.message == new.message
                        }
                        _ => false,
                    };
                    this.sessions.load = Some(result);
                    if !unchanged {
                        cx.notify();
                    }
                }
            });
        }));
    }

    pub(super) fn port_forwards_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = surface::pending_slot(
            self.loading
                .phase("surface-sessions", self.sessions.load.is_none()),
        );
        let body: AnyElement = match (&self.sessions.load, pending) {
            (_, Some(slot)) => slot,
            (None, None) => div().into_any_element(),
            (Some(Err(error)), None) => {
                self.page_read_failure("Live sessions unavailable", error, Operation::Surface, cx)
            }
            (Some(Ok(sessions)), None) => {
                let (forwards, exec) = sessions.for_context(&self.scope.context);
                let counts = (forwards.len(), exec.len());
                let forward_rows = self.forward_rows(sessions, cx);
                let exec_rows = self.exec_rows(sessions, cx);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .when_some(self.sessions.action.failure().map(str::to_owned), |body, error| body.child(
                        div().id("session-action-message").test_support().aria_label(error.clone())
                            .child(ui::xs(error).text_color(Tokens::destructive()))
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::background().opacity(0.5))
                            .child(
                                div().flex().items_center().gap(px(8.)).id("auto-start-forwards").test_support()
                                    .aria_label("Auto-start saved port forwards").on_click(cx.listener(|this, _, _, cx| {
                                        this.persisted.preferences.auto_start_saved_port_forwards = !this.persisted.preferences.auto_start_saved_port_forwards;
                                        this.save_preferences(cx);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .size(px(16.))
                                            .rounded(px(4.))
                                            .border_1()
                                            .border_color(Tokens::input())
                                            .bg(Tokens::input().opacity(0.3))
                                            .when(self.persisted.preferences.auto_start_saved_port_forwards, |box_| box_.bg(Tokens::primary()).child(ui::icon("check", 14., Tokens::primary_foreground()))),
                                    )
                                    .child(ui::muted(ui::xs("Auto-start saved"))),
                            )
                            .child(surface::small_button(
                                Some("play"),
                                "Start saved",
                            ).id("restore-forwards").test_support().on_click(cx.listener(|this, _, window, cx| this.restore_forwards(window, cx)))),
                    )
                    .child(
                        // `StatGrid`: `xl:grid-cols-5`, two cards filled.
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(stat("Port forwards", counts.0))
                            .child(stat("Exec sessions", counts.1))
                            .child(div().flex_1())
                            .child(div().flex_1())
                            .child(div().flex_1()),
                    )
                    .child(
                        card()
                            .child(
                                card_title(
                                    "Port forwards",
                                    "Active local tunnels with guarded stop controls.",
                                )
                                .px(px(12.)),
                            )
                            .child(table(
                                &["Target", "Namespace", "Local", "Resolved", "Status", "Actions"],
                                forward_rows,
                                "No active port forwards",
                            )),
                    )
                    .child(
                        card()
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .justify_between()
                                    .gap(px(12.))
                                    .px(px(12.))
                                    // The web wraps these in a plain `div`, so the
                                    // header's `gap-1` does not apply between them.
                                    .child(
                                        card_title(
                                            "Saved Service forwards",
                                            "Launch saved selector-backed Service forwards for this workspace.",
                                        )
                                        .gap(px(0.)),
                                    )
                                    .child(surface::small_button(
                                        Some("plus"),
                                        "Save forward",
                                    ).id("save-forward").test_support().on_click(cx.listener(|this, _, window, cx| this.open_preset_form(None, window, cx)))),
                            )
                            .children(self.preset_form(cx))
                            .child(table(
                                &[
                                    "Preset",
                                    "Cluster",
                                    "Namespace",
                                    "Service",
                                    "Local",
                                    "Status",
                                    "Actions",
                                ],
                                self.preset_table_rows(cx),
                                "No saved Service forwards",
                            )),
                    )
                    .child(
                        card()
                            .child(
                                card_title(
                                    "Pod exec sessions",
                                    "Active interactive Pod sessions with guarded stop controls.",
                                )
                                .px(px(12.)),
                            )
                            .child(table(
                                &["Pod", "Namespace", "Command", "Status", "Started", "Actions"],
                                exec_rows,
                                "No active exec sessions",
                            )),
                    )
                    .into_any_element()
            }
        };
        surface::frame("link-2", "Live Sessions", false, body)
    }
}
