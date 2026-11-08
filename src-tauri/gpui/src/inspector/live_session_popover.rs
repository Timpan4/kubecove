//! `ActiveLiveSessionsButton.svelte`: the header's live-session count opens a
//! popover listing every session in the workspace's context, with Show,
//! Reconnect, Copy and Stop row controls and a Manage button for the page.

use super::{
    Inspector, Page,
    detail::{Detail, DetailTab},
    surface,
};
use crate::live_sessions::{exec_command, exec_status, forward_status, forward_title, local_url};
use crate::targets::ObservedElement;
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::{
    AnyElement, ClipboardItem, Context, Div, SharedString, Stateful, deferred, div, prelude::*, px,
};
use kubecove_lib::models::{
    PodExecSessionStatus, PodExecSessionSummary, PortForwardSessionSummary,
};

/// `Button size="sm"` row control with its test id and accessible name.
fn row_button(id: String, icon: &str, label: &str) -> ObservedElement<Stateful<Div>> {
    surface::small_button(Some(icon), label.to_owned())
        .id(SharedString::from(id))
        .test_support()
        .aria_label(label.to_owned())
}

impl Inspector {
    /// The header's count button, with its popover anchored beneath it while open.
    pub(super) fn live_session_control(&self, count: usize, cx: &mut Context<Self>) -> Div {
        let open = self.sessions.popover;
        let trigger = div()
            .id("live-sessions")
            .test_support()
            .aria_label(format!("Open live sessions ({count} active)"))
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(32.))
            .pl(px(6.))
            .pr(px(8.))
            .mr(px(4.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|button| button.bg(Tokens::muted()))
            .child(ui::icon("cable", 12., Tokens::muted_foreground()))
            .child(ui::muted(ui::medium(ui::text(
                count.to_string(),
                12.,
                19.5,
            ))))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.sessions.popover = !open;
                    cx.notify();
                }),
            );
        let mut wrapper = div().relative().child(trigger);
        if self.sessions.popover {
            wrapper = wrapper.child(deferred(self.live_session_panel(cx)).with_priority(1));
        }
        wrapper
    }

    fn live_session_panel(&self, cx: &mut Context<Self>) -> ObservedElement<Stateful<Div>> {
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(Ok(sessions)) = &self.sessions.load {
            let (forwards, execs) = sessions.for_context(&self.scope.context);
            let show_labels = sessions.show_source_labels;
            for session in execs {
                rows.push(
                    self.exec_control_row(session, show_labels, cx)
                        .into_any_element(),
                );
            }
            for session in forwards {
                rows.push(
                    self.forward_control_row(session, show_labels, cx)
                        .into_any_element(),
                );
            }
        }
        div()
            .id("live-session-popover")
            .test_support()
            .aria_label("Live sessions")
            .occlude()
            .absolute()
            .top(px(36.))
            .right_0()
            .w(px(384.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(6.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .shadow_lg()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.sessions.popover = false;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(ui::semibold(ui::sm("Live sessions")).truncate())
                            .child(ui::muted(ui::xs(
                                "Active port-forwards and Pod exec sessions",
                            ))),
                    )
                    .child(
                        surface::small_button(Some("cable"), "Manage")
                            .id("live-sessions-manage")
                            .test_support()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sessions.popover = false;
                                this.open_page(Page::PortForwards, cx)
                            })),
                    ),
            )
            .when_some(
                self.sessions.action.failure().map(str::to_owned),
                |panel, error| {
                    panel.child(
                        ui::xs(error)
                            .text_color(Tokens::destructive())
                            .id("live-session-popover-error")
                            .test_support(),
                    )
                },
            )
            .child(div().h(px(1.)).w_full().bg(Tokens::border()))
            .child(
                div()
                    .id("live-session-list")
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    fn exec_control_row(
        &self,
        session: &PodExecSessionSummary,
        show_labels: bool,
        cx: &mut Context<Self>,
    ) -> ObservedElement<Stateful<Div>> {
        let label = format!("{}/Pod/{}", session.namespace, session.pod_name);
        let showable = self
            .exec
            .tabs
            .iter()
            .any(|tab| tab.id.as_deref() == Some(session.id.as_str()));
        let (session_id, pod, namespace) = (
            session.id.clone(),
            session.pod_name.clone(),
            session.namespace.clone(),
        );
        let stop_id = session.id.clone();
        let source_label = session
            .kubeconfig_source_label
            .clone()
            .filter(|_| show_labels);
        let status = exec_status(session.status);
        let mut controls = div().flex().flex_wrap().justify_end().gap(px(8.));
        if showable {
            controls = controls.child(
                row_button(format!("live-session:{label}:show"), "cable", "Show").on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.show_exec_session(&session_id, &pod, &namespace, cx)
                    }),
                ),
            );
        }
        controls = controls.child(
            row_button(format!("live-session:{label}:stop"), "square", "Stop").on_click(
                cx.listener(move |this, _, _, cx| this.stop_exec_id(stop_id.clone(), cx)),
            ),
        );
        let badge = if matches!(session.status, PodExecSessionStatus::Error) {
            ui::status_badge(Tone::Error, status)
        } else {
            ui::outline_badge(status)
        };
        row_frame(&label)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::xs(label.clone())).truncate())
                            .child(
                                ui::muted(ui::xs(exec_command(&session.command)))
                                    .font_family(MONO)
                                    .truncate(),
                            )
                            .when_some(source_label, |text, source| {
                                text.child(ui::muted(ui::xs(source)).truncate())
                            }),
                    )
                    .child(status_badge(badge, &label, status)),
            )
            .when_some(session.last_error.clone(), |row, error| {
                row.child(
                    ui::xs(format!("Exec session failed: {error}"))
                        .text_color(Tokens::destructive()),
                )
            })
            .child(controls)
    }

    fn forward_control_row(
        &self,
        session: &PortForwardSessionSummary,
        show_labels: bool,
        cx: &mut Context<Self>,
    ) -> ObservedElement<Stateful<Div>> {
        let label = format!("{}/{}", session.namespace, forward_title(session));
        let (status, destructive) = forward_status(session.status);
        let url = local_url(session);
        let source_label = session
            .kubeconfig_source_label
            .clone()
            .filter(|_| show_labels);
        let description = format!("{url} {status}");
        let (reconnect, stop_id, copy_url) = (session.clone(), session.id.clone(), url.clone());
        let badge = if destructive {
            ui::status_badge(Tone::Error, status)
        } else {
            ui::outline_badge(status)
        };
        row_frame(&label)
            .aria_description(description)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::xs(label.clone())).truncate())
                            .child(ui::muted(ui::xs(url)).font_family(MONO).truncate())
                            .child(
                                ui::muted(ui::xs(format!(
                                    "Resolved {}:{}",
                                    session.resolved_pod_name, session.resolved_pod_port
                                )))
                                .truncate(),
                            )
                            .when_some(source_label, |text, source| {
                                text.child(ui::muted(ui::xs(source)).truncate())
                            }),
                    )
                    .child(status_badge(badge, &label, status)),
            )
            .when_some(session.last_error.clone(), |row, error| {
                row.child(
                    ui::xs(format!("Port-forward failed: {error}"))
                        .text_color(Tokens::destructive()),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        row_button(
                            format!("live-session:{label}:reconnect"),
                            "rotate-ccw",
                            "Reconnect",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.reconnect_forward(reconnect.clone(), window, cx)
                            },
                        )),
                    )
                    .child(
                        row_button(format!("live-session:{label}:copy"), "copy", "Copy").on_click(
                            cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()))
                            }),
                        ),
                    )
                    .child(
                        row_button(format!("live-session:{label}:stop"), "square", "Stop")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.stop_forward_id(stop_id.clone(), cx)
                            })),
                    ),
            )
    }

    /// `showOpenExecSession`: brings the Pod's Exec tab for this session into view.
    fn show_exec_session(
        &mut self,
        session_id: &str,
        pod: &str,
        namespace: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .exec
            .tabs
            .iter()
            .position(|tab| tab.id.as_deref() == Some(session_id))
        else {
            return;
        };
        self.exec.active = Some(index);
        self.sessions.popover = false;
        if let Detail::Ready(detail) = &mut self.detail
            && detail.data.summary.kind == "Pod"
            && detail.data.summary.name == pod
            && detail.data.summary.namespace.as_deref() == Some(namespace)
        {
            detail.tab = DetailTab::Exec;
        }
        cx.notify();
    }
}

/// The row's status badge, tested by the row's label and the status text.
fn status_badge(badge: Div, label: &str, status: &str) -> ObservedElement<Stateful<Div>> {
    badge
        .id(SharedString::from(format!("live-session-status:{label}")))
        .test_support()
        .aria_label(status.to_owned())
}

/// `rounded-md border bg-background p-2` row shell, identified by its label.
fn row_frame(label: &str) -> ObservedElement<Stateful<Div>> {
    div()
        .id(SharedString::from(format!("live-session-row:{label}")))
        .test_support()
        .aria_label(label.to_owned())
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background())
}
