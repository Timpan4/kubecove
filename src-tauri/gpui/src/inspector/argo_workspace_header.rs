//! The workspace header in `ArgoApplicationWorkspaceHeader.svelte`.
use super::{Inspector, Page, Snapshot};
use crate::inspector::{
    controls::{self, Variant},
    delivery_ui as dui,
    detail::DetailTab,
    surface,
};
use crate::{
    gitops_surface::Selection,
    model::exact_timestamp,
    theme::{Palette, Tokens},
    ui,
};
use gpui_kit::{Context, Div, TestSupportExt, div, prelude::*, px};

impl Inspector {
    pub(in crate::inspector) fn application_workspace_header(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let state = self
            .gitops_page
            .workspace
            .as_ref()
            .filter(|_| self.page == Page::Resources)?;
        let app = &state.app;
        let inspector = match &state.snapshot {
            Snapshot::Ready(snapshot) => Some(snapshot.as_ref()),
            _ => None,
        };
        let status = |path: &[&str]| -> Option<&str> {
            path.iter()
                .try_fold(inspector?.status.as_ref()?, |value, key| value.get(*key))?
                .as_str()
        };
        let health = status(&["health", "status"])
            .or(app.health_status.as_deref())
            .unwrap_or("Unknown");
        let sync = status(&["sync", "status"])
            .or(app.sync_status.as_deref())
            .unwrap_or("Unknown");
        let source = app.sources.first();
        let repo = source
            .and_then(|source| source.repo_url.as_deref())
            .or(app.source_repo.as_deref())
            .unwrap_or("Repository unavailable");
        let path = source
            .and_then(|source| source.path.as_deref().or(source.chart.as_deref()))
            .or(app.source_mode.as_deref())
            .unwrap_or("Source path unavailable");
        let configured = source
            .and_then(|source| source.target_revision.as_deref())
            .or(app.source_revision.as_deref())
            .unwrap_or("Revision unavailable");
        let resolved = status(&["sync", "revision"])
            .or_else(|| source.and_then(|source| source.resolved_revision.as_deref()))
            .unwrap_or(configured);
        let destination = app
            .destination_namespace
            .as_deref()
            .or(app.destination_server.as_deref())
            .unwrap_or("Target unavailable");
        let destination_detail = if app.destination_namespace.is_some() {
            app.destination_server.as_deref()
        } else {
            None
        }
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{} project", app.project.as_deref().unwrap_or("default")));
        let message = match &state.snapshot {
            Snapshot::Loading => "Loading Application and managed resources…".to_owned(),
            Snapshot::Failed(error) => format!("Argo data unavailable: {error}"),
            Snapshot::Ready(_) => status(&["reconciledAt"])
                .and_then(|value| {
                    exact_timestamp(value, self.persisted.preferences.timestamp_timezone)
                })
                .map(|time| format!("Reconciled {time}"))
                .unwrap_or_else(|| "Application CRD managed-resource state".into()),
        };
        let resource = Selection::App(app).resource();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(
                surface::small_button(Some("refresh-cw"), "Refresh")
                    .id("argo-workspace-refresh")
                    .test_support()
                    .on_click(cx.listener(|this, _, _, cx| this.load_application_workspace(cx))),
            )
            .child(surface::disabled(
                controls::button(Variant::Default, 24., false)
                    .child(ui::icon("rotate-ccw", 12., Tokens::foreground()))
                    .child("Sync"),
            ))
            .child(
                dui::sm_button(Variant::Ghost, "Dashboard")
                    .child(ui::icon("panel-right-open", 12., Tokens::foreground()))
                    .id("argo-workspace-dashboard")
                    .test_support()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_resource_on(resource.clone(), Some(DetailTab::Argo), window, cx)
                    })),
            );
        let title = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .child(copyable("argo-workspace-name", &app.name, false, cx))
            .child(surface::health_badge(&app.health_assessment));
        let raw = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .mt(px(4.))
            .child(ui::muted(ui::text("Raw Argo:", 11., 16.5)))
            .child(dui::tinted_badge(raw_tone(health), health.to_owned()))
            .child(dui::tinted_badge(raw_tone(sync), sync.to_owned()));
        let reconciled = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .mt(px(2.))
            .child(ui::icon("wifi", 12., Palette::emerald400()))
            .child(ui::muted(ui::xs(message)));
        let identity = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(32.))
                    .rounded(px(8.))
                    .child(ui::icon("git-branch", 16., Tokens::primary())),
            )
            .child(div().min_w_0().child(title).child(raw).child(reconciled));
        let header = div().px(px(12.)).py(px(10.)).border_b_1().border_color(Tokens::border())
            .flex().flex_wrap().items_start().gap(px(12.))
            .child(identity).child(actions)
            .child(ui::muted(ui::text("Sync unavailable here until operation progress tracking is wired to this overview.", 11., 16.5))
                .w_full().text_align(gpui_kit::TextAlign::Right));
        let cards = div().id("argo-workspace-route").overflow_x_scroll().child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(10.))
                .min_w(px(1120.))
                .child(
                    route("1", "Repository", repo, path, cx)
                        .min_w(px(256.))
                        .flex_grow(1.),
                )
                .child(ui::icon("arrow-right", 16., Tokens::muted_foreground()))
                .child(
                    route("2", "Release", resolved, &format!("from {configured}"), cx)
                        .min_w(px(144.))
                        .flex_grow(0.55),
                )
                .child(ui::icon("arrow-right", 16., Tokens::muted_foreground()))
                .child(
                    route("3", "Cluster target", destination, &destination_detail, cx)
                        .min_w(px(176.))
                        .flex_grow(0.65),
                )
                .child(ui::icon("arrow-right", 16., Tokens::muted_foreground()))
                .child(result_card(state.counts).min_w(px(384.)).flex_grow(1.2)),
        );
        Some(
            div()
                .flex_none()
                .rounded(px(10.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::card())
                .overflow_hidden()
                .child(header)
                .child(cards),
        )
    }
}

