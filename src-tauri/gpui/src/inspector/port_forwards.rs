//! Port Forwards page, mirroring `LiveSessionsSurface.svelte` and
//! `LiveSessionsView.svelte`. Row actions use typed ADR 0003 session commands.

use super::{
    Inspector,
    friendly_error::Operation,
    port_forward_rows::{card, card_title, stat, table},
    request::Query,
    surface,
};
use crate::backend::LiveSessions;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{AnyElement, Context, Task, div, prelude::*, px};

pub(super) struct SessionsState {
    pub(super) form: Option<super::saved_forward_form::PresetForm>,
    pub(super) load: Query<LiveSessions>,
    pub(super) action: SessionAction,
    /// The workspace contexts of the last read. A change to the set cleans up
    /// sessions outside it, unless sessions are kept across workspaces.
    pub(super) scope_contexts: Option<Vec<String>>,
    /// A scope change's cleanup has not succeeded yet, so the next read retries it.
    cleanup_pending: bool,
    /// `ActiveLiveSessionsButton`'s popover, open from the header count.
    pub(super) popover: bool,
    poll: Option<Task<()>>,
    pub(super) rows: super::windowed::Windowed,
}

impl SessionsState {
    pub(super) fn new() -> Self {
        Self {
            form: None,
            load: Query::default(),
            action: SessionAction::Idle,
            scope_contexts: None,
            cleanup_pending: false,
            popover: false,
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
        let sessions = self.sessions.load.data()?;
        let (forwards, execs) = sessions.for_contexts(&self.workspace_contexts());
        Some(forwards.len() + execs.len())
    }

    pub(super) fn ensure_sessions(&mut self, cx: &mut Context<Self>) {
        let contexts = self.workspace_contexts();
        if self
            .sessions
            .scope_contexts
            .as_deref()
            .is_none_or(|old| !super::scope_load::same_contexts(old, &contexts))
            || !self.sessions.load.fetching()
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
        let contexts = self.workspace_contexts();
        let state = &mut self.sessions;
        let changed = state
            .scope_contexts
            .as_deref()
            .is_some_and(|old| !super::scope_load::same_contexts(old, &contexts));
        state.scope_contexts = Some(contexts.clone());
        // A failed or replaced read leaves the cleanup pending for the next one.
        let cleanup = (changed || state.cleanup_pending) && !keep;
        state.cleanup_pending = cleanup;
        let previous_source = state.load.data().and_then(|s| s.source_key.clone());
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
                        allowed_cluster_contexts: contexts,
                        kubeconfig_source_key: source.clone(),
                    })
                    .await?;
                return read_sessions().await;
            }
            Ok(sessions)
        };
        state.load.start(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                // The 3 s poll usually reads the same list. Redrawing the window for it
                // costs a full Inspector render, which at 10k rows dominates idle CPU.
                let load = &this.sessions.load;
                let unchanged = match &result {
                    Ok(new) => load.data() == Some(new) && load.refresh_error().is_none(),
                    Err(new) => load
                        .refresh_error()
                        .or(load.failure())
                        .is_some_and(|old| old.kind == new.kind && old.message == new.message),
                };
                if cleanup && result.is_ok() {
                    this.sessions.cleanup_pending = false;
                }
                this.sessions.load.settle(result);
                if !unchanged {
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn port_forwards_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = surface::pending_slot(
            self.loading
                .phase("surface-sessions", self.sessions.load.pending()),
        );
        let body: AnyElement = match (self.sessions.load.settled(), pending) {
            (_, Some(slot)) => slot,
            (None, None) => div().into_any_element(),
            (Some(Err(error)), None) => {
                self.page_read_failure("Live sessions unavailable", error, Operation::Surface, cx)
            }
            (Some(Ok(sessions)), None) => {
                let (forwards, exec) = sessions.for_contexts(&self.workspace_contexts());
                let counts = (forwards.len(), exec.len());
                let forward_rows = self.forward_rows(sessions, cx);
                let exec_rows = self.exec_rows(sessions, cx);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    // `SurfaceFrame` shows a failed refetch above the data it keeps.
                    .when_some(self.sessions.load.refresh_error().cloned(), |body, error| {
                        body.child(self.page_read_failure(
                            "Live sessions unavailable",
                            &error,
                            Operation::Surface,
                            cx,
                        ))
                    })
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
