use super::surface;
use crate::model::health_source;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::topology::node::{Lifecycle, LifecycleTone};
use crate::ui;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, Div, Hsla, SharedString, Transformation, div, percentage,
    prelude::*, px,
};
use kubecove_lib::models::{HealthAssessmentCompleteness, ResourceSummary};
use std::time::Duration;

/// `ResourceTableStatus` for resources without a lifecycle: the compact
/// `HealthAssessmentBadge`. Its tooltip lists the state, source and raw status,
/// and a partial assessment adds the dashed marker.
pub(super) fn health_badge(row: &ResourceSummary) -> Div {
    let assessment = &row.health_assessment;
    let mut lines = vec![
        surface::health_state_label(assessment.state).to_owned(),
        health_source(&assessment.winning_sources),
    ];
    lines.extend(row.status.clone());
    let tooltip = lines.join("\n");
    let key = format!(
        "{}:{}:{}",
        row.kind,
        row.namespace.as_deref().unwrap_or(""),
        row.name
    );
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .id(SharedString::from(format!("health:{key}")))
                .test_support()
                .aria_description(tooltip.clone())
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .child(surface::health_state_badge(assessment.state)),
        )
        .when(
            assessment.completeness == HealthAssessmentCompleteness::Partial,
            |badge| {
                badge.child(
                    ui::outline_badge("Partial")
                        .border_dashed()
                        .id(SharedString::from(format!("partial:{key}")))
                        .test_support()
                        .aria_label("Partial"),
                )
            },
        )
}

/// `ResourceTableStatus.svelte`: a lifecycle label replaces the health badge
/// for workloads in progress or failed. Progress icons show activity only while
/// live updates are current; a paused row keeps its last observed state.
pub(super) fn lifecycle_status(
    id: SharedString,
    lifecycle: &Lifecycle,
    active: bool,
) -> impl IntoElement {
    let color = match lifecycle.tone {
        LifecycleTone::Info => Palette::sky300(),
        LifecycleTone::Warning => Palette::amber300(),
        LifecycleTone::Error => Tokens::destructive(),
    };
    let icon = match (lifecycle.progress, lifecycle.tone) {
        (true, _) => activity_icon(&id, active, color),
        (false, LifecycleTone::Warning) => ui::icon("pause", 12., color).into_any_element(),
        (false, _) => ui::icon("circle-alert", 12., color).into_any_element(),
    };
    div()
        .id(id)
        .test_support()
        .aria_label(lifecycle.label.clone())
        .flex()
        .items_center()
        .gap(px(6.))
        .min_w_0()
        .text_color(color)
        .when(lifecycle.progress && !active, |status| {
            status.aria_description(
                "Last observed state. Activity is paused until live updates recover.",
            )
        })
        .child(icon)
        .child(ui::xs(lifecycle.label.clone()).truncate())
}

/// `resource-activity-animation.ts` turns the arc once per 1150 ms
/// (`angle = (time / 1150) * Math.PI * 2`), on one clock shared by every row.
const ACTIVITY_TURN: Duration = Duration::from_millis(1150);

/// `ResourceActivityIcon.svelte`: an active row turns its arc; a paused row
/// paints it at angle 0. GPUI schedules frames only while an active arc is
/// rendered, and draws it static when `App::reduce_motion` is set, as the web
/// does under `prefers-reduced-motion`.
fn activity_icon(id: &SharedString, active: bool, color: Hsla) -> AnyElement {
    let icon = ui::icon("loader-circle", 12., color);
    let frame = div()
        .id(SharedString::from(format!("activity:{id}")))
        .test_support()
        .aria_description(if active { "active" } else { "paused" })
        .flex_none();
    if !active {
        return frame.child(icon).into_any_element();
    }
    frame
        .child(icon.with_animation(
            SharedString::from(format!("activity-turn:{id}")),
            Animation::new(ACTIVITY_TURN).repeat_synced(),
            |icon, turn| icon.with_transformation(Transformation::rotate(percentage(turn))),
        ))
        .into_any_element()
}
