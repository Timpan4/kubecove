//! Sync options follow the Svelte flow. Choices beyond the Application defaults
//! open the Application-name confirmation before the reviewed operation starts.
use super::super::{
    Inspector,
    controls::{Variant, button},
};
use super::tab::ArgoTab;
use crate::targets::TestSupportExt;
use crate::ui;
use gpui_kit::{
    AppContext, Context, Div, Entity, FocusHandle, Window,
    base::FocusTrapElement,
    component::input::{Input, InputState},
    div,
    prelude::*,
    px,
};

#[derive(Clone)]
pub(super) struct HistoryChoice {
    pub index: usize,
    pub revision: gpui_kit::SharedString,
    pub id: Option<i64>,
}
impl gpui_kit::component::searchable_list::SearchableListItem for HistoryChoice {
    type Value = usize;
    fn value(&self) -> &usize {
        &self.index
    }
    fn title(&self) -> gpui_kit::SharedString {
        self.revision.clone()
    }
    fn render(&self, _: &mut Window, _: &mut gpui_kit::App) -> impl IntoElement {
        gpui_kit::div()
            .child(self.revision.clone())
            .when_some(self.id, |row, id| {
                row.child(ui::muted(ui::xs(format!("Deployment {id}"))))
            })
    }
}
pub(super) struct Options {
    pub open: bool,
    /// Second step of Sync with options: the name confirmation is shown.
    pub confirming: bool,
    pub revision: Entity<InputState>,
    pub name: Entity<InputState>,
    pub prune: Option<bool>,
    pub default_prune: bool,
    pub dry_run: bool,
    pub force: bool,
    pub history: Option<i64>,
    /// Focused when the Sync options popover opens. Its focus trap keeps Tab inside.
    pub sync_focus: FocusHandle,
    /// Focused when the Refresh options popover opens. Its focus trap keeps Tab inside.
    pub refresh_focus: FocusHandle,
    /// Keyboard focus targets for the Sync options checkboxes.
    pub prune_focus: FocusHandle,
    pub dry_run_focus: FocusHandle,
    pub force_focus: FocusHandle,
    /// Receives focus again when the Sync options popover closes with Escape.
    pub sync_trigger: FocusHandle,
    /// Receives focus again when the Refresh options popover closes with Escape.
    pub refresh_trigger: FocusHandle,
    pub picker: Entity<
        gpui_kit::component::select::SelectState<
            gpui_kit::component::select::SearchableVec<HistoryChoice>,
        >,
    >,
    pub _subscription: gpui_kit::Subscription,
}
impl Options {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
        let picker = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<HistoryChoice>::new()),
                None,
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(&picker, window, |this, _, event, _, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            if let Some(state) = this.argo_tab_state() {
                state.options.history = state
                    .inspector
                    .as_ref()
                    .and_then(|s| s.history.get(*value).and_then(|h| h.id));
                cx.notify();
            }
        });
        Self {
            picker,
            _subscription: subscription,
            open: false,
            confirming: false,
            revision: cx.new(|cx| InputState::new(window, cx)),
            name: cx.new(|cx| InputState::new(window, cx)),
            prune: None,
            default_prune: false,
            dry_run: false,
            force: false,
            history: None,
            sync_focus: cx.focus_handle(),
            refresh_focus: cx.focus_handle(),
            prune_focus: cx.focus_handle(),
            dry_run_focus: cx.focus_handle(),
            force_focus: cx.focus_handle(),
            sync_trigger: cx.focus_handle(),
            refresh_trigger: cx.focus_handle(),
        }
    }
}

fn needs_confirmation(state: &ArgoTab, cx: &gpui_kit::App) -> bool {
    let options = &state.options;
    options.prune.unwrap_or(options.default_prune) != options.default_prune
        || options.dry_run
        || options.force
        || !options.revision.read(cx).value().as_str().trim().is_empty()
}

fn choice(
    label: &'static str,
    checked: bool,
    focus: FocusHandle,
    cx: &mut Context<Inspector>,
) -> gpui_kit::AnyElement {
    button(Variant::Outline, 24., false)
        .id(gpui_kit::SharedString::from(format!("argo-sync:{label}")))
        .test_support()
        .track_focus(&focus)
        .role(gpui_kit::Role::CheckBox)
        .aria_label(label)
        .aria_description(if checked { "checked" } else { "unchecked" })
        .flex_1()
        .child(
            div()
                .size(px(14.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(crate::theme::Tokens::border())
                .when(checked, |boxed| {
                    boxed.bg(crate::theme::Tokens::primary()).child(ui::icon(
                        "check",
                        10.,
                        crate::theme::Tokens::primary_foreground(),
                    ))
                }),
        )
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(s) = this.argo_tab_state() {
                match label {
                    "Prune" => s.options.prune = Some(!s.options.prune.unwrap_or(false)),
                    "Dry run" => s.options.dry_run = !s.options.dry_run,
                    _ => s.options.force = !s.options.force,
                };
                cx.notify();
            }
        }))
        .into_any_element()
}

