//! The ownership map's "Map context" select. Ownership edges never cross
//! clusters, so the map shows one workspace context at a time (ADR 0033). The
//! select lists the workspace contexts in workspace order, defaults to the
//! primary context and shows only when the workspace has more than one.
//!
//! Its keys follow the web's Bits UI Select. Keyboard focus stays on the
//! trigger. Enter, Space and the vertical arrows open the options with the
//! current context highlighted. While they are open, the arrows move the
//! highlight without wrapping, Home and End jump to the first and last option,
//! Enter or Space chooses the highlighted context and Escape closes them
//! without a change.

use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{
    AnyElement, ClickEvent, Context, KeyDownEvent, SharedString, deferred, div, prelude::*, px,
};

impl Inspector {
    /// The context the map reads: the chosen one while it is still a workspace
    /// context, else the primary context.
    pub(super) fn map_context(&self) -> String {
        self.map
            .context
            .as_ref()
            .filter(|chosen| self.workspace_contexts().contains(chosen))
            .cloned()
            .unwrap_or_else(|| self.scope.context.clone())
    }

    fn choose_map_context(&mut self, context: String, cx: &mut Context<Self>) {
        self.map.context_open = false;
        if context == self.map_context() {
            cx.notify();
            return;
        }
        self.map.context = Some(context);
        self.load_topology(cx);
    }

    fn open_map_context(&mut self, cx: &mut Context<Self>) {
        let current = self.map_context();
        self.map.context_highlight = self
            .workspace_contexts()
            .iter()
            .position(|context| *context == current);
        self.map.context_open = true;
        cx.notify();
    }

    /// Applies a key pressed on the trigger. Returns whether the select used it.
    fn map_context_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if !self.map.context_open {
            if !matches!(key, "enter" | "space" | "down" | "up") {
                return false;
            }
            self.open_map_context(cx);
            return true;
        }
        let contexts = self.workspace_contexts();
        let last = contexts.len().saturating_sub(1);
        let highlight = self.map.context_highlight;
        self.map.context_highlight = match key {
            "down" => Some(highlight.map_or(0, |index| (index + 1).min(last))),
            "up" => Some(highlight.map_or(last, |index| index.saturating_sub(1))),
            "home" => Some(0),
            "end" => Some(last),
            "enter" | "space" => {
                match highlight.and_then(|index| contexts.get(index)) {
                    Some(context) => self.choose_map_context(context.clone(), cx),
                    None => {
                        self.map.context_open = false;
                        cx.notify();
                    }
                }
                return true;
            }
            "escape" => {
                self.map.context_open = false;
                cx.notify();
                return true;
            }
            _ => return false,
        };
        cx.notify();
        true
    }

    pub(super) fn map_context_select(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let contexts = self.workspace_contexts();
        if contexts.len() < 2 {
            return None;
        }
        let current = self.map_context();
        let open = self.map.context_open;
        let highlight = self.map.context_highlight;
        let popup = open.then(|| {
            deferred(
                div()
                    .id("map-context-options")
                    .test_support()
                    .role(gpui_kit::Role::ListBox)
                    .aria_label("Map context")
                    .occlude()
                    .absolute()
                    .top(px(34.))
                    .right_0()
                    .min_w(px(200.))
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::surface2())
                    .shadow_lg()
                    .children(contexts.into_iter().enumerate().map(|(index, context)| {
                        let checked = context == current;
                        div()
                            .id(SharedString::from(format!("map-context-option:{context}")))
                            .test_support()
                            .role(gpui_kit::Role::ListBoxOption)
                            .aria_label(context.clone())
                            .aria_selected(checked)
                            .h(px(32.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|row| row.bg(Tokens::muted()))
                            .when(highlight == Some(index), |row| row.bg(Tokens::muted()))
                            .child(ui::xs(context.clone()))
                            .when(checked, |row| {
                                row.child(ui::icon("check", 14., Tokens::foreground()))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_map_context(context.clone(), cx);
                            }))
                    })),
            )
            .with_priority(1)
        });
        Some(
            div()
                .relative()
                .child(
                    ui::outline_button()
                        .id("map-context")
                        .test_support()
                        .role(gpui_kit::Role::ComboBox)
                        .aria_label("Map context")
                        .aria_expanded(open)
                        .track_focus(&self.map.context_trigger)
                        .focus(|style| style.border_color(Tokens::ring()))
                        .h(px(32.))
                        .px(px(10.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .child(ui::muted(ui::xs("Map context")))
                        .child(
                            ui::semibold(ui::xs(current.clone()))
                                .id("map-context-value")
                                .test_support()
                                .aria_label(current),
                        )
                        .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            // The key handler owns Enter and Space, which GPUI also
                            // reports as a click on the focused trigger.
                            if event.is_keyboard() {
                                return;
                            }
                            if this.map.context_open {
                                this.map.context_open = false;
                                cx.notify();
                            } else {
                                this.open_map_context(cx);
                            }
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            let keystroke = &event.keystroke;
                            if !keystroke.modifiers.modified()
                                && this.map_context_key(&keystroke.key, cx)
                            {
                                cx.stop_propagation();
                            }
                        })),
                )
                .children(popup)
                .into_any_element(),
        )
    }
}
