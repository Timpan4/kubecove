//! Pieces shared by the Incidents and Port Forwards pages: `SurfaceFrame.svelte`,
//! small buttons, `HealthAssessmentBadge.svelte`, and the page read dispatch.

use super::{Inspector, Page};
use crate::model::health_source;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, Styled, div, prelude::*, px,
};
use kubecove_lib::models::{HealthAssessment, HealthAssessmentCompleteness, HealthAssessmentState};

impl Inspector {
    /// Pages with their own reads load when shown and when the scope changes.
    /// Live sessions load on every page because the header shows their count.
    pub(super) fn load_page(&mut self, cx: &mut Context<Self>) {
        self.sync_page_reads(cx);
        self.ensure_sessions(cx);
        match self.page {
            Page::GitOps => self.refresh_focused_page(cx),
            Page::Incidents => self.load_incidents(cx),
            Page::Helm => self.load_helm(cx),
            Page::Namespaces => self.load_namespace_list(cx),
            Page::Resources => self.load_application_workspace(cx),
            _ => {}
        }
        if self.page == Page::Rbac {
            self.load_rbac(cx);
        }
    }
}

/// `SurfaceFrame.svelte`. `wide` drops the `max-w-7xl` cap and uses `px-5`.
pub(super) fn frame(icon: &str, title: &str, wide: bool, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .w_full()
        .when(!wide, |frame| frame.max_w(px(1280.)).mx_auto())
        .px(px(if wide { 20. } else { 16. }))
        .py(px(20.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(ui::icon(icon, 20., Tokens::muted_foreground()))
                .child(
                    // App.css overrides `text-lg` to 16px/24px.
                    ui::text(title.to_owned(), 16., 24.)
                        .font_family(crate::theme::HEADING)
                        .font_weight(FontWeight::SEMIBOLD),
                ),
        )
        .child(body)
}

/// `SurfaceFrame`'s loading slot, which precedes its error state: the
/// indicator, an empty slot during the show delay, or `None` once the content
/// may render.
pub(super) fn pending_slot(phase: super::loading::Phase) -> Option<AnyElement> {
    match phase {
        super::loading::Phase::Shown => Some(loading()),
        super::loading::Phase::Waiting => Some(div().into_any_element()),
        super::loading::Phase::Ready => None,
    }
}

/// `SurfaceFrame`'s pending and error states.
pub(super) fn loading() -> AnyElement {
    div()
        .id("surface-loading")
        .test_support()
        .aria_label("Loading...")
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .min_h(px(192.))
        .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
        .child(ui::muted(ui::sm("Loading...")))
        .into_any_element()
}

pub(super) fn failed(title: &str, message: String) -> AnyElement {
    ui::panel()
        .id("surface-error")
        .test_support()
        .aria_label(title.to_owned())
        .aria_description(message.clone())
        .role(gpui_kit::Role::Alert)
        .p(px(12.))
        .border_color(Tokens::destructive().opacity(0.4))
        .child(ui::semibold(ui::sm(title.to_owned())))
        .child(ui::muted(ui::xs(message)).mt(px(4.)))
        .into_any_element()
}

/// `Button size="sm"` (`h-6 px-2 text-xs/relaxed`, `size-3` icon, `pl-1.5`
/// with a leading icon) in the outline variant.
pub(super) fn small_button(icon: Option<&str>, label: impl Into<SharedString>) -> Div {
    ui::outline_button()
        .h(px(24.))
        .pl(px(if icon.is_some() { 6. } else { 8. }))
        .pr(px(8.))
        .when_some(icon, |button, icon| {
            button.child(ui::icon(icon, 12., Tokens::foreground()))
        })
        .child(ui::medium(ui::text(label, 12., 19.5)))
}

/// A disabled control (`disabled:opacity-50`) for actions the native client
/// does not run.
pub(super) fn disabled<E: Styled>(element: E) -> E {
    element.opacity(0.5).cursor_default()
}

/// `Badge variant="secondary"`.
pub(super) fn secondary_badge(content: impl Into<SharedString>) -> Div {
    ui::status_badge(Tone::Neutral, content)
}

/// `Badge variant="outline"` with tone classes. `ui::outline_badge` fixes its
/// text to the foreground, so the tone goes on the text itself.
fn tinted_outline(color: Hsla, border: Hsla, content: impl Into<SharedString>) -> Div {
    super::delivery_ui::badge((border, Tokens::input().opacity(0.3), color), content)
}

/// `healthStateLabel`.
pub(super) fn health_state_label(state: HealthAssessmentState) -> &'static str {
    match state {
        HealthAssessmentState::Healthy => "Healthy",
        HealthAssessmentState::NeedsAttention => "Needs attention",
        HealthAssessmentState::Degraded => "Degraded",
        HealthAssessmentState::Unknown => "Unknown",
        HealthAssessmentState::NotEvaluated => "Not evaluated",
    }
}

/// The badge alone, with the web's tone classes.
pub(super) fn health_state_badge(state: HealthAssessmentState) -> Div {
    let label = health_state_label(state);
    match state {
        HealthAssessmentState::Degraded => ui::status_badge(Tone::Error, label),
        HealthAssessmentState::Healthy => {
            tinted_outline(Palette::emerald300(), Palette::emerald500(0.5), label)
        }
        HealthAssessmentState::NeedsAttention => {
            tinted_outline(Palette::amber300(), Palette::amber500(0.5), label)
        }
        _ => ui::outline_badge(label),
    }
}

/// `HealthAssessmentBadge` without `compact`: badge, source and partial marker.
pub(super) fn health_badge(assessment: &HealthAssessment) -> Div {
    div()
        .flex()
        .flex_wrap()
        .min_w_0()
        .items_center()
        .gap(px(6.))
        .child(health_state_badge(assessment.state))
        .child(
            ui::muted(ui::text(
                format!("Source: {}", health_source(&assessment.winning_sources)),
                11.,
                16.5,
            ))
            .truncate(),
        )
        .when(
            assessment.completeness == HealthAssessmentCompleteness::Partial,
            |row| row.child(ui::outline_badge("Partial").border_dashed()),
        )
}

/// Severity-colored `border-l-4` marker shared by queue cards and evidence.
pub(super) fn left_rule(color: Hsla) -> Div {
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(px(4.))
        .bg(color)
}
