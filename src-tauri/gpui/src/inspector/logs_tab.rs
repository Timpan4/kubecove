//! Logs tab of the resource detail panel, mirroring `LogsTab.svelte`. Rows are
//! virtualized: the list renders only the rows in view, and `log_row` builds
//! each one from precomputed line text.

use super::Inspector;
use super::controls::{Variant, button, icon_button};
use super::detail::ResourceDetail;
use super::events::tw;
use super::events_tab::empty;
use super::log_row::{chip_colors, log_row, red500};
use super::logs_state::{LogStatus, LogsState, SINCE};
use crate::logs::LogLevel;
use crate::preferences::LogLayout;
use crate::theme::{MONO, Tokens};
use crate::ui;
use gpui_kit::component::{input::Input, select::Select, tooltip::Tooltip};
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, Stateful, TestSupportExt, deferred,
    div, list, prelude::*, px,
};

fn status_dot(logs: &LogsState) -> Hsla {
    if logs.paused {
        tw::amber500(1.)
    } else {
        match logs.status {
            LogStatus::Connected => tw::emerald500(1.),
            LogStatus::Error => red500(1.),
            _ => Tokens::muted_foreground().opacity(0.5),
        }
    }
}

fn tail_menu(logs: &LogsState, cx: &mut Context<Inspector>) -> Stateful<Div> {
    let label = |text: &str| ui::muted(ui::xs(text.to_owned()));
    let segments = div()
        .flex()
        .items_center()
        .p(px(2.))
        .rounded(px(8.))
        .bg(Tokens::background())
        .border_1()
        .border_color(Tokens::border().opacity(0.6))
        .children(SINCE.iter().map(|(seconds, _, short)| {
            let active = logs.since == *seconds;
            let seconds = *seconds;
            div()
                .id(SharedString::from(format!("log-since-{short}")))
                .test_support()
                .cursor_pointer()
                .flex()
                .items_center()
                .h(px(24.))
                .px(px(8.))
                .rounded(px(6.))
                .when(active, |button| button.bg(Tokens::card()).shadow_sm())
                .child(ui::medium(ui::xs(*short)).text_color(if active {
                    Tokens::foreground()
                } else {
                    Tokens::muted_foreground()
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(logs) = this.logs_state()
                        && logs.since != seconds
                    {
                        logs.since = seconds;
                        this.restart_logs(window, cx);
                    }
                }))
        }));
    div()
        .id("log-tail-menu")
        .occlude()
        .absolute()
        .top(px(32.))
        .right_0()
        .w(px(240.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .shadow_md()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
            if let Some(logs) = this.logs_state() {
                logs.tail_menu_open = false;
            }
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(label("Tail lines"))
                .child(
                    Input::new(&logs.tail_input)
                        .id("log-tail-lines")
                        .aria_label("Tail log lines")
                        .text_xs()
                        .max_h(px(28.))
                        .w(px(96.)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(label("Since"))
                .child(segments),
        )
}

fn toolbar(logs: &LogsState, cx: &mut Context<Inspector>) -> Div {
    // The label shrinks and ends in an ellipsis, as the row runs out of width; the full text stays on hover.
    let status_label = logs.status_label();
    let status_title = format!("{status_label} · {} lines", logs.buffer.len());
    let status = div()
        .id("log-status")
        .test_support()
        .aria_label(status_title.clone())
        .tooltip(move |window, cx| Tooltip::new(status_title.clone()).build(window, cx))
        .flex()
        .min_w_0()
        .overflow_hidden()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .child(if logs.connecting() {
            ui::icon("loader-circle", 12., Tokens::muted_foreground()).into_any_element()
        } else {
            div()
                .size(px(7.))
                .flex_none()
                .rounded_full()
                .bg(status_dot(logs))
                .into_any_element()
        })
        .child(ui::xs(status_label).min_w_0().truncate())
        .child(ui::muted(
            ui::xs(format!("· {} lines", logs.buffer.len())).min_w_0().truncate(),
        ));
    let tail = div()
        .relative()
        .child(
            button(Variant::Outline, 28., false)
                .id("log-tail")
                .test_support()
                .aria_label("Log tail and time window")
                .pr(px(6.))
                .font_weight(FontWeight::NORMAL)
                .child(logs.tail_label())
                .child(ui::icon("chevron-down", 14., Tokens::foreground()))
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(logs) = this.logs_state() {
                        logs.tail_menu_open = !logs.tail_menu_open;
                    }
                    cx.notify();
                })),
        )
        .when(logs.tail_menu_open, |wrapper| {
            wrapper.child(deferred(tail_menu(logs, cx)).with_priority(1))
        });
    let (variant, icon, text) = if logs.paused {
        (Variant::Default, "play", "Resume")
    } else {
        (Variant::Outline, "pause", "Pause")
    };
    let icon_color = if logs.paused {
        Tokens::primary_foreground()
    } else {
        Tokens::foreground()
    };
    // One line, as in `LogsTab.svelte`: the status label gives up width, so nothing draws over the controls.
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .when_some(logs.container_picker.as_ref(), |row, picker| {
            // `Select` styles only its inner box; its root is `size_full`, and its title has no
            // minimum width, so as a bare flex item it shrinks to the chevron. This wrapper is the
            // flex item instead: as wide as the selected name, at most the web's `w-40`.
            row.child(
                div().flex_none().max_w(px(160.)).child(
                    Select::new(picker)
                        .id("log-container")
                        .h(px(28.))
                        .text_xs(),
                ),
            )
        })
        .child(status)
        .child(div().flex_1())
        .child(tail)
        .child(
            button(variant, 28., false)
                .id("log-pause")
                .test_support()
                .pl(px(6.))
                .child(ui::icon(icon, 14., icon_color))
                .child(text)
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(logs) = this.logs_state() {
                        logs.toggle_pause();
                    }
                    cx.notify();
                })),
        )
}

