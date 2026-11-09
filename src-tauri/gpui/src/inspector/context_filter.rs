//! The Resources toolbar's Context filter (ADR 0033): a multi-select of the
//! workspace contexts in rank order, all selected by default, shown only when
//! the workspace has more than one context. It filters display only; every
//! context is still read and watched.

use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, SharedString, deferred, div, prelude::*, px};

/// The contexts that stay selected after toggling `context`, from the
/// selection `selected` (empty means every context) over `available`. A
/// selection that holds every available context is stored as empty.
fn next_context_selection(
    selected: &[String],
    available: &[String],
    context: &str,
    checked: bool,
) -> Vec<String> {
    let current = if selected.is_empty() {
        available
    } else {
        selected
    };
    let next: Vec<String> = available
        .iter()
        .filter(|item| {
            if item.as_str() == context {
                checked
            } else {
                current.contains(item)
            }
        })
        .cloned()
        .collect();
    if next.len() == available.len() {
        Vec::new()
    } else {
        next
    }
}

impl Inspector {
    /// The contexts the table shows, or `None` for every context. A stored
    /// selection keeps only contexts that are still in the workspace, and one
    /// that keeps none shows every context.
    pub(super) fn context_selection(&self) -> Option<Vec<String>> {
        if self.filters.contexts.is_empty() {
            return None;
        }
        let workspace = self.workspace_contexts();
        let selected: Vec<String> = self
            .filters
            .contexts
            .iter()
            .filter(|context| workspace.contains(context))
            .cloned()
            .collect();
        (!selected.is_empty() && selected.len() < workspace.len()).then_some(selected)
    }

    pub(super) fn context_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let contexts = self.workspace_contexts();
        if contexts.len() < 2 {
            return None;
        }
        let selected = self.context_selection();
        let label = match &selected {
            None => "All".to_owned(),
            Some(selected) => selected.join(", "),
        };
        let popup = self
            .filters
            .context_open
            .then(|| self.context_popup(&contexts, selected.as_deref(), cx));
        Some(
            div()
                .relative()
                .child(
                    ui::outline_button()
                        .id("context-picker")
                        .test_support()
                        .aria_label("Filter contexts")
                        .h(px(32.))
                        .px(px(10.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .child(ui::muted(ui::xs("Context")))
                        .child(
                            ui::semibold(ui::xs(label.clone()))
                                .id("context-value")
                                .test_support()
                                .aria_label(label),
                        )
                        .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.filters.context_open = !this.filters.context_open;
                            cx.notify();
                        })),
                )
                .children(popup)
                .into_any_element(),
        )
    }

    fn context_popup(
        &self,
        contexts: &[String],
        selected: Option<&[String]>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        deferred(
            div()
                .id("context-options")
                .test_support()
                .occlude()
                .absolute()
                .top(px(34.))
                .left_0()
                .min_w(px(240.))
                .p(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::surface2())
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(ui::caps("Contexts", 10.))
                .children(contexts.iter().map(|context| {
                    let checked = selected.is_none_or(|selected| selected.contains(context));
                    let target = context.clone();
                    div()
                        .id(SharedString::from(format!("context-option:{context}")))
                        .test_support()
                        .role(gpui_kit::Role::CheckBox)
                        .aria_label(context.clone())
                        .aria_description(if checked { "checked" } else { "unchecked" })
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|row| row.bg(Tokens::muted()))
                        .child(super::resource_filter_controls::check_box(checked))
                        .child(ui::xs(context.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_context(&target, !checked, cx);
                        }))
                })),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn toggle_context(&mut self, context: &str, checked: bool, cx: &mut Context<Self>) {
        let available = self.workspace_contexts();
        let selected = self.context_selection().unwrap_or_default();
        // Unchecking the last selected context leaves none, which clears the
        // filter and shows every context, as `toggleContext` does on the web.
        self.filters.contexts = next_context_selection(&selected, &available, context, checked);
        self.table_page = 0;
        self.filter(cx);
        cx.notify();
    }
}