pub(super) fn panel(state: &ArgoTab, name: &str, cx: &mut Context<Inspector>) -> Div {
    let mut panel = super::panels::panel().flex().flex_col().gap(px(8.));
    if !state.options.open {
        return panel;
    }
    if state.options.confirming {
        let revision = state.options.revision.read(cx).value();
        let revision = if revision.is_empty() {
            state.configured_revision()
        } else {
            revision.as_str()
        };
        let enabled = |value| if value { "Enabled" } else { "Disabled" };
        panel = panel
            .child(ui::sm("Confirm advanced sync"))
            .child(ui::xs(
                "These choices go beyond Application defaults. Type Application name to continue.",
            ))
            .child(ui::xs(format!(
                "Revision: {}. Prune: {}. Dry run: {}. Force: {}.",
                revision,
                enabled(state.options.prune.unwrap_or(state.options.default_prune)),
                enabled(state.options.dry_run),
                enabled(state.options.force)
            )))
            .child(ui::xs(format!("Type {name} to confirm advanced sync")))
            .child(
                Input::new(&state.options.name)
                    .id("argo-sync-name")
                    .aria_label("Application name confirmation"),
            )
            .child(
                button(
                    Variant::Default,
                    24.,
                    state.options.name.read(cx).value().as_str() != name,
                )
                .id("argo-sync-confirm")
                .test_support()
                .child("Confirm sync")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.review_argo_operation("Advanced Sync", window, cx)
                })),
            )
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-sync-back")
                    .test_support()
                    .child("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(s) = this.argo_tab_state() {
                            s.options.confirming = false;
                            s.options.open = false;
                            cx.notify();
                        }
                    })),
            );
        return panel;
    }
    panel = panel
        .child(ui::sm("Sync with options"))
        .child(ui::xs(
            "Main Sync uses Application defaults. Choices beyond those defaults require confirmation.",
        ).id("argo-sync-description").test_support().aria_label("Main Sync uses Application defaults. Choices beyond those defaults require confirmation."))
        .child(ui::xs("Revision override"))
        .child(
            Input::new(&state.options.revision)
                .id("argo-sync-revision")
                .aria_label("Revision override"),
        )
        .child(
            div()
                .flex()
                .gap(px(6.))
                .child(choice(
                    "Prune",
                    state.options.prune.unwrap_or(false),
                    state.options.prune_focus.clone(),
                    cx,
                ))
                .child(choice(
                    "Dry run",
                    state.options.dry_run,
                    state.options.dry_run_focus.clone(),
                    cx,
                )),
        )
        .child(choice(
            "Force / replace",
            state.options.force,
            state.options.force_focus.clone(),
            cx,
        ))
        .child(
            button(Variant::Default, 24., false)
                .id("argo-sync-advanced")
                .test_support()
                .w_full()
                .child("Sync with these options")
                .on_click(cx.listener(|this, _, window, cx| {
                    let Some(state) = this.argo_tab_state() else {
                        return;
                    };
                    if needs_confirmation(state, cx) {
                        state.options.name.update(cx, |input, cx| input.set_value("", window, cx));
                        state.options.confirming = true;
                        cx.notify();
                    } else {
                        this.review_argo_operation("Sync", window, cx);
                    }
                })),
        );
    panel
}

impl Inspector {
    pub(in crate::inspector) fn argo_sync_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let super::super::detail::Detail::Ready(detail) = &self.detail else {
            return None;
        };
        let state = detail.argo.as_ref()?;
        if !state.options.confirming {
            return None;
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(crate::theme::Tokens::background().opacity(0.8))
                .child(
                    div()
                        .id("argo-sync-dialog")
                        .test_support()
                        .role(gpui_kit::Role::Dialog)
                        .aria_label("Confirm advanced sync")
                        .w(px(512.))
                        .child(panel(state, &detail.data.summary.name, cx)),
                )
                .into_any_element(),
        )
    }
}

/// Toggles from the open state the frame was drawn with. The panel's outside-press handler
/// closes it first on the same press, so toggling the live state would reopen it.
fn trigger(
    disabled: bool,
    open: bool,
    focus: FocusHandle,
    trigger_focus: FocusHandle,
    cx: &mut Context<Inspector>,
) -> gpui_kit::AnyElement {
    button(Variant::Default, 24., disabled)
        .id("argo-sync-options")
        .test_support()
        .track_focus(&trigger_focus)
        .aria_label("Sync options")
        .aria_description(if disabled { "disabled" } else { "enabled" })
        .child(crate::ui::icon(
            "chevron-down",
            12.,
            crate::theme::Tokens::primary_foreground(),
        ))
        .on_mouse_down(
            gpui_kit::MouseButton::Left,
            cx.listener(move |this, _, window, cx| {
                if disabled {
                    return;
                }
                if let Some(s) = this.argo_tab_state() {
                    s.options.open = !open;
                    s.options.confirming = false;
                    if !open {
                        // The button's own press focus runs after this listener, so focus
                        // the popover once the press has been handled.
                        let focus = focus.clone();
                        cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
                    }
                    cx.notify();
                }
            }),
        )
        .into_any_element()
}

