//! Incident signal cards from `DetailsTab.svelte` and the timeline from
//! `ResourceTimeline.svelte`.

mod model;
pub(super) use model::State;

use super::Inspector;
use super::detail::ResourceDetail;
use super::events::{EventsLoad, badge, tone_colors, tw};
use super::windowed::Windowed;
use crate::incident::{self, SignalTone};
use crate::incident_timeline::{self as entries, TimelineItem};
use crate::theme::{MONO, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{Context, Div, FontWeight, Hsla, div, prelude::*, px};
use std::hash::{DefaultHasher, Hash, Hasher};

/// `incidentToneClass`: the `border-l-4` color and background.
fn signal_accent(tone: SignalTone) -> (Hsla, Hsla) {
    match tone {
        SignalTone::Error => (tw::red500(1.), tw::red500(0.05)),
        SignalTone::Warning => (tw::amber500(1.), tw::amber500(0.05)),
        SignalTone::Info => (tw::sky500(1.), tw::sky500(0.05)),
        SignalTone::Success => (tw::emerald500(1.), tw::emerald500(0.05)),
        SignalTone::Neutral => (Tokens::muted(), Tokens::background().opacity(0.5)),
    }
}

/// `mt-3 flex flex-col gap-2`: up to three signals, else the status line.
pub(super) fn incident_signals(detail: &ResourceDetail) -> Div {
    let summary = &detail.data.summary;
    let conditions = incident::condition_rows(detail.data.status.as_ref());
    let signals = incident::incident_signals(summary, &conditions, detail.events.events());
    let list = div().mt(px(12.)).flex().flex_col().gap(px(8.));
    if signals.is_empty() {
        let message = match detail.events.load {
            EventsLoad::Pending => "Checking events and status signals...",
            EventsLoad::Failed(_) => "Status loaded; event signals are unavailable.",
            EventsLoad::Ready(_) => "No active incident signals for this resource.",
        };
        return list.child(
            div()
                .px(px(12.))
                .py(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::background())
                .child(ui::muted(ui::xs(message))),
        );
    }
    list.children(signals.into_iter().take(3).map(|signal| {
        let (accent, background) = signal_accent(signal.tone);
        // `rounded-md border border-l-4 px-2.5 py-1.5`: GPUI borders take one
        // color, so a strip over the left edge draws the 4px accent.
        div()
            .id(gpui_kit::SharedString::from(format!(
                "incident-signal:{}",
                signal.label
            )))
            .test_support()
            .aria_label(signal.value.clone())
            .relative()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(8.))
            .pl(px(13.))
            .pr(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(background)
            .child(
                div()
                    .absolute()
                    .left(px(-1.))
                    .top(px(-1.))
                    .bottom(px(-1.))
                    .w(px(4.))
                    .rounded_l(px(8.))
                    .bg(accent),
            )
            .child(
                div()
                    .min_w_0()
                    .child(ui::semibold(ui::xs(signal.label)))
                    .child(ui::muted(ui::text(signal.value, 12., 16.5)).mt(px(4.))),
            )
            // `compactToneBadgeClass`: `rounded-full px-2 py-0 text-[0.6875rem]`.
            .child(badge(tone_colors(signal.tone), 11., None, signal.source))
    }))
}

/// `toneClass`: legend, dot and accent text.
fn tone_text(tone: SignalTone) -> Hsla {
    match tone {
        SignalTone::Error => Tokens::destructive(),
        SignalTone::Warning => tw::amber300(),
        SignalTone::Info => tw::sky300(),
        SignalTone::Neutral | SignalTone::Success => Tokens::foreground(),
    }
}

/// `cardToneClass`: border and background.
fn card_tone(tone: SignalTone) -> (Hsla, Hsla) {
    match tone {
        SignalTone::Error => (tw::red500(0.2), tw::red500(0.05)),
        SignalTone::Warning => (tw::amber500(0.2), tw::amber500(0.05)),
        SignalTone::Info => (tw::sky500(0.2), tw::sky500(0.05)),
        SignalTone::Neutral | SignalTone::Success => {
            (Tokens::border(), Tokens::background().opacity(0.3))
        }
    }
}

/// `text-[0.6875rem]` inheriting `text-xs/relaxed` (1.625).
fn small(content: impl Into<gpui_kit::SharedString>) -> Div {
    ui::text(content, 11., 17.875)
}

fn ghost_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    // `Button variant="ghost" size="sm"`: `h-6 px-2 text-xs/relaxed font-medium`.
    div()
        .id(id)
        .test_support()
        .cursor_pointer()
        .flex()
        .items_center()
        .h(px(24.))
        .px(px(8.))
        .rounded(px(8.))
        .child(ui::medium(ui::text(label, 12., 19.5)))
        .on_click(on_click)
}

