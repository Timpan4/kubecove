//! Events tab of the resource detail panel, mirroring `EventsTab.svelte` at
//! its `xl:` layout (the capture viewport is 1440px wide).

use super::detail::ResourceDetail;
use super::events::{EventsLoad, badge, outline, tw};
use super::loading::LoadingDelays;
use super::windowed::row_id;
use super::{Inspector, timestamp_text};
use crate::incident::{self, EventFilter};
use crate::model::exact_timestamp;
use crate::preferences::TimestampTimezone;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, HighlightStyle, SharedString, StyledText, div,
    prelude::*, px,
};
use kubecove_lib::models::ResourceEventSummary;

const FILTERS: &[(EventFilter, &str, &str)] = &[
    (EventFilter::All, "events-filter-all", "All"),
    (EventFilter::Warning, "events-filter-warning", "Warnings"),
    (EventFilter::Normal, "events-filter-normal", "Normal"),
];

fn warning(event: &ResourceEventSummary) -> bool {
    event.event_type.to_lowercase() == "warning"
}

fn source(event: &ResourceEventSummary) -> String {
    match event.source.trim() {
        "" => "unknown".into(),
        source => source.to_owned(),
    }
}

/// `eventAgeLabel`.
fn age(event: &ResourceEventSummary) -> String {
    match event.last_seen.trim() {
        "" => "-".into(),
        value if value.ends_with("ago") => value.to_owned(),
        value => format!("{value} ago"),
    }
}

/// `formatEventCompactTime`: `formatExactTimeOnly(..., "second")` in the saved
/// timezone, as the exact-time tooltip uses, else the relative label.
fn compact_time(event: &ResourceEventSummary, timezone: TimestampTimezone) -> String {
    let Some(time) = incident::parse(event.last_seen_at.as_deref()) else {
        return event.last_seen.clone();
    };
    match timezone {
        TimestampTimezone::Local => time
            .with_timezone(&chrono::Local)
            .format("%H:%M:%S")
            .to_string(),
        TimestampTimezone::Utc => time
            .with_timezone(&chrono::Utc)
            .format("%H:%M:%S UTC")
            .to_string(),
    }
}

/// `Empty class="border border-dashed"` with a title and description.
pub(super) fn empty(
    title: &str,
    description: &str,
    height: f32,
) -> crate::targets::ObservedElement<gpui_kit::Stateful<Div>> {
    div()
        .id(SharedString::from(format!("empty:{title}")))
        .test_support()
        .aria_label(title.to_owned())
        .min_h(px(height))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .p(px(24.))
        .rounded(px(10.))
        .border_1()
        .border_dashed()
        .border_color(Tokens::border())
        .child(ui::medium(ui::sm(title.to_owned())))
        .child(ui::muted(ui::xs(description.to_owned())))
}

/// `rounded-md border bg-background/40 p-2` summary tile.
fn tile(label: &str, value: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .p(px(8.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.4))
        .child(
            // `text-[10px] font-medium uppercase tracking-wide`, inheriting `/relaxed`.
            ui::muted(ui::text(label.to_uppercase(), 10., 16.25))
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap(),
        )
        .child(div().mt(px(4.)).min_w_0().child(value))
}

/// `mt-1 truncate text-sm font-semibold`.
fn tile_value(text: impl Into<SharedString>) -> Div {
    ui::semibold(ui::sm(text)).truncate()
}

/// `formatEventExactTime`: the exact time in the saved timezone, or the raw
/// relative label when the event has no timestamp.
fn exact_time(event: &ResourceEventSummary, timezone: TimestampTimezone) -> String {
    event
        .last_seen_at
        .as_deref()
        .and_then(|value| exact_timestamp(value, timezone))
        .unwrap_or_else(|| event.last_seen.clone())
}

