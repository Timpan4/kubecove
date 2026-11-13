use super::resource_filters::HealthFilter;
use super::{Inspector, Page};
use crate::{targets::TestSupportExt, theme::Tokens, ui};
use gpui_kit::{Context, Div, FocusHandle, KeyDownEvent, SharedString, Window, prelude::*, px};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

pub(super) struct Projection {
    pub rows: Vec<kubecove_lib::models::ResourceSummary>,
    pub health: crate::model::HealthSummary,
    pub model: crate::workspaces::overview::Overview,
}

#[derive(Default)]
pub(super) struct State {
    pub expanded: HashSet<String>,
    pub quiet: bool,
    pub page: usize,
    focus: RefCell<HashMap<String, FocusHandle>>,
}
impl Inspector {
    pub(super) fn overview_button(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> crate::targets::ObservedElement<gpui_kit::Stateful<Div>> {
        let id = id.into();
        let focus = self
            .overview_ui
            .focus
            .borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| cx.focus_handle())
            .clone()
            .tab_stop(enabled);
        let action = Rc::new(action);
        let clicked = action.clone();
        ui::outline_button()
            .id(SharedString::from(id))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label.into())
            .aria_description(if enabled { "enabled" } else { "disabled" })
            .track_focus(&focus)
            .focus(|style| style.border_color(Tokens::ring()))
            .when(!enabled, |button| button.opacity(0.5))
            .when(enabled, |button| button.cursor_pointer())
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    clicked(this, window, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if enabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    action(this, window, cx);
                    cx.stop_propagation();
                }
            }))
            .px(px(8.))
            .py(px(4.))
    }
    pub(super) fn open_overview_resources(
        &mut self,
        namespace: Option<String>,
        health: HealthFilter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.select_namespace(namespace, window, cx);
        if self.page != Page::Resources {
            return;
        }
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.filters.health = health;
        self.filters.contexts.clear();
        self.filters.gitops_owner = None;
        self.filter(cx);
    }
}