impl Inspector {
    /// Escape closes an open Sync or Refresh options popover, as the web does.
    /// Returns whether one was open, so the caller can stop the key there.
    pub(in crate::inspector) fn close_argo_option_popovers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(state) = self.argo_tab_state() else {
            return false;
        };
        let sync_open = state.options.open && !state.options.confirming;
        let refresh_open = state.refresh_options;
        let trigger = if sync_open {
            Some(state.options.sync_trigger.clone())
        } else if refresh_open {
            Some(state.options.refresh_trigger.clone())
        } else {
            None
        };
        state.options.open = false;
        state.refresh_options = false;
        let Some(trigger) = trigger else {
            return false;
        };
        trigger.focus(window, cx);
        true
    }
}

/// `unavailable` is the web's `argoOperationAvailability` failing for the selected connection.
/// It disables Sync and Refresh and their option triggers; Rollback stays available, as on the web.
pub(super) fn controls(
    state: &ArgoTab,
    name: &str,
    pending: bool,
    unavailable: bool,
    cx: &mut Context<Inspector>,
) -> Div {
    use crate::theme::Tokens;
    use gpui_kit::SharedString;
    let in_flight = state.operation.in_flight();
    let busy = pending || in_flight.is_some();
    // Only the button that started the in-flight operation shows a spinner.
    let refreshing = matches!(in_flight, Some("refresh" | "hardRefresh"));
    let syncing = in_flight == Some("sync");
    let refresh_open = state.refresh_options;
    let refresh_focus = state.options.refresh_focus.clone();
    let refresh_focus_on_open = refresh_focus.clone();
    let refresh_trigger = state.options.refresh_trigger.clone();
    let sync_focus = state.options.sync_focus.clone();
    let sync_focus_on_open = sync_focus.clone();
    let sync_trigger = state.options.sync_trigger.clone();
    let has_previous = state
        .inspector
        .as_ref()
        .is_some_and(|snapshot| snapshot.history.len() > 1);
    let operation = |label: &'static str, variant, cx: &mut Context<Inspector>| {
        let disabled =
            busy || (label == "Rollback" && !has_previous) || (unavailable && label != "Rollback");
        let spinning = match label {
            "Refresh" | "Hard refresh" => refreshing,
            "Sync" => syncing,
            _ => false,
        };
        button(variant, 24., disabled)
            .id(SharedString::from(format!("argo-operation:{label}")))
            .test_support()
            .aria_description(if disabled { "disabled" } else { "enabled" })
            .when(label != "Rollback", |b| {
                b.child(ui::icon(
                    if spinning {
                        "loader-circle"
                    } else if label == "Refresh" {
                        "refresh-cw"
                    } else {
                        "rotate-ccw"
                    },
                    12.,
                    if label == "Sync" {
                        Tokens::primary_foreground()
                    } else {
                        Tokens::foreground()
                    },
                ))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| {
                if disabled {
                    return;
                }
                this.review_argo_operation(label, window, cx)
            }))
    };
    div().flex().flex_wrap().gap(px(6.))
        .child(operation("Rollback", Variant::Outline, cx))
        .child(div().relative().flex()
            .child(operation("Refresh", Variant::Outline, cx))
            .child(button(Variant::Outline, 24., busy || unavailable).id("argo-refresh-options").test_support().track_focus(&refresh_trigger).aria_label("Refresh options")
                .aria_description(if busy || unavailable { "disabled" } else { "enabled" })
                .child(ui::icon("chevron-down", 12., Tokens::foreground()))
                .on_mouse_down(gpui_kit::MouseButton::Left, cx.listener(move |this, _, window, cx| {
                    if busy || unavailable { return; }
                    if let Some(s) = this.argo_tab_state() {
                        s.refresh_options = !refresh_open;
                        if !refresh_open {
                            let focus = refresh_focus_on_open.clone();
                            cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
                        }
                        cx.notify();
                    }
                })))
            .when(state.refresh_options, |group| group.child(gpui_kit::deferred(super::panels::panel().id("argo-refresh-panel").test_support().occlude().absolute().top(px(28.)).right_0().w(px(256.)).shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if let Some(s) = this.argo_tab_state() { s.refresh_options = false; }
                    cx.notify();
                }))
                .child(ui::sm("Refresh Application"))
                .child(ui::muted(ui::xs("Normal refresh re-reads Application state. Hard refresh also invalidates Argo CD manifest and repository caches.")))
                .child(operation("Hard refresh", Variant::Outline, cx))
                .focus_trap("argo-refresh-panel", &refresh_focus)).with_priority(1))))
        .child(div().relative().flex()
            .child(operation("Sync", Variant::Default, cx))
            .child(trigger(busy || unavailable, state.options.open, sync_focus_on_open, sync_trigger, cx))
            .when(state.options.open && !state.options.confirming, |group| group.child(gpui_kit::deferred(panel(state, name, cx).id("argo-sync-panel").test_support().occlude().absolute().top(px(28.)).right_0().w(px(320.)).shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if let Some(s) = this.argo_tab_state() { s.options.open = false; }
                    cx.notify();
                }))
                .focus_trap("argo-sync-panel", &sync_focus)).with_priority(1))))
}
