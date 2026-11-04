//! Port Forwards page, mirroring `LiveSessionsSurface.svelte` and
//! `LiveSessionsView.svelte`. Read-only: sessions and their states are listed,
//! but starting, reconnecting and stopping sessions (ADR 0003), saving
//! forwards and auto-start are rendered disabled. Copying a local URL stays
//! available because it changes nothing.

use super::{Inspector, surface};
use crate::backend::LiveSessions;
use crate::live_sessions::{
    exec_command, exec_status, forward_resolution, forward_status, forward_title, local_url,
};
use crate::theme::{HEADING, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, FontWeight, SharedString, Task, div, prelude::*, px,
};
use std::rc::Rc;

pub(super) struct SessionsState {
    load: Option<Result<LiveSessions, String>>,
    request: Rc<()>,
    task: Option<Task<()>>,
}

impl SessionsState {
    pub(super) fn new() -> Self {
        Self {
            load: None,
            request: Rc::new(()),
            task: None,
        }
    }
}

/// `Card size="sm"`: `rounded-lg border bg-surface-1 py-3 gap-3`.
fn card() -> Div {
    ui::card().py(px(12.)).gap(px(12.))
}

/// `CardHeader` with `CardTitle` and `CardDescription`.
fn card_title(title: &str, description: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(ui::heading(title.to_owned(), 14., 20.))
        .child(ui::muted(ui::text(description.to_owned(), 12., 19.5)))
}

/// `Button size="xs"`: `h-5 rounded-sm px-2 text-[0.625rem]`, `size-2.5` icon.
fn xs_button(icon: &str, label: &str, destructive: bool) -> Div {
    let button = ui::outline_button()
        .h(px(20.))
        .pl(px(6.))
        .pr(px(8.))
        .rounded(px(4.));
    let (button, color) = if destructive {
        (
            button
                .border_color(gpui_kit::transparent_black())
                .bg(Tokens::destructive().opacity(0.2)),
            Tokens::destructive(),
        )
    } else {
        (button, Tokens::foreground())
    };
    button
        .child(ui::icon(icon, 10., color))
        .child(ui::medium(ui::text(label.to_owned(), 10., 15.)).text_color(color))
}

fn cell(content: impl IntoElement) -> Div {
    div()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .px(px(12.))
        .py(px(8.))
        .overflow_hidden()
        .child(content)
}

/// `Table class="table-fixed text-sm"`: equal columns, header row `h-10`.
fn table(headers: &[&str], rows: Vec<Vec<AnyElement>>, empty: &str) -> Div {
    let columns = headers.len();
    let header = div()
        .flex()
        .items_center()
        .h(px(40.))
        .border_b_1()
        .border_color(Tokens::border())
        .children(headers.iter().map(|header| {
            cell(
                ui::muted(ui::xs(header.to_uppercase()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_nowrap(),
            )
        }));
    let body: Vec<Div> = if rows.is_empty() {
        vec![
            div()
                .flex()
                .justify_center()
                .px(px(12.))
                .py(px(32.))
                .child(ui::muted(ui::sm(empty.to_owned()))),
        ]
    } else {
        let last = rows.len() - 1;
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                debug_assert_eq!(row.len(), columns);
                div()
                    .flex()
                    .items_center()
                    .when(index < last, |row| {
                        row.border_b_1().border_color(Tokens::border())
                    })
                    .children(row.into_iter().map(cell))
            })
            .collect()
    };
    div().px(px(12.)).child(header).children(body)
}

fn text(content: impl Into<SharedString>) -> AnyElement {
    ui::sm(content).truncate().into_any_element()
}

fn stat(label: &str, value: usize) -> Div {
    card().flex_1().min_w_0().child(
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .child(ui::muted(ui::text(label.to_owned(), 12., 19.5)))
            .child(
                ui::text(value.to_string(), 20., 28.)
                    .font_family(HEADING)
                    .font_weight(FontWeight::MEDIUM),
            ),
    )
}

impl Inspector {
    /// `ActiveLiveSessionsButton`'s count: every port forward and exec session
    /// on the host, without the workspace filter the page applies.
    pub(super) fn live_session_count(&self) -> Option<usize> {
        match &self.sessions.load {
            Some(Ok(sessions)) => Some(sessions.port_forwards.len() + sessions.exec_sessions.len()),
            _ => None,
        }
    }

