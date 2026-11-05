//! The incident signal queue and empty states, mirroring `IncidentQueue.svelte`.

use super::{Inspector, incidents::severity_badge, surface};
use crate::incidents;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, ClipboardItem, Context, Hsla, SharedString, div, prelude::*, px};
use kubecove_lib::models::{
    IncidentCockpitItem as Item, IncidentSeverity as Severity, IncidentSignalState as SignalState,
};

/// `severityClass` in `IncidentQueue.svelte`.
fn severity_color(item: &Item) -> Hsla {
    match (item.state, item.severity) {
        (SignalState::Historical, _) => Tokens::muted_foreground(),
        (SignalState::Resolved, _) => Palette::sky500(1.),
        (_, Severity::Degraded) => Tokens::destructive(),
        (_, Severity::Attention | Severity::Restarted) => Palette::amber500(1.),
        _ => Tokens::muted_foreground(),
    }
}

impl Inspector {
    fn queue_card(&self, item: &Item, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let key = incidents::item_key(item);
        let resource = &item.resource;
        let border = if selected {
            Tokens::primary()
        } else {
            Tokens::border()
        };
        let rule = if selected {
            Tokens::primary()
        } else {
            severity_color(item)
        };
        let copied = self
            .incidents
            .copied
            .as_ref()
            .filter(|(copied, _)| *copied == key)
            .map(|(_, ok)| *ok);
        let copy_label = match copied {
            Some(true) => "Copied",
            Some(false) => "Copy failed",
            None => "Copy name",
        };
        let name = resource.name.clone();
        let copy_key = key.clone();
        let badges = [
            resource.status.clone(),
            resource
                .ready
                .as_ref()
                .map(|ready| format!("Ready {ready}")),
            resource
                .restarts
                .filter(|restarts| *restarts > 0)
                .map(|restarts| format!("{restarts} restarts")),
            (item.warning_event_count > 0)
                .then(|| format!("{} warnings", item.warning_event_count)),
        ];
        div()
            .relative()
            .overflow_hidden()
            .rounded(px(6.))
            .border_t_1()
            .border_r_1()
            .border_b_1()
            .border_color(border)
            .bg(if selected {
                Tokens::primary().opacity(0.1)
            } else {
                Tokens::background().opacity(0.35)
            })
            .pl(px(4.))
            .child(surface::left_rule(rule).rounded_l(px(6.)))
            .child(
                div()
                    .id(SharedString::from(format!("incident:{key}")))
                    .test_support()
                    .aria_label(format!("{}/{}", resource.kind, resource.name))
                    .w_full()
                    .p(px(12.))
                    .cursor_pointer()
                    .hover(|card| card.bg(Tokens::primary().opacity(0.05)))
                    .on_click(cx.listener({
                        let key = key.clone();
                        move |this, _, _, cx| this.select_incident(key.clone(), cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .items_start()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .min_w_0()
                                    .child(ui::semibold(ui::sm(incidents::resource_label(
                                        resource,
                                    ))))
                                    .child(
                                        ui::muted(ui::xs(incidents::scope_label(item))).mt(px(4.)),
                                    ),
                            )
                            .child(severity_badge(item)),
                    )
                    .child(surface::health_badge(&resource.health_assessment).mt(px(8.)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .mt(px(8.))
                            .children(badges.into_iter().flatten().map(ui::outline_badge)),
                    )
                    .child(
                        ui::muted(ui::text(incidents::signal_summary(item), 12., 19.5))
                            .mt(px(8.))
                            .line_clamp(2),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .px(px(8.))
                    .py(px(6.))
                    .border_t_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .id(SharedString::from(format!("incident-copy:{key}")))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .h(px(24.))
                            .pl(px(6.))
                            .pr(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|button| button.bg(Tokens::muted().opacity(0.5)))
                            .child(ui::icon(
                                if copied == Some(true) {
                                    "check"
                                } else {
                                    "copy"
                                },
                                12.,
                                Tokens::foreground(),
                            ))
                            .child(ui::medium(ui::text(copy_label, 12., 19.5)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
                                this.incidents.copied = Some((copy_key.clone(), true));
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn queue(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.incidents;
        let surface = incidents::surface_state(state.items(), state.filter, state.selected_key());
        let selected = surface.selected.map(incidents::item_key);
        let groups: Vec<AnyElement> = surface
            .groups
            .iter()
            .map(|group| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(ui::caps(&group.label, 12.).line_height(px(16.)).truncate())
                    .children(group.items.iter().map(|item| {
                        let is_selected =
                            selected.as_deref() == Some(incidents::item_key(item).as_str());
                        self.queue_card(item, is_selected, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .h_full()
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .p(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::semibold(ui::sm("Signal queue")))
                            .child(
                                ui::muted(ui::xs(
                                    "Grouped by ownership, sorted by severity and recency.",
                                ))
                                .mt(px(4.)),
                            ),
                    )
                    .child(surface::secondary_badge(surface.visible.to_string())),
            )
            .child(
                div()
                    .id("incident-queue")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(12.))
                    .child(div().flex().flex_col().gap(px(12.)).children(groups)),
            )
    }

    pub(super) fn empty_state(
        &self,
        title: &str,
        description: &str,
        cx: &mut Context<Self>,
        clean: bool,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .min_h(px(160.))
            .p(px(24.))
            .rounded(px(8.))
            .border_1()
            .border_dashed()
            .border_color(Tokens::border())
            .bg(Tokens::card().opacity(0.5))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(ui::heading(title.to_owned(), 14., 20.))
                    .child(ui::muted(ui::text(description.to_owned(), 12., 19.5))),
            )
            .when(clean, |empty| {
                empty.child(
                    ui::outline_button()
                        .id("incidents-empty-open-resources")
                        .h(px(28.))
                        .pl(px(6.))
                        .pr(px(8.))
                        .cursor_pointer()
                        .child(ui::icon("external-link", 14., Tokens::foreground()))
                        .child(ui::medium(ui::text("Open resources", 12., 19.5)))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_namespace(None, window, cx)
                        })),
                )
            })
            .into_any_element()
    }
}
