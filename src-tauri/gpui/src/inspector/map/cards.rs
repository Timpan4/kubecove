//! Map cards: `OwnershipResourceNode.svelte`, `StandaloneKindGroupNode.svelte`
//! and the selection ring. `z` is the flow zoom every length is scaled by.

use crate::model::kind_visual;
use crate::theme::{Palette, Tokens, mix, oklch};
use crate::topology::FlowNode;
use crate::topology::node::{self, LifecycleTone, Rail, Tone};
use crate::ui;
use gpui_kit::{
    BoxShadow, Div, FontWeight, Hsla, SharedString, div, linear_color_stop, linear_gradient, point,
    prelude::*, px,
};
use kubecove_lib::models::TopologyNode;

fn tone_color(tone: Tone) -> Hsla {
    match tone {
        Tone::Success => oklch(0.78, 0.13, 164., 1.),
        Tone::Warning => oklch(0.769, 0.188, 70.08, 1.),
        Tone::Error => Tokens::destructive(),
        Tone::Neutral => Tokens::muted_foreground(),
    }
}

fn scaled_text(content: impl Into<SharedString>, size: f32, z: f32) -> Div {
    ui::text(content, size * z, size * z).whitespace_nowrap()
}

pub(super) fn resource_card(
    node: &FlowNode,
    resource: &TopologyNode,
    z: f32,
    network: bool,
    metric: Option<&kubecove_lib::models::ResourceMetricSummary>,
) -> Div {
    let s = |value: f32| px(value * z);
    let (icon, tone) = kind_visual(&resource.kind);
    let mut card = node::card(resource);
    if network && card.lifecycle.is_none() && !resource.port_hints.is_empty() {
        card.status = resource
            .port_hints
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
    }
    if let Some(metric) = metric
        && let Some(hint) = crate::metrics::compact(metric)
    {
        card.meta.push_str(&format!(" · {hint}"));
    }
    let rail = match card.rail {
        Rail::Info => Palette::sky300(),
        Rail::Tone(tone) => tone_color(tone),
    };
    let lifecycle_color = |tone: LifecycleTone| match tone {
        LifecycleTone::Info => Palette::sky300(),
        LifecycleTone::Warning => Palette::amber300(),
        LifecycleTone::Error => Tokens::destructive(),
    };
    let status_color = match card.lifecycle.as_ref().map(|lifecycle| lifecycle.tone) {
        Some(LifecycleTone::Info) => Palette::sky300(),
        Some(LifecycleTone::Error) => Tokens::destructive(),
        _ => Tokens::foreground().opacity(0.85),
    };
    let marker = match &card.lifecycle {
        Some(lifecycle) => ui::icon(
            match (lifecycle.progress, lifecycle.tone) {
                (true, _) => "loader-circle",
                (false, LifecycleTone::Warning) => "pause",
                (false, _) => "circle-alert",
            },
            12. * z,
            lifecycle_color(lifecycle.tone),
        )
        .into_any_element(),
        None => {
            let chip = tone_color(card.dot);
            div()
                .flex_none()
                .size(s(8.))
                .rounded_full()
                .bg(chip)
                .shadow(vec![BoxShadow {
                    color: chip.opacity(0.62),
                    offset: point(px(0.), px(0.)),
                    blur_radius: s(8.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .into_any_element()
        }
    };
    let restart = card.restarts.map(|text| {
        let chip = tone_color(card.restart_tone);
        div()
            .ml_auto()
            .flex()
            .flex_none()
            .items_center()
            .h(s(20.))
            .max_w(s(138.))
            .px(s(8.))
            .rounded(s(6.))
            .border(s(1.))
            .border_color(chip.opacity(0.36))
            .bg(chip.opacity(0.1))
            .child(
                scaled_text(text, 11., z)
                    .font_weight(FontWeight::BOLD)
                    .text_color(mix(chip, Tokens::foreground(), 0.88))
                    .truncate(),
            )
    });
    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .overflow_hidden()
        .rounded(s(8.))
        .border(s(1.))
        .border_color(mix(tone, Tokens::border(), 0.4))
        .bg(linear_gradient(
            135.,
            linear_color_stop(mix(tone, Tokens::surface2(), 0.15), 0.),
            linear_color_stop(mix(tone, Tokens::card(), 0.05), 1.),
        ))
        .pt(s(10.))
        .pb(s(10.))
        .pl(s(16.))
        .pr(s(14.))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(s(4.))
                .bg(rail.opacity(0.96)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(s(8.))
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(s(24.))
                        .rounded(s(8.))
                        .border(s(1.))
                        .border_color(tone.opacity(0.32))
                        .bg(tone.opacity(0.19))
                        .child(ui::icon(icon, 14. * z, tone)),
                )
                .child(
                    scaled_text(card.name, 14., z)
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::foreground()),
                )
                .child(
                    scaled_text(card.kind_label, 10., z)
                        .flex_none()
                        .max_w(s(92.))
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(mix(tone, Tokens::foreground(), 0.78)),
                ),
        )
        .child(
            scaled_text(card.meta, 12., z)
                .mt(s(4.))
                .pl(s(32.))
                .truncate()
                .text_color(Tokens::muted_foreground()),
        )
        .child(
            div()
                .mt_auto()
                .ml(s(32.))
                .flex()
                .items_center()
                .gap(s(6.))
                .min_w_0()
                .border_t(s(1.))
                .border_color(Tokens::border().opacity(0.35))
                .pt(s(6.))
                .child(marker)
                .child(
                    scaled_text(card.status, 12., z)
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(status_color),
                )
                .when_some(card.ready, |footer, ready| {
                    footer
                        .child(
                            scaled_text("•", 12., z)
                                .flex_none()
                                .text_color(Tokens::muted_foreground()),
                        )
                        .child(
                            scaled_text(ready, 12., z)
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::foreground().opacity(0.85)),
                        )
                })
                .children(restart),
        )
        .when(node.dimmed, |card| card.opacity(0.35))
}

pub(super) fn group_card(kind: &str, count: usize, expanded: bool, dimmed: bool, z: f32) -> Div {
    let s = |value: f32| px(value * z);
    let (icon, tone) = kind_visual(kind);
    div()
        .relative()
        .size_full()
        .rounded(s(8.))
        .border(s(1.))
        .border_dashed()
        .border_color(mix(tone, Tokens::border(), 0.62))
        .bg(mix(tone, Tokens::card(), 0.2))
        .px(s(12.))
        .py(s(8.))
        .child(
            div()
                .absolute()
                .top(s(-16.))
                .left(gpui_kit::relative(0.5))
                .h(s(16.))
                .border_l(s(1.))
                .border_dashed()
                .border_color(Tokens::muted_foreground().opacity(0.35)),
        )
        .child(
            div()
                .flex()
                .size_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap(s(12.))
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(s(8.))
                        .child(ui::icon(
                            if expanded {
                                "chevron-down"
                            } else {
                                "chevron-right"
                            },
                            12. * z,
                            Tokens::muted_foreground(),
                        ))
                        .child(ui::icon(icon, 14. * z, tone))
                        .child(
                            scaled_text(format!("Ownerless {kind}"), 12., z)
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::foreground()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(s(20.))
                        .px(s(8.))
                        .rounded_full()
                        .border(s(1.))
                        .border_color(Tokens::border())
                        .child(
                            scaled_text(count.to_string(), 10., z).font_weight(FontWeight::MEDIUM),
                        ),
                ),
        )
        .when(dimmed, |card| card.opacity(0.35))
}

/// The selection ring (`ring-2`/`ring-1`) and glow (`::after` box shadow).
pub(super) fn selection_ring(node: &FlowNode, z: f32) -> Option<Div> {
    let (width, ring, glow, blur) = if node.selected {
        (2., Tokens::primary(), 0.42, 10.)
    } else if node.connected {
        (1., Tokens::primary().opacity(0.3), 0.3, 7.)
    } else {
        return None;
    };
    // The ring is drawn inside the zoomed flow viewport, so it scales too.
    let width = width * z;
    Some(
        div()
            .absolute()
            .top(px(-width))
            .left(px(-width))
            .right(px(-width))
            .bottom(px(-width))
            .rounded(px(8. * z + width))
            .border(px(width))
            .border_color(ring)
            .shadow(vec![BoxShadow {
                color: Tokens::primary().opacity(glow),
                offset: point(px(0.), px(0.)),
                blur_radius: px(blur * z),
                spread_radius: px(0.),
                inset: false,
            }]),
    )
}