fn evidence(item: &TimelineItem) -> gpui_kit::base::ObservedElement<gpui_kit::Stateful<Div>> {
    let row = |label: &str, value: String, mono: bool| {
        [
            ui::text(label.to_owned(), 11., 16.).text_color(Tokens::foreground().opacity(0.65)),
            ui::text(value, 11., 16.)
                .when(mono, |value| value.font_family(MONO))
                .when(!mono, |value| value.font_weight(FontWeight::MEDIUM)),
        ]
    };
    let mut rows = Vec::new();
    match item.title.split_once('=') {
        Some((condition, state)) if item.source == "condition" => {
            rows.push(row("Condition", condition.to_owned(), false));
            rows.push(row("State", state.to_owned(), false));
        }
        _ => rows.push(row("Reported as", item.title.clone(), false)),
    }
    rows.push(row(
        "Timestamp",
        item.timestamp.clone().unwrap_or("Not reported".into()),
        true,
    ));
    let has_detail = !item.detail.is_empty();
    // `grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5`; labels share a column.
    let (labels, values): (Vec<_>, Vec<_>) = rows.into_iter().map(|[l, v]| (l, v)).unzip();
    let list = div()
        .flex()
        .gap(px(12.))
        .when(has_detail, |list| {
            list.mt(px(12.))
                .pt(px(12.))
                .border_t_1()
                .border_color(Tokens::foreground().opacity(0.1))
        })
        .child(div().flex().flex_col().gap(px(6.)).children(labels))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .min_w_0()
                .children(values),
        );
    div()
        .mx(px(12.))
        .id(gpui_kit::SharedString::from(format!(
            "timeline-evidence:{}",
            item.key()
        )))
        .test_support()
        .aria_label("Timeline evidence")
        .pt(px(8.))
        .pb(px(12.))
        .border_t_1()
        .border_color(Tokens::foreground().opacity(0.1))
        .text_color(Tokens::foreground().opacity(0.9))
        .when(has_detail, |pane| {
            pane.child(ui::text(item.detail.clone(), 12., 19.5))
        })
        .child(list)
}

pub(super) const RAIL_EXTENSION: f32 = 10.;

fn entry(
    item: &TimelineItem,
    time: String,
    first: bool,
    last: bool,
    open: bool,
    resource_kind: &str,
    cx: &mut Context<Inspector>,
) -> Div {
    let tone = if item.source == "restart" {
        SignalTone::Warning
    } else {
        item.tone
    };
    let (border, background) = card_tone(tone);
    let accent = tone_text(tone);
    let key = item.key();
    let mut source = item.source.to_owned();
    source[..1].make_ascii_uppercase();
    // `before:` rail at `left-20`: from the first dot down, ending 28px into the last.
    let (top, bottom) = (
        if first { 28. } else { 0. },
        if last { None } else { Some(-RAIL_EXTENSION) },
    );
    let rail = div()
        .absolute()
        .left(px(80.))
        .top(px(top))
        .w(px(1.))
        .bg(Tokens::border().opacity(0.6));
    let rail = match bottom {
        Some(bottom) => rail.bottom(px(bottom)),
        None => rail.h(px(28.)),
    };
    // `grid-cols-[4rem_1rem_minmax(0,1fr)] grid-rows-[0.75rem_auto] gap-x-2`.
    div()
        .relative()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(rail)
        .child(
            div()
                .w(px(64.))
                .flex_none()
                .mt(px(12.))
                .h(px(32.))
                .flex()
                .items_center()
                .justify_end()
                .child(ui::muted(ui::text(time, 11., 20.)).font_family(MONO)),
        )
        .child(
            div()
                .w(px(16.))
                .flex_none()
                .mt(px(12.))
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(8.)).rounded_full().bg(accent)),
        )
        .child(
            // `fieldset rounded-lg border` with the source as its `legend`.
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .absolute()
                        .top(px(5.5))
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .rounded(px(10.))
                        .border_1()
                        .border_color(border)
                        .bg(background),
                )
                .child(
                    div().h(px(12.)).pl(px(7.)).flex().child(
                        div()
                            .relative()
                            .px(px(6.))
                            // The legend interrupts the top border.
                            .child(
                                div()
                                    .absolute()
                                    .top(px(5.5))
                                    .left_0()
                                    .right_0()
                                    .h(px(1.))
                                    .bg(Tokens::card().blend(background)),
                            )
                            .child(ui::medium(ui::text(source, 11., 12.)).text_color(accent)),
                    ),
                )
                .child(
                    div()
                        .id(gpui_kit::SharedString::from(format!(
                            "timeline-entry:{key}"
                        )))
                        .test_support()
                        .cursor_pointer()
                        .mx(px(1.))
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(6.))
                        .child({
                            let title = entries::timeline_title(item, resource_kind);
                            ui::medium(ui::text(title.clone(), 12., 20.))
                                .id(gpui_kit::SharedString::from(format!(
                                    "timeline-title:{key}"
                                )))
                                .flex_1()
                                .min_w_0()
                                .test_support()
                                .aria_label(title)
                        })
                        .child(
                            ui::icon(
                                if open {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                },
                                12.,
                                Tokens::foreground().opacity(0.6),
                            )
                            .mt(px(4.)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(events) = this.events_state() {
                                let open = events.is_open(&key);
                                events.overrides.insert(key.clone(), !open);
                            }
                            cx.notify();
                        })),
                )
                .when(open, |card| {
                    card.child(div().mx(px(1.)).child(evidence(item)))
                })
                .child(div().h(px(1.))),
        )
}