    pub(super) fn load_sessions(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.sessions;
        state.request = Rc::new(());
        let request = state.request.clone();
        let read = self.backend.live_sessions();
        state.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if Rc::ptr_eq(&this.sessions.request, &request) {
                    this.sessions.load = Some(result.map_err(|error| error.message));
                    cx.notify();
                }
            });
        }));
    }

    fn forward_rows(
        &self,
        sessions: &LiveSessions,
        cx: &mut Context<Self>,
    ) -> Vec<Vec<AnyElement>> {
        let (forwards, _) = sessions.for_context(&self.scope.context);
        forwards
            .into_iter()
            .map(|session| {
                let (status, destructive) = forward_status(session.status);
                let url = local_url(session);
                vec![
                    div()
                        .id(SharedString::from(format!("forward:{}", session.id)))
                        .test_support()
                        .aria_label(forward_title(session))
                        .min_w_0()
                        .child(ui::medium(ui::sm(forward_title(session))).truncate())
                        .when_some(
                            session
                                .kubeconfig_source_label
                                .clone()
                                .filter(|_| sessions.show_source_labels),
                            |title, label| title.child(ui::muted(ui::xs(label)).truncate()),
                        )
                        .into_any_element(),
                    text(session.namespace.clone()),
                    text(url.clone()),
                    text(forward_resolution(session)),
                    div()
                        .min_w_0()
                        .child(if destructive {
                            ui::status_badge(Tone::Error, status)
                        } else {
                            ui::outline_badge(status)
                        })
                        .when_some(session.last_error.clone(), |cell, error| {
                            cell.child(
                                ui::xs(format!("Port-forward failed: {error}"))
                                    .text_color(Tokens::destructive())
                                    .mt(px(4.)),
                            )
                        })
                        .into_any_element(),
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(surface::disabled(xs_button(
                            "rotate-ccw",
                            "Reconnect",
                            false,
                        )))
                        .child(
                            xs_button("copy", "Copy", false)
                                .id(SharedString::from(format!("copy-forward:{}", session.id)))
                                .test_support()
                                .cursor_pointer()
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))
                                })),
                        )
                        .child(surface::disabled(xs_button("square", "Stop", true)))
                        .into_any_element(),
                ]
            })
            .collect()
    }

    fn exec_rows(sessions: &LiveSessions, context: &str) -> Vec<Vec<AnyElement>> {
        let (_, exec) = sessions.for_context(context);
        exec.into_iter()
            .map(|session| {
                vec![
                    div()
                        .min_w_0()
                        .child(ui::medium(ui::sm(session.pod_name.clone())).truncate())
                        .when_some(
                            session
                                .kubeconfig_source_label
                                .clone()
                                .filter(|_| sessions.show_source_labels),
                            |title, label| title.child(ui::muted(ui::xs(label)).truncate()),
                        )
                        .into_any_element(),
                    text(session.namespace.clone()),
                    text(exec_command(&session.command)),
                    text(exec_status(session.status)),
                    text(super::incident_guide::timestamp(&session.started_at)),
                    surface::disabled(xs_button("square", "Stop", true)).into_any_element(),
                ]
            })
            .collect()
    }

    pub(super) fn port_forwards_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let body: AnyElement = match &self.sessions.load {
            None => surface::loading(),
            Some(Err(error)) => surface::failed("Live sessions unavailable", error.clone()),
            Some(Ok(sessions)) => {
                let (forwards, exec) = sessions.for_context(&self.scope.context);
                let counts = (forwards.len(), exec.len());
                let forward_rows = self.forward_rows(sessions, cx);
                let exec_rows = Self::exec_rows(sessions, &self.scope.context);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
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
                                surface::disabled(div().flex().items_center().gap(px(8.)))
                                    .child(
                                        div()
                                            .size(px(16.))
                                            .rounded(px(4.))
                                            .border_1()
                                            .border_color(Tokens::input())
                                            .bg(Tokens::input().opacity(0.3)),
                                    )
                                    .child(ui::muted(ui::xs("Auto-start saved"))),
                            )
                            .child(surface::disabled(surface::small_button(
                                Some("play"),
                                "Start saved",
                            ))),
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
                                    .child(surface::disabled(surface::small_button(
                                        Some("plus"),
                                        "Save forward",
                                    ))),
                            )
                            // The native client has no saved workspaces yet.
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
                                vec![],
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
