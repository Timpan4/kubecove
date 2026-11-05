//! Sync options keep the Application-name confirmation guard.
use super::{
    Inspector,
    argo_tab::ArgoTab,
    controls::{Variant, button},
};
use crate::ui;
use gpui_kit::{
    AppContext, Context, Div, Entity, TestSupportExt, Window,
    component::input::{Input, InputState},
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
    pub revision: Entity<InputState>,
    pub name: Entity<InputState>,
    pub prune: Option<bool>,
    pub dry_run: bool,
    pub force: bool,
    pub history: Option<i64>,
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
            revision: cx.new(|cx| InputState::new(window, cx)),
            name: cx.new(|cx| InputState::new(window, cx)),
            prune: None,
            dry_run: false,
            force: false,
            history: None,
        }
    }
}
pub(super) fn panel(state: &ArgoTab, name: &str, cx: &mut Context<Inspector>) -> Div {
    let mut panel = super::argo_panels::panel().flex().flex_col().gap(px(8.));
    if state.options.open {
        panel = panel
            .child(ui::xs(
                "Sync with options. Choices beyond Application defaults require confirmation.",
            ))
            .child(
                Input::new(&state.options.revision)
                    .id("argo-sync-revision")
                    .aria_label("Revision override"),
            )
            .children(["Prune", "Dry run", "Force / replace"].map(|label| {
                button(Variant::Outline, 24., false)
                    .id(gpui_kit::SharedString::from(format!("argo-sync:{label}")))
                    .test_support()
                    .child(format!(
                        "{label}: {}",
                        match label {
                            "Prune" => state.options.prune.unwrap_or(false),
                            "Dry run" => state.options.dry_run,
                            _ => state.options.force,
                        }
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(s) = this.argo_tab_state() {
                            match label {
                                "Prune" => {
                                    s.options.prune = Some(!s.options.prune.unwrap_or(false))
                                }
                                "Dry run" => s.options.dry_run = !s.options.dry_run,
                                _ => s.options.force = !s.options.force,
                            };
                            cx.notify();
                        }
                    }))
            }))
            .child(ui::xs(format!("Type {name} to confirm advanced sync")))
            .child(
                Input::new(&state.options.name)
                    .id("argo-sync-name")
                    .aria_label("Application name confirmation"),
            )
            .child(
                button(Variant::Default, 24., false)
                    .id("argo-sync-advanced")
                    .test_support()
                    .child("Review advanced sync")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.review_argo_operation("Advanced Sync", window, cx)
                    })),
            );
    }
    panel
}

pub(super) fn trigger(cx: &mut Context<Inspector>) -> gpui_kit::AnyElement {
    button(Variant::Default, 24., false)
        .id("argo-sync-options")
        .test_support()
        .aria_label("Sync options")
        .child(crate::ui::icon(
            "chevron-down",
            12.,
            crate::theme::Tokens::primary_foreground(),
        ))
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(s) = this.argo_tab_state() {
                s.options.open = !s.options.open;
                cx.notify();
            }
        }))
        .into_any_element()
}