/// `ResourceTimeline.svelte`: `section px-1 py-2`.
pub(super) fn timeline(detail: &ResourceDetail, state: &State, cx: &mut Context<Inspector>) -> Div {
    let summary = &detail.data.summary;
    let model = state.model(detail);
    let items = &model.items;
    let groups = &model.groups;
    let hide_today = entries::hide_today_heading(groups.iter().map(|group| group.key.as_deref()));
    let geometry = if matches!(detail.events.load, EventsLoad::Pending) {
        Windowed::default()
    } else {
        state.rows.clone()
    };
    geometry.truncate(groups.len());
    let mut layout = DefaultHasher::new();
    detail.timeline_layout.hash(&mut layout);
    detail.events.layout_key.hash(&mut layout);
    hide_today.hash(&mut layout);
    for group in groups {
        group.label.hash(&mut layout);
        group.key.hash(&mut layout);
    }
    let layout = layout.finish();
    let scroll = f32::from(detail.scroll.offset().y);
    let count = format!(
        "{} {}",
        items.len(),
        if items.len() == 1 { "entry" } else { "entries" }
    );
    // `expandAll`: an explicit override for every current entry.
    let expand = |open: bool| {
        cx.listener(
            move |this: &mut Inspector, _: &gpui_kit::ClickEvent, _: &mut gpui_kit::Window, cx| {
                if let Some(keys) = this.timeline_keys()
                    && let Some(events) = this.events_state()
                {
                    events.overrides = keys.into_iter().map(|key| (key, open)).collect();
                }
                cx.notify();
            },
        )
    };
    let header = div()
        .mb(px(12.))
        .pb(px(8.))
        .border_b_1()
        .border_color(Tokens::border())
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::icon("clock", 14., Tokens::muted_foreground()))
                .child(ui::semibold(ui::xs("Timeline")))
                .child(ui::muted(small(count))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(ghost_button(
                    "timeline-expand-all",
                    "Expand all",
                    expand(true),
                ))
                .child(ghost_button(
                    "timeline-collapse-all",
                    "Collapse all",
                    expand(false),
                )),
        );
    let mut section = div()
        .id("incident-timeline")
        .test_support()
        .aria_label("Incident timeline")
        .px(px(4.))
        .py(px(8.))
        .child(header)
        .child(ui::muted(small("Oldest first")).mb(px(8.)));
    if items.is_empty() {
        return div().child(section.child(
            ui::muted(ui::xs("No incident timeline entries for this resource.")).py(px(16.)),
        ));
    }
    let entity = cx.entity_id();
    for (slot, group) in groups.iter().enumerate() {
        if !(hide_today && group.key.is_some()) {
            section = section.child(
                div()
                    .mt(px(12.))
                    .mb(px(4.))
                    .py(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::medium(ui::xs(group.label.clone())))
                    .child(div().h(px(1.)).flex_1().bg(Tokens::border().opacity(0.6))),
            );
        }
        let last = group.rows.len().saturating_sub(1);
        let ids = group
            .rows
            .iter()
            .map(|row| row.ids[usize::from(detail.events.is_open(&row.key))])
            .collect();
        // `flex-col gap-2.5`, as padding above every entry after the first.
        let list = geometry.scrolled_rows(slot, ids, layout, scroll, entity, |index| {
            let row = &group.rows[index];
            let open = detail.events.is_open(&row.key);
            div()
                .flex()
                .flex_col()
                .when(index > 0, |row| row.pt(px(10.)))
                .child(entry(
                    &items[row.item],
                    row.time.clone(),
                    index == 0,
                    index == last,
                    open,
                    &summary.kind,
                    cx,
                ))
                .into_any_element()
        });
        section = section.child(list);
    }
    div().child(section)
}

impl Inspector {
    /// Keys of the open resource's timeline entries, for Expand all and Collapse all.
    fn timeline_keys(&self) -> Option<Vec<String>> {
        let super::detail::Detail::Ready(detail) = &self.detail else {
            return None;
        };
        Some(
            self.timeline
                .model(detail)
                .groups
                .iter()
                .flat_map(|group| &group.rows)
                .map(|row| row.key.clone())
                .collect(),
        )
    }
}