fn event_row(event: &ResourceEventSummary, index: usize, timezone: TimestampTimezone) -> Div {
    let warning = warning(event);
    let (dot, ring) = if warning {
        (tw::amber400(1.), tw::amber400(0.12))
    } else {
        (tw::emerald400(1.), tw::emerald400(0.12))
    };
    let tone = if warning {
        (tw::amber500(0.4), tw::amber500(0.1), tw::amber200())
    } else {
        (tw::emerald500(0.3), tw::emerald500(0.1), tw::emerald200())
    };
    // `grid-cols-[5.75rem_0.75rem_minmax(0,1fr)_7rem] gap-2 px-3 py-2`.
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .child(
            div()
                .w(px(92.))
                .flex_none()
                .child(timestamp_text::with_tooltip(
                    format!("event-time:{index}"),
                    Some(exact_time(event, timezone)),
                    ui::muted(ui::xs(compact_time(event, timezone))).whitespace_nowrap(),
                )),
        )
        .child(
            // `mt-1.5 size-2 rounded-full` with a 3px `shadow` ring.
            div().w(px(12.)).flex_none().h(px(14.)).relative().child(
                div()
                    .absolute()
                    .left(px(-3.))
                    .top(px(3.))
                    .size(px(14.))
                    .rounded_full()
                    .bg(ring)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().size(px(8.)).rounded_full().bg(dot)),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(ui::medium(ui::text(event.message.clone(), 12., 19.5)))
                .child(
                    div()
                        .mt(px(6.))
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(badge(tone, 10., None, event.event_type.clone()))
                        .child(badge(outline(), 10., None, event.reason.clone()))
                        .child(badge(
                            outline(),
                            10.,
                            None,
                            format!("count {}", event.count),
                        ))
                        .child(badge(outline(), 10., None, source(event)))
                        .child(badge(
                            outline(),
                            10.,
                            None,
                            event.namespace.clone().unwrap_or("cluster".into()),
                        )),
                ),
        )
        .child(
            div()
                .w(px(112.))
                .flex_none()
                .min_w_0()
                .flex()
                .flex_col()
                .items_end()
                .child(ui::medium(ui::xs(source(event))).truncate())
                .child(ui::muted(ui::xs(age(event))).truncate()),
        )
}

