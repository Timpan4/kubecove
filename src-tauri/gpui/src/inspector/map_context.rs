//! The ownership map's "Map context" select. Ownership edges never cross
//! clusters, so the map shows one workspace context at a time (ADR 0033). The
//! select lists the workspace contexts in workspace order, defaults to the
//! primary context and shows only when the workspace has more than one.

use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, SharedString, deferred, div, prelude::*, px};

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

    pub(super) fn map_context_select(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let contexts = self.workspace_contexts();
        if contexts.len() < 2 {
            return None;
        }
        let current = self.map_context();
        let popup = self.map.context_open.then(|| {
            deferred(
                div()
                    .id("map-context-options")
                    .test_support()
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
                    .children(contexts.into_iter().map(|context| {
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
                        .aria_label("Map context")
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
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.map.context_open = !this.map.context_open;
                            cx.notify();
                        })),
                )
                .children(popup)
                .into_any_element(),
        )
    }
}
