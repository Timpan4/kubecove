//! `ExecTab.svelte`: empty `min-h-72`, session strip `h-9 gap-0.5`, and
//! `ExecSessionTerminal.svelte`'s `px-2.5 pt-2` on the #101113 terminal.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
    exec_menu,
};
use crate::targets::TestSupportExt;
use crate::{
    theme::{MONO, Tokens},
    ui,
};
use gpui_kit::{AnyElement, Context, SharedString, div, prelude::*, px, rgb};

fn status_dot(status: &str) -> gpui_kit::Div {
    let color = match status {
        "running" => super::events::tw::emerald500(1.),
        "starting" | "connecting" => super::events::tw::amber500(1.),
        "error" => super::log_row::red500(1.),
        _ => Tokens::muted_foreground().opacity(0.6),
    };
    // `ExecTab.svelte`: size-[7px] rounded-full.
    div().size(px(7.)).flex_none().rounded_full().bg(color)
}

impl Inspector {
    pub(super) fn exec_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Detail::Ready(detail) = &self.detail else {
            return div().into_any_element();
        };
        let status = detail.data.status.as_ref();
        let mut containers: Vec<String> = status
            .into_iter()
            .flat_map(|s| {
                ["containerStatuses", "ephemeralContainerStatuses"]
                    .into_iter()
                    .flat_map(|key| s[key].as_array().into_iter().flatten())
            })
            .filter_map(|c| c["name"].as_str().map(str::to_owned))
            .collect();
        if containers.is_empty() {
            containers.push(String::new());
        }
        let default = containers.first().cloned().unwrap_or_default();
        let menu = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .when(self.exec.menu, |menu| {
                menu.absolute()
                    .top(px(36.))
                    .left(px(4.))
                    .w(px(256.))
                    .p(px(4.))
                    .bg(Tokens::background())
                    .border_1()
                    .border_color(Tokens::border())
                    .rounded(px(6.))
                    .occlude()
                    .when_some(self.exec_custom_menu(cx), |menu, custom| menu.child(custom))
                    .when(self.exec.custom.is_none(), |menu| {
                        menu.children(containers.iter().flat_map(|container| {
                            ["/bin/sh", "/bin/bash"].map(|shell| {
                                let container = container.clone();
                                button(Variant::Outline, 28., false)
                                    .id(SharedString::from(format!(
                                        "exec-open:{container}:{shell}"
                                    )))
                                    .test_support()
                                    .child(ui::xs(format!("Open {shell} in {container}")))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.start_exec(
                                            container.clone(),
                                            vec![shell.into()],
                                            window,
                                            cx,
                                        )
                                    }))
                            })
                        }))
                        .child(
                            button(Variant::Ghost, 28., false)
                                .id("exec-custom")
                                .test_support()
                                .child(ui::xs("Custom command…"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_exec_custom(window, cx)
                                })),
                        )
                    })
            });
        if self.exec.tabs.is_empty() {
            return div().relative().flex().flex_col().items_center().justify_center().min_h(px(288.)).gap(px(12.))
                .border_1().border_color(Tokens::border()).rounded(px(8.))
                .child(ui::heading("No exec sessions", 14., 20.))
                .child(ui::muted(ui::xs("Open a shell inside this Pod. Sessions stop when you close the Pod or switch to another one.")))
                .child(button(Variant::Default, 28., false).id("exec-start").test_support()
                    .child(ui::xs(format!("Open /bin/sh in {}", if default.is_empty() { "default container" } else { &default })))
                    .on_click(cx.listener(move |this, _, window, cx| this.start_exec(default.clone(), vec!["/bin/sh".into()], window, cx))))
                .child(button(Variant::Outline, 28., false).id("exec-other").test_support().child(ui::xs("Other"))
                    .on_click(cx.listener(|this, _, _, cx| { this.exec.menu = !this.exec.menu; cx.notify(); })))
                .child(menu).into_any_element();
        }
        // `ExecSessionTerminal.svelte` wraps its `FriendlyError` in `shrink-0 p-2`.
        let error = self.exec.active().and_then(|t| t.error.as_ref()).map(|error| {
            div().flex_none().p(px(8.)).child(self.friendly_failure(
                "exec-error",
                error.kind,
                &error.message,
                super::friendly_error::Context {
                    operation: Some(super::friendly_error::Operation::Exec),
                    fallback_title: Some("Exec failed"),
                    ..Default::default()
                },
                None,
                cx,
            ))
        });
        let tabs = self.exec.tabs.iter().map(|tab| {
            let key = tab.key;
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .px(px(8.))
                .border_b_2()
                .border_color(gpui_kit::transparent_black())
                .when(self.exec.active == Some(key), |tab| {
                    tab.border_color(Tokens::foreground())
                })
                .child(
                    div()
                        .id(SharedString::from(format!("exec-tab:{key}")))
                        .test_support()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .aria_label(format!(
                            "{} · {}, {}",
                            tab.command[0].rsplit('/').next().unwrap_or_default(),
                            tab.container,
                            tab.status_label()
                        ))
                        .child(status_dot(tab.status_label()))
                        .child(ui::xs(format!(
                            "{} · {}",
                            tab.command[0].rsplit('/').next().unwrap_or_default(),
                            tab.container
                        )))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.exec.active = Some(key);
                            window.focus(&this.exec.focus, cx);
                            cx.notify();
                        })),
                )
                .child(
                    ui::ghost_icon("x", 24., 14.)
                        .id(SharedString::from(format!("exec-close:{key}")))
                        .test_support()
                        .aria_label("Stop and close")
                        .on_click(cx.listener(move |this, _, _, cx| this.close_exec(key, cx))),
                )
        });
        let active = self.exec.active();
        let entity = cx.entity();
        let focus = self.exec.focus.clone();
        let active_key = self.exec.active;
        let current_size = active.map(|t| t.parser.screen().size());
        let pending_start =
            active.is_some_and(|t| matches!(t.phase, super::exec_state::ExecPhase::Pending(_)));
        let dragging = self.exec.drag.is_some();
        let has_selection = active.is_some_and(|tab| tab.has_selection());
        let menu_owner = entity.clone();
        div()
            .relative()
            .flex()
            .flex_col()
            .min_h(px(288.))
            .flex_1()
            .border_1()
            .border_color(Tokens::border())
            .rounded(px(8.))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .h(px(36.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .children(tabs)
                    .child(
                        ui::ghost_icon("plus", 28., 14.)
                            .id("exec-new")
                            .test_support()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.exec.menu = !this.exec.menu;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .when_some(active, |row, tab| {
                        let key = tab.key;
                        let container = tab.container.clone();
                        let command = tab.command.clone();
                        row.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(status_dot(tab.status_label()))
                                .child(ui::xs(tab.status_label())),
                        )
                        .child(
                            ui::ghost_icon("rotate-ccw", 28., 14.)
                                .id("exec-clear")
                                .test_support()
                                .aria_label("Clear terminal")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(tab) =
                                        this.exec.tabs.iter_mut().find(|t| t.key == key)
                                    {
                                        tab.parser.process(b"\x1b[2J\x1b[H");
                                        tab.lines.clear();
                                        tab.refresh_view();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(if tab.session_id().is_some() {
                            button(Variant::Ghost, 28., false)
                                .child(ui::icon("square", 14., Tokens::destructive()))
                                .id("exec-stop")
                                .test_support()
                                .aria_label("Stop session")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    // Stopping keeps the terminal's selection copyable from the keyboard.
                                    window.focus(&this.exec.focus, cx);
                                    this.stop_exec_tab(key, cx)
                                }))
                        } else {
                            ui::ghost_icon("play", 28., 14.)
                                .id("exec-restart")
                                .test_support()
                                .aria_label(format!(
                                    "Restart {} in {}",
                                    command.join(" "),
                                    container
                                ))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.close_exec(key, cx);
                                    this.start_exec(container.clone(), command.clone(), window, cx);
                                }))
                        })
                    }),
            )
            .children(error)
            .child(exec_menu::with_terminal_menu(
                div()
                    .id("exec-terminal")
                    .relative()
                    .test_support()
                    .track_focus(&self.exec.focus)
                    .tab_index(0)
                    .aria_label(active.map(|t| t.accessible.clone()).unwrap_or_default())
                    .flex_1()
                    .min_h(px(288.))
                    .bg(rgb(0x101113))
                    .text_color(rgb(0xf4f4f5))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .line_height(px(14.))
                    .px(px(10.))
                    .pt(px(8.))
                    .overflow_hidden()
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, event, window, cx| this.select_exec(event, window, cx)),
                    )
                    .map(|terminal| super::exec_drag::middle_paste(terminal, cx))
                    .on_mouse_move(
                        cx.listener(|this, event, _, cx| this.report_exec_motion(event, cx)),
                    )
                    .on_mouse_up(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseUpEvent, _, cx| {
                            if !event.modifiers.shift
                                && let Some(cell) = this.exec_cell(event.position)
                            {
                                this.exec_mouse(cell, 0, true, cx);
                            }
                        }),
                    )
                    .on_scroll(cx.listener(|this, event: &gpui_kit::ScrollEvent, _, cx| {
                        let delta =
                            (f32::from(event.delta.pixel_delta(px(14.)).y) / 14.).round() as isize;
                        if !event.modifiers.shift
                            && this.exec.active().is_some_and(|tab| {
                                tab.parser.screen().mouse_protocol_mode()
                                    != vt100::MouseProtocolMode::None
                            })
                            && let Some(cell) = this.exec_cell(event.position)
                        {
                            for _ in 0..delta.unsigned_abs() {
                                this.exec_mouse(cell, if delta > 0 { 64 } else { 65 }, false, cx);
                            }
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(tab) = this
                            .exec
                            .active
                            .and_then(|key| this.exec.tabs.iter_mut().find(|t| t.key == key))
                        {
                            let offset = tab.parser.screen().scrollback().saturating_add_signed(
                                if event.modifiers.alt {
                                    delta * 5
                                } else {
                                    delta
                                },
                            );
                            tab.parser.screen_mut().set_scrollback(offset);
                            tab.refresh_view();
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }))
                    .on_key_down(cx.listener(|this, event, _, cx| this.exec_key(event, cx)))
                    .children(active.into_iter().flat_map(|tab| {
                        let (shape, blink) =
                            super::exec_buffer::cursor_shape(tab.parser.screen().cursor_style());
                        let paint = super::exec_buffer::CursorPaint {
                            shape,
                            focused: self.exec.focused,
                            blink,
                            phase: tab.cursor_visible,
                        };
                        tab.rendered
                            .iter()
                            .map(move |row| super::exec_buffer::row_element(row, paint))
                    }))
                    .child(
                        gpui_kit::canvas(
                            |_, _, _| (),
                            move |bounds, (), window, cx| {
                                if let Some(key) = active_key
                                    && let Some(cols) = super::exec_resize::terminal_columns(
                                        f32::from(bounds.size.width),
                                        window,
                                    )
                                {
                                    // `podExecHelpers.ts` accepts terminal dimensions in 1..=500.
                                    let rows = ((f32::from(bounds.size.height) - 8.) / 14.)
                                        .floor()
                                        .clamp(1., 500.)
                                        as u16;
                                    if pending_start
                                        || current_size.map(|(r, c)| (r.get(), c.get()))
                                            != Some((rows, cols))
                                    {
                                        let owner = entity.clone();
                                        cx.defer(move |cx| {
                                            owner.update(cx, |this, cx| {
                                                this.resize_exec(key, cols, rows, cx)
                                            });
                                        });
                                    }
                                }
                                entity.update(cx, |this, _| this.exec.bounds = Some(bounds));
                                if dragging {
                                    // xterm listens on the document so a drag outside
                                    // the terminal still selects and autoscrolls.
                                    let owner = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &gpui_kit::MouseMoveEvent, phase, _, cx| {
                                            if phase == gpui_kit::DispatchPhase::Capture {
                                                owner.update(cx, |this, cx| {
                                                    this.drag_exec(event, cx)
                                                });
                                            }
                                        },
                                    );
                                    let owner = entity.clone();
                                    window.on_mouse_event(
                                        move |event: &gpui_kit::MouseUpEvent, phase, _, cx| {
                                            if phase == gpui_kit::DispatchPhase::Capture
                                                && event.button == gpui_kit::MouseButton::Left
                                            {
                                                owner.update(cx, |this, cx| {
                                                    this.end_exec_drag(event, cx)
                                                });
                                            }
                                        },
                                    );
                                }
                                window.handle_input(
                                    &focus,
                                    gpui_kit::ElementInputHandler::new(bounds, entity.clone()),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    ),
                menu_owner,
                has_selection,
            ))
            .child(menu)
            .into_any_element()
    }
}