pub(super) fn events_tab(
    detail: &ResourceDetail,
    timezone: TimestampTimezone,
    loading: &LoadingDelays,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let state = &detail.events;
    let phase = loading.phase("events", matches!(state.load, EventsLoad::Pending));
    let events = match &state.load {
        // EventsTab.svelte checks the loading state before the error.
        _ if !phase.ready() => {
            return div()
                .min_h(px(128.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .when(phase.shown(), |slot| {
                    slot.child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
                        .child(ui::muted(ui::xs("Loading events")))
                })
                .into_any_element();
        }
        EventsLoad::Pending => return div().into_any_element(),
        EventsLoad::Failed(error) => {
            return div()
                .id("events-error")
                .role(gpui_kit::Role::Alert)
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::destructive().opacity(0.3))
                .bg(Tokens::destructive().opacity(0.1))
                .child(ui::semibold(ui::xs("Failed to load events")))
                .child(ui::muted(ui::xs(error.clone())).mt(px(4.)))
                .into_any_element();
        }
        EventsLoad::Ready(events) if events.is_empty() => {
            return empty(
                "No events",
                "No Kubernetes events returned for this resource.",
                128.,
            )
            .into_any_element();
        }
        EventsLoad::Ready(events) => events,
    };
    let summary = incident::event_summary(events);
    let query = state.search.read(cx).value().to_string();
    let visible = incident::filter_events(events, state.filter, &query);
    let latest = summary.latest.map_or(tile_value("-"), |event| {
        // The reason and its `ml-1 text-xs font-normal` time truncate as one line.
        let reason = event.reason.clone();
        let text = format!("{reason} {}", compact_time(event, timezone));
        let time = reason.len() + 1..text.len();
        ui::semibold(div().text_size(px(14.)).line_height(px(20.)))
            .truncate()
            .child(StyledText::new(text).with_highlights(vec![(
                time,
                HighlightStyle {
                    color: Some(Tokens::muted_foreground()),
                    font_weight: Some(FontWeight::NORMAL),
                    ..Default::default()
                },
            )]))
    });
    let warnings = tile_value(summary.warnings.to_string()).text_color(if summary.warnings > 0 {
        tw::amber200()
    } else {
        tw::emerald200()
    });
    let normal = summary.total - summary.warnings;
    let segmented = div()
        .flex()
        .flex_none()
        .items_center()
        // `ring-1` draws outside the box, so the border sits in a -1px margin.
        .m(px(-1.))
        .p(px(2.))
        .rounded(px(10.))
        .bg(Tokens::background())
        .border_1()
        .border_color(Tokens::border().opacity(0.6))
        .children(FILTERS.iter().map(|(filter, id, label)| {
            let active = state.filter == *filter;
            let filter = *filter;
            div()
                .id(*id)
                .test_support()
                .cursor_pointer()
                .flex()
                .items_center()
                .h(px(24.))
                .px(px(8.))
                .rounded(px(8.))
                .when(active, |button| button.bg(Tokens::card()).shadow_sm())
                .child(ui::medium(ui::xs(*label)).text_color(if active {
                    Tokens::foreground()
                } else {
                    Tokens::muted_foreground()
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(events) = this.events_state() {
                        events.filter = filter;
                    }
                    cx.notify();
                }))
        }));
    let list = if visible.is_empty() {
        empty(
            "No matching events",
            "Adjust the search text or event type filter.",
            112.,
        )
        .into_any_element()
    } else {
        let last = visible.len() - 1;
        // The message and badges wrap; the time, dot and source columns are fixed.
        let ids = visible
            .iter()
            .enumerate()
            .map(|(index, event)| {
                row_id((
                    &event.message,
                    &event.event_type,
                    &event.reason,
                    event.count,
                    &event.source,
                    &event.namespace,
                    index == last,
                ))
            })
            .collect();
        let rows = state.rows.rows(0, ids, cx.entity_id(), |index| {
            let event = visible[index];
            event_row(event, index, timezone)
                .id(SharedString::from(format!("event:{index}")))
                .test_support()
                .aria_label(format!("{} {}", event.reason, event.message))
                .when(index < last, |row| {
                    row.border_b_1().border_color(Tokens::border())
                })
                .into_any_element()
        });
        div()
            .id("events-list")
            .test_support()
            .min_w_0()
            .overflow_hidden()
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::background().opacity(0.3))
            .child(rows)
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(tile(
                    "Total",
                    tile_value(format!("{} events", summary.total)),
                ))
                .child(tile("Warnings", warnings))
                .child(tile("Latest", latest))
                .child(tile("Sources", tile_value(summary.sources.to_string())))
                .child(tile("Namespace", tile_value(summary.namespace.clone())))
                .id("events-summary")
                .test_support()
                .aria_label(format!(
                    "Total {} events; Warnings {}; Sources {}; Namespace {}",
                    summary.total, summary.warnings, summary.sources, summary.namespace
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&state.search)
                            .id("events-search")
                            .aria_label("Filter resource events")
                            .prefix(ui::icon("search", 14., Tokens::muted_foreground()))
                            .bg(Tokens::input().opacity(0.3))
                            .text_xs()
                            // `h-7`: GPUI Kit inputs are 24px or 32px, so cap the medium size.
                            .max_h(px(28.))
                            .w_full(),
                    ),
                )
                .child(segmented),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .child(badge(
                    (
                        tw::emerald500(0.3),
                        Tokens::input().opacity(0.3),
                        tw::emerald200(),
                    ),
                    10.,
                    Some(tw::emerald400(1.)),
                    format!("Normal {normal}"),
                ))
                .child(badge(
                    (
                        tw::amber500(0.4),
                        Tokens::input().opacity(0.3),
                        tw::amber200(),
                    ),
                    10.,
                    Some(tw::amber400(1.)),
                    format!("Warnings {}", summary.warnings),
                ))
                .child(badge(
                    outline(),
                    10.,
                    None,
                    format!("Sources {}", summary.sources),
                ))
                .child(badge(outline(), 10., None, summary.namespace.clone())),
        )
        .child(list)
        .into_any_element()
}