fn filters(logs: &LogsState, layout: LogLayout, cx: &mut Context<Inspector>) -> Div {
    let counts = logs.buffer.counts();
    let toggle = |shown: bool| {
        if shown {
            Tokens::foreground()
        } else {
            Tokens::muted_foreground()
        }
    };
    let chips = logs.buffer.has_levels().then(|| {
        div()
            .flex()
            .gap(px(4.))
            .children(LogLevel::ALL.iter().map(|level| {
                let level = *level;
                let shown = !logs.hidden[level.index()];
                let (border, background, foreground) = if shown {
                    chip_colors(level)
                } else {
                    (
                        Tokens::border(),
                        Tokens::input().opacity(0.3),
                        Tokens::muted_foreground().opacity(0.7),
                    )
                };
                button(Variant::Outline, 28., false)
                    .id(SharedString::from(format!("log-level-{}", level.label())))
                    .test_support()
                    .aria_label(level.label())
                    .gap(px(6.))
                    .px(px(6.))
                    .border_color(border)
                    .bg(background)
                    .text_color(foreground)
                    .font_family(MONO)
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(level.label())
                    .child(
                        div()
                            .font_weight(FontWeight::NORMAL)
                            .opacity(0.85)
                            .child(counts[level.index()].to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(logs) = this.logs_state() {
                            logs.hidden[level.index()] = !logs.hidden[level.index()];
                            logs.refilter();
                        }
                        cx.notify();
                    }))
            }))
    });
    let count = logs.filtering().then(|| {
        ui::muted(ui::text(
            format!("{}/{}", logs.visible.len(), logs.buffer.len()),
            11.,
            16.,
        ))
    });
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .child(
            // `flex-[1_1_12rem]`.
            div()
                .flex_grow(1.)
                .flex_shrink(1.)
                .flex_basis(px(192.))
                .child(
                    Input::new(&logs.filter)
                        .id("log-filter")
                        .aria_label("Filter log lines")
                        .prefix(ui::icon("search", 12., Tokens::muted_foreground()))
                        .when_some(count, |input, count| input.suffix(count))
                        .bg(Tokens::input().opacity(0.3))
                        .text_xs()
                        .max_h(px(28.))
                        .w_full(),
                ),
        )
        .child(
            div()
                .ml_auto()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .children(chips)
                // Only the one-line layout has rows to expand, as in `LogsTab.svelte`.
                .children((layout == LogLayout::OneLine).then(|| {
                    icon_button("chevrons-up-down", toggle(logs.expand_all))
                        .id("log-expand-all")
                        .test_support()
                        .aria_label(if logs.expand_all {
                            "Collapse all"
                        } else {
                            "Expand all"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(logs) = this.logs_state() {
                                logs.toggle_expand_all();
                                logs.list.remeasure();
                            }
                            cx.notify();
                        }))
                }))
                // Wrap applies only to the other layouts, as in `LogsTab.svelte`.
                .when(layout != LogLayout::OneLine, |row| {
                    row.child(
                        icon_button("wrap-text", toggle(logs.wrap))
                            .id("log-wrap")
                            .test_support()
                            .aria_label("Wrap lines")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(logs) = this.logs_state() {
                                    logs.wrap = !logs.wrap;
                                    logs.list.reset(logs.visible.len());
                                }
                                cx.notify();
                            })),
                    )
                })
                .child(
                    icon_button("arrow-down-up", toggle(logs.latest_first))
                        .id("log-order")
                        .test_support()
                        .aria_label(if logs.latest_first {
                            "Latest first"
                        } else {
                            "Oldest first"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(logs) = this.logs_state() {
                                let latest_first = !logs.latest_first;
                                logs.set_latest_first(latest_first);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    icon_button("rotate-ccw", Tokens::muted_foreground())
                        .id("log-clear")
                        .test_support()
                        .aria_label("Clear")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(logs) = this.logs_state() {
                                logs.clear();
                            }
                            cx.notify();
                        })),
                ),
        )
}

