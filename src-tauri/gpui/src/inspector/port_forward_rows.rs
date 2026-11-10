//! `LiveSessionsView.svelte`: table cells, row controls, and small cards.
use super::Inspector;
use super::live_session_popover::RowLabels;
use super::windowed::row_id;
use crate::targets::TestSupportExt;
use crate::{
    backend::LiveSessions,
    live_sessions::*,
    theme::{HEADING, Tokens},
    ui::{self, Tone},
};
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, FontWeight, SharedString, div, prelude::*, px,
};
/// `Card size="sm"`: `rounded-lg border bg-surface-1 py-3 gap-3`.
pub(super) fn card() -> Div {
    ui::card().py(px(12.)).gap(px(12.))
}

/// `CardHeader` with `CardTitle` and `CardDescription`.
pub(super) fn card_title(title: &str, description: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(ui::heading(title.to_owned(), 14., 20.))
        .child(ui::muted(ui::text(description.to_owned(), 12., 19.5)))
}

/// `Button size="xs"`: `h-5 rounded-sm px-2 text-[0.625rem]`, `size-2.5` icon.
pub(super) fn xs_button(icon: &str, label: &str, destructive: bool) -> Div {
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

pub(super) fn cell(content: impl IntoElement) -> Div {
    div()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .px(px(12.))
        .py(px(8.))
        .overflow_hidden()
        .child(content)
}

/// One body row of `table`, ruled below unless it is the last.
pub(super) fn table_row(cells: Vec<AnyElement>, last: bool) -> AnyElement {
    div()
        .flex()
        .items_center()
        .when(!last, |row| row.border_b_1().border_color(Tokens::border()))
        .children(cells.into_iter().map(cell))
        .into_any_element()
}

/// `Table class="table-fixed text-sm"`: equal columns, header row `h-10`.
/// `rows` holds the `table_row`s, or is `None` when the table is empty.
pub(super) fn table(headers: &[&str], rows: Option<Div>, empty: &str) -> Div {
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
    let body = rows.unwrap_or_else(|| {
        div().flex().justify_center().px(px(12.)).py(px(32.)).child(
            ui::muted(ui::sm(empty.to_owned()))
                .id(SharedString::from(format!("session-empty:{empty}")))
                .test_support()
                .aria_label(empty.to_owned()),
        )
    });
    div().px(px(12.)).child(header).child(body)
}

pub(super) fn text(content: impl Into<SharedString>) -> AnyElement {
    ui::sm(content).truncate().into_any_element()
}

pub(super) fn stat(label: &str, value: usize) -> Div {
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
    /// The port forward table's rows, rendered only near the page's view.
    pub(super) fn forward_rows(
        &self,
        sessions: &LiveSessions,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let contexts = self.workspace_contexts();
        let (forwards, _) = sessions.for_contexts(&contexts);
        let labels = RowLabels {
            source: sessions.show_source_labels,
            context: contexts.len() > 1,
        };
        let last = forwards.len().checked_sub(1)?;
        // The title gains context and source lines and the status an error
        // line; the other cells truncate.
        let ids = forwards
            .iter()
            .enumerate()
            .map(|(index, session)| {
                row_id((
                    forward_title(session),
                    labels.context(&session.cluster_context),
                    sessions
                        .show_source_labels
                        .then_some(&session.kubeconfig_source_label),
                    forward_status(session.status),
                    &session.last_error,
                    index == last,
                ))
            })
            .collect();
        let entity = cx.entity_id();
        Some(self.sessions.rows.rows(0, ids, entity, |index| {
            table_row(
                self.forward_cells(sessions, forwards[index], labels, cx),
                index == last,
            )
        }))
    }

    /// The exec session table's rows, rendered only near the page's view.
    pub(super) fn exec_rows(&self, sessions: &LiveSessions, cx: &mut Context<Self>) -> Option<Div> {
        let contexts = self.workspace_contexts();
        let (_, exec) = sessions.for_contexts(&contexts);
        let labels = RowLabels {
            source: sessions.show_source_labels,
            context: contexts.len() > 1,
        };
        let last = exec.len().checked_sub(1)?;
        // Only the pod cell's optional context and source lines change a row's height.
        let ids = exec
            .iter()
            .enumerate()
            .map(|(index, session)| {
                row_id((
                    &session.pod_name,
                    labels.context(&session.cluster_context),
                    sessions
                        .show_source_labels
                        .then_some(&session.kubeconfig_source_label),
                    index == last,
                ))
            })
            .collect();
        Some(self.sessions.rows.rows(1, ids, cx.entity_id(), |index| {
            table_row(
                self.exec_cells(sessions, exec[index], labels, cx),
                index == last,
            )
        }))
    }

    fn forward_cells(
        &self,
        sessions: &LiveSessions,
        session: &kubecove_lib::models::PortForwardSessionSummary,
        labels: RowLabels,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let reconnect = session.clone();
        let stop_id = session.id.clone();
        let (status, destructive) = forward_status(session.status);
        let url = local_url(session);
        vec![
            div()
                .id(SharedString::from(format!("forward:{}", session.id)))
                .test_support()
                .aria_label(forward_title(session))
                .aria_description(format!(
                    "{} {} {}:{}",
                    session.local_port,
                    status,
                    session.resolved_pod_name,
                    session.resolved_pod_port
                ))
                .min_w_0()
                .child(ui::medium(ui::sm(forward_title(session))).truncate())
                .when_some(
                    labels.context(&session.cluster_context),
                    |title, context| title.child(ui::muted(ui::xs(context)).truncate()),
                )
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
                    let message = format!("Port-forward failed: {error}");
                    cell.child(
                        ui::xs(message.clone())
                            .id(SharedString::from(format!("forward-error:{}", session.id)))
                            .test_support()
                            .aria_label(message)
                            .text_color(Tokens::destructive())
                            .mt(px(4.)),
                    )
                })
                .into_any_element(),
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .child(
                    xs_button("rotate-ccw", "Reconnect", false)
                        .id(SharedString::from(format!(
                            "reconnect-forward:{}",
                            forward_title(session)
                        )))
                        .test_support()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.reconnect_forward(reconnect.clone(), window, cx)
                        })),
                )
                .child(
                    xs_button("copy", "Copy", false)
                        .id(SharedString::from(format!("copy-forward:{}", session.id)))
                        .test_support()
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))
                        })),
                )
                .child(
                    xs_button("square", "Stop", true)
                        .id(SharedString::from(format!(
                            "stop-forward:{}",
                            forward_title(session)
                        )))
                        .test_support()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.stop_forward_id(stop_id.clone(), cx)
                        })),
                )
                .into_any_element(),
        ]
    }

    fn exec_cells(
        &self,
        sessions: &LiveSessions,
        session: &kubecove_lib::models::PodExecSessionSummary,
        labels: RowLabels,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let id = session.id.clone();
        vec![
            div()
                .min_w_0()
                .child(ui::medium(ui::sm(session.pod_name.clone())).truncate())
                .when_some(
                    labels.context(&session.cluster_context),
                    |title, context| title.child(ui::muted(ui::xs(context)).truncate()),
                )
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
            super::timestamp_text::with_tooltip(
                format!("exec-started:{id}"),
                Some(session.started_at.clone()),
                text(super::incident_guide::timestamp(&session.started_at)),
            ),
            xs_button("square", "Stop", true)
                .id(SharedString::from(format!("stop-exec:{id}")))
                .test_support()
                .on_click(cx.listener(move |this, _, _, cx| this.stop_exec_id(id.clone(), cx)))
                .into_any_element(),
        ]
    }
}
