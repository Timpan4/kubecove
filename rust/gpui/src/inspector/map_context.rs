//! The ownership map's "Map context" select. Ownership edges never cross
//! clusters, so the map shows one workspace context at a time (ADR 0033). The
//! select lists the workspace contexts in workspace order, defaults to the
//! primary context and shows only when the workspace has more than one.
//!
//! Its keys follow the web's Bits UI Select. Keyboard focus stays on the
//! trigger. Enter, Space and the vertical arrows open the options with the
//! current context highlighted. While they are open, the arrows move the
//! highlight without wrapping, Home and End jump to the first and last option,
//! Enter or Space chooses the highlighted context, and Escape closes them
//! without a change. Tab and Shift+Tab close them and move focus on.

use super::{Inspector, overlay_motion::Overlay};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{
    AnyElement, App, Bounds, ClickEvent, Context, Global, KeyBinding, KeyDownEvent, MouseDownEvent,
    Pixels, SharedString, Window, actions, deferred, div, prelude::*, px,
};
use std::{cell::Cell, rc::Rc};

/// The trigger's key context. Its bindings outrank the window root's.
const CONTEXT: &str = "MapContextSelect";

actions!(map_context, [LeaveMapContext]);

struct Bound;
impl Global for Bound {}

/// Binds Tab and Shift+Tab on the trigger, once per app. The window root binds
/// them to focus traversal, and key bindings run before key listeners, so the
/// trigger's own key handler never sees them.
pub(super) fn bind(cx: &mut App) {
    if cx.has_global::<Bound>() {
        return;
    }
    cx.bind_keys([
        KeyBinding::new("tab", LeaveMapContext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", LeaveMapContext, Some(CONTEXT)),
    ]);
    cx.set_global(Bound);
}

impl Inspector {
    /// Closes the options and lets the key reach the root's focus traversal.
    fn leave_map_context(&mut self, _: &LeaveMapContext, _: &mut Window, cx: &mut Context<Self>) {
        self.close_map_context(cx);
        cx.propagate();
    }

    fn close_map_context(&mut self, cx: &mut Context<Self>) {
        if self.map.context_open {
            self.map.context_open = false;
            cx.notify();
        }
    }

    /// The context the map reads: the chosen one while it is still a workspace
    /// context, else the primary context.
    pub(super) fn map_context(&self) -> &str {
        self.map
            .context
            .as_deref()
            .filter(|chosen| {
                !chosen.is_empty()
                    && self.scope_workspace().is_some_and(|workspace| {
                        workspace.context == *chosen
                            || workspace
                                .group
                                .iter()
                                .any(|member| member.as_str() == *chosen)
                    })
            })
            .unwrap_or(&self.scope.context)
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
                    None => self.close_map_context(cx),
                }
                return true;
            }
            "escape" => {
                self.close_map_context(cx);
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
        let current = self.map_context().to_owned();
        let open = self.map.context_open;
        let highlight = self.map.context_highlight;
        let trigger_bounds = open.then(|| Rc::new(Cell::new(None::<Bounds<Pixels>>)));
        let shown = self.overlay_motion.sample(Overlay::MapContext, open);
        let popup = shown.map(|shown| {
            deferred(
                shown.surface(
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
                        .when_some(trigger_bounds.clone(), |panel, trigger_bounds| {
                            panel.on_mouse_down_out(cx.listener(
                                move |this, event: &MouseDownEvent, _, cx| {
                                    // The trigger handles its own click after this press.
                                    if !trigger_bounds
                                        .get()
                                        .is_some_and(|bounds| bounds.contains(&event.position))
                                    {
                                        this.close_map_context(cx);
                                    }
                                },
                            ))
                        })
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
                ),
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
                        .key_context(CONTEXT)
                        .on_action(cx.listener(Self::leave_map_context))
                        .focus_visible(|style| style.border_color(Tokens::ring()))
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
                                this.close_map_context(cx);
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
                .when_some(trigger_bounds, |picker, trigger_bounds| {
                    picker.on_children_prepainted(move |bounds, _, _| {
                        trigger_bounds.set(bounds.first().copied());
                    })
                })
                .children(popup)
                .into_any_element(),
        )
    }
}