fn viewport(logs: &LogsState, cx: &mut Context<Inspector>) -> impl IntoElement {
    let frame = div()
        .id("log-viewport")
        .test_support()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.5));
    let body = if logs.buffer.is_empty() {
        let text = if logs.paused {
            "Paused. New lines are held until you resume.".to_owned()
        } else if logs.message.is_empty() {
            "Waiting for log lines...".to_owned()
        } else {
            logs.message.clone()
        };
        div()
            .id("log-waiting")
            .test_support()
            .aria_label(text.clone())
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(12.))
            .when(logs.connecting(), |row| {
                row.child(ui::icon("loader-circle", 14., Tokens::muted_foreground()))
            })
            .child(ui::muted(ui::sm(text)))
            .into_any_element()
    } else if logs.visible.is_empty() {
        ui::muted(ui::sm("No log lines match the current filters."))
            .id("log-no-match")
            .test_support()
            .aria_label("No log lines match the current filters.")
            .p(px(12.))
            .into_any_element()
    } else {
        list(
            logs.list.clone(),
            cx.processor(|this: &mut Inspector, position: usize, _, cx| {
                let layout = this.persisted.preferences.log_layout;
                let Some(logs) = this.logs_state() else {
                    return div().into_any_element();
                };
                let line = logs
                    .visible
                    .get(position)
                    .and_then(|index| logs.buffer.line(*index));
                match line {
                    Some(line) => log_row(line, logs, position, layout, cx).into_any_element(),
                    None => div().into_any_element(),
                }
            }),
        )
        .size_full()
        .into_any_element()
    };
    let jump = (logs.unseen > 0).then(|| {
        let count = logs.unseen;
        let text = format!("{count} new line{}", if count == 1 { "" } else { "s" });
        div()
            .absolute()
            .left_0()
            .right_0()
            .when(logs.latest_first, |row| row.top(px(12.)))
            .when(!logs.latest_first, |row| row.bottom(px(12.)))
            .flex()
            .justify_center()
            .child(
                button(Variant::Default, 28., false)
                    .id("log-jump")
                    .test_support()
                    .aria_label(text.clone())
                    .rounded_full()
                    .px(px(12.))
                    .shadow_lg()
                    .child(ui::icon(
                        if logs.latest_first {
                            "arrow-up"
                        } else {
                            "arrow-down"
                        },
                        14.,
                        Tokens::primary_foreground(),
                    ))
                    .child(text)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(logs) = this.logs_state() {
                            logs.jump_to_latest();
                        }
                        cx.notify();
                    })),
            )
    });
    frame.child(body).children(jump)
}

pub(super) fn logs_tab(
    detail: &ResourceDetail,
    layout: LogLayout,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let Some(logs) = detail.logs.as_ref() else {
        return div().into_any_element();
    };
    if detail.data.summary.kind == "Pod" && logs.containers.is_empty() {
        return empty(
            "No containers found",
            "Container status has not loaded for this Pod yet.",
            128.,
        )
        .id("logs-empty")
        .test_support()
        .aria_label("No containers found")
        .into_any_element();
    }
    let error = logs.error.as_ref().map(|error| {
        div()
            .id("log-error")
            .test_support()
            .aria_label(format!("Log stream failed: {error}"))
            .aria_description(error.clone())
            .role(gpui_kit::Role::Alert)
            .flex_none()
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::destructive().opacity(0.3))
            .bg(Tokens::destructive().opacity(0.1))
            .child(ui::semibold(ui::xs("Log stream failed")))
            .child(ui::muted(ui::xs(error.clone())).mt(px(4.)))
    });
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(px(6.))
        .child(toolbar(logs, cx))
        .child(filters(logs, layout, cx))
        .children(error)
        .child(viewport(logs, cx))
        .into_any_element()
}