/// `RouteCard`: `rounded-md border bg-surface-1/50 p-2.5`.
fn route(step: &str, label: &str, value: &str, detail: &str, cx: &mut Context<Inspector>) -> Div {
    div()
        .min_w_0()
        .flex_basis(px(0.))
        .p(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card().opacity(0.5))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(step_circle(step, false))
                .child(ui::caps(label, 10.)),
        )
        .child(copyable(&format!("argo-route-{step}-value"), value, false, cx).mt(px(4.)))
        .child(copyable(
            &format!("argo-route-{step}-detail"),
            detail,
            true,
            cx,
        ))
}

fn raw_tone(status: &str) -> ui::Tone {
    match status {
        "Healthy" | "Synced" => ui::Tone::Success,
        "Degraded" | "Missing" => ui::Tone::Error,
        "OutOfSync" | "Progressing" | "Unknown" => ui::Tone::Warning,
        _ => ui::Tone::Neutral,
    }
}

fn copyable(id: &str, value: &str, muted: bool, cx: &mut Context<Inspector>) -> Div {
    let text = if muted {
        ui::muted(ui::text(value.to_owned(), 11., 16.5))
    } else {
        ui::semibold(ui::sm(value.to_owned()))
    };
    div()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .child(
            text.id(gpui_kit::SharedString::from(id.to_owned()))
                .test_support()
                .aria_label(value.to_owned())
                .truncate(),
        )
        .child(crate::inspector::gitops_tooltip::reveal(
            div()
                .flex_none()
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .hover(|button| button.bg(Tokens::muted()))
                .child(ui::icon("copy", 12., Tokens::muted_foreground())),
            format!("{id}-copy"),
            value.to_owned(),
            cx,
        ))
}

fn step_circle(step: &str, warning: bool) -> Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(if warning {
            Palette::amber500(0.15)
        } else {
            Tokens::primary().opacity(0.05)
        })
        .text_color(if warning {
            Palette::amber300()
        } else {
            Tokens::primary()
        })
        .child(ui::semibold(ui::text(step.to_owned(), 10., 15.)))
}

fn result_card(counts: [usize; 6]) -> Div {
    let [total, needs_sync, _, degraded, progressing, prune] = counts;
    let current = total - needs_sync;
    let mut signals = div().flex().min_w_0().flex_wrap().gap(px(4.));
    for (count, label, tone) in [
        (degraded, "degraded", ui::Tone::Error),
        (progressing, "progressing", ui::Tone::Warning),
        (prune, "to prune", ui::Tone::Error),
    ] {
        if count > 0 {
            signals = signals.child(signal_badge(tone, format!("{count} {label}")));
        }
    }
    if degraded + progressing + prune == 0 {
        signals = signals.child(signal_badge(
            ui::Tone::Success,
            "No additional signals".into(),
        ));
    }
    let bar = div()
        .flex()
        .h(px(6.))
        .overflow_hidden()
        .rounded_full()
        .mt(px(6.))
        .bg(Tokens::border().opacity(0.5))
        .when(needs_sync > 0, |bar| {
            bar.child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(needs_sync as f32)
                    .bg(Palette::amber500(0.7)),
            )
        })
        .when(current > 0, |bar| {
            bar.child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(current as f32)
                    .bg(Palette::emerald400().opacity(0.7)),
            )
        });
    div()
        .flex_basis(px(0.))
        .p(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card().opacity(0.5))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(step_circle("4", true))
                .child(ui::caps("Resource result", 10.))
                .child(
                    ui::xs(format!(
                        "{needs_sync} need sync · {current} current · {total} total"
                    ))
                    .ml_auto()
                    .whitespace_nowrap(),
                ),
        )
        .child(bar)
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .mt(px(6.))
                .child(
                    ui::muted(ui::text(
                        "Health and prune may overlap sync state",
                        10.,
                        15.,
                    ))
                    .flex_none(),
                )
                .child(signals),
        )
}

/// The workspace header's `statusBadgeClass` uses 11px text.
fn signal_badge(tone: ui::Tone, label: String) -> Div {
    let (background, foreground, border) = match tone {
        ui::Tone::Error => (
            Tokens::destructive().opacity(0.2),
            Tokens::destructive(),
            Tokens::destructive().opacity(0.1),
        ),
        ui::Tone::Warning => (
            Palette::amber500(0.15),
            Palette::amber300(),
            Palette::amber500(0.3),
        ),
        _ => (
            Palette::emerald500(0.15),
            Palette::emerald300(),
            Palette::emerald500(0.3),
        ),
    };
    div()
        .flex_none()
        .h(px(20.))
        .px(px(8.))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
        .text_color(foreground)
        .child(ui::medium(ui::text(label, 11., 16.5)))
}
