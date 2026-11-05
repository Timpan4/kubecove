//! Toolbar controls for the resource filters in `resource_filters.rs`, laid
//! out as `ResourceBrowserTopBar.svelte` lays them out.
use super::Inspector;
use super::resource_filters::{HealthFilter, namespace_label, next_namespace_selection};
use crate::{theme::Tokens, ui};
use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, TestSupportExt, Window, deferred,
    div, prelude::*, px,
};

fn check_box(checked: bool) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(4.))
        .border_1()
        .border_color(Tokens::input())
        .bg(Tokens::input().opacity(0.3))
        .when(checked, |box_| {
            box_.child(ui::icon("check", 14., Tokens::foreground()))
        })
}

impl Inspector {
    /// The namespaces the toolbar shows as selected: the chosen set, or the
    /// sidebar's single namespace scope when no set is chosen.
    pub(super) fn selected_namespaces(&self) -> Vec<String> {
        if self.filters.namespaces.is_empty() {
            self.scope.namespace.iter().cloned().collect()
        } else {
            self.filters.namespaces.clone()
        }
    }

    /// `hasFilters` in `ResourceBrowserTopBar.svelte`.
    pub(super) fn filters_active(&self, cx: &Context<Self>) -> bool {
        !self.search.read(cx).value().is_empty()
            || self.filters.gitops_owner.is_some()
            || self.filters.health != HealthFilter::All
            || self
                .gitops_page
                .workspace
                .as_ref()
                .is_some_and(|state| state.filter.is_some())
    }

    pub(super) fn namespace_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.selected_namespaces();
        let label = namespace_label(&selected);
        let popup = self
            .filters
            .namespace_open
            .then(|| self.namespace_popup(&selected, cx));
        div()
            .relative()
            .child(
                ui::outline_button()
                    .id("namespace-picker")
                    .test_support()
                    .aria_label("Edit namespace scope")
                    .h(px(32.))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(ui::muted(ui::xs("Namespace")))
                    .child(
                        ui::semibold(ui::xs(label.clone()))
                            .id("namespace-value")
                            .test_support()
                            .aria_label(label),
                    )
                    .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.filters.namespace_open = !this.filters.namespace_open;
                        cx.notify();
                    })),
            )
            .children(popup)
            .into_any_element()
    }

    fn namespace_popup(&self, selected: &[String], cx: &mut Context<Self>) -> AnyElement {
        let query = self
            .filters
            .namespace_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let all = selected.is_empty();
        let names: Vec<String> = self
            .namespaces
            .iter()
            .map(|namespace| namespace.name.clone())
            .filter(|name| name.to_lowercase().contains(&query))
            .collect();
        let empty_message = if self.namespaces.is_empty() {
            "No namespaces found"
        } else {
            "No matching namespaces"
        };
        deferred(
            div()
                .id("namespace-popup")
                .occlude()
                .absolute()
                .top(px(34.))
                .left_0()
                .w(px(344.))
                .p(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::surface2())
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(ui::caps("Namespaces", 10.))
                .child(
                    ui::outline_button()
                        .id("select-all-namespaces")
                        .test_support()
                        .aria_label("Select all namespaces")
                        .h(px(32.))
                        .child(ui::xs("Select all namespaces"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_all_namespaces(window, cx);
                        })),
                )
                .child(
                    Input::new(&self.filters.namespace_search)
                        .id("namespace-search")
                        .aria_label("Search namespaces")
                        .h(px(32.))
                        .prefix(ui::icon("search", 16., Tokens::muted_foreground()))
                        .text_xs(),
                )
                .child(
                    div()
                        .id("namespace-options")
                        .test_support()
                        .h(px(224.))
                        .mt(px(4.))
                        .p(px(4.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::background().opacity(0.3))
                        .overflow_y_scroll()
                        .children(
                            names
                                .is_empty()
                                .then(|| ui::muted(ui::xs(empty_message)).p(px(8.))),
                        )
                        .children(names.into_iter().map(|name| {
                            let checked = all || selected.contains(&name);
                            div()
                                .id(SharedString::from(format!("namespace-option:{name}")))
                                .test_support()
                                .role(gpui_kit::Role::CheckBox)
                                .aria_label(name.clone())
                                .aria_description(if checked { "checked" } else { "unchecked" })
                                .h(px(32.))
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .cursor_pointer()
                                .hover(|row| row.bg(Tokens::muted()))
                                .child(check_box(checked))
                                .child(ui::xs(name.clone()))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.toggle_namespace(name.clone(), !checked, window, cx);
                                }))
                        })),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn toggle_namespace(
        &mut self,
        namespace: String,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        let available: Vec<String> = self
            .namespaces
            .iter()
            .map(|item| item.name.clone())
            .collect();
        let next =
            next_namespace_selection(&self.selected_namespaces(), &available, &namespace, checked);
        self.set_namespace_selection(next, window, cx);
    }

    fn select_all_namespaces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_leave(cx) {
            return;
        }
        self.set_namespace_selection(Vec::new(), window, cx);
    }

    fn set_namespace_selection(
        &mut self,
        next: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.filters.namespaces = next;
        self.close_detail(cx);
        self.table_page = 0;
        if self.scope.namespace.take().is_some() {
            // The toolbar filters namespaces client-side, so read them all.
            self.load_scope(false, window, cx);
        } else {
            self.filter(cx);
        }
        cx.notify();
    }

    pub(super) fn gitops_owner_select(&self, cx: &mut Context<Self>) -> AnyElement {
        let label = self
            .filters
            .gitops_owner
            .as_ref()
            .and_then(|key| self.filters.owners.iter().find(|(owner, _)| owner == key))
            .map_or_else(
                || "All GitOps owners".to_owned(),
                |(_, label)| label.clone(),
            );
        let popup = self.filters.owner_open.then(|| self.gitops_owner_popup(cx));
        div()
            .relative()
            .child(
                ui::outline_button()
                    .id("gitops-owner")
                    .test_support()
                    .aria_label(label.clone())
                    .h(px(32.))
                    .px(px(8.))
                    .gap(px(8.))
                    .bg(Tokens::background().opacity(0.7))
                    .child(ui::xs(label))
                    .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.filters.owner_open = !this.filters.owner_open;
                        cx.notify();
                    })),
            )
            .children(popup)
            .into_any_element()
    }

    fn gitops_owner_popup(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.filters.gitops_owner.clone();
        let options = std::iter::once(("All GitOps owners".to_owned(), None)).chain(
            self.filters
                .owners
                .iter()
                .map(|(key, label)| (label.clone(), Some(key.clone()))),
        );
        deferred(
            div()
                .id("gitops-owner-options")
                .test_support()
                .occlude()
                .absolute()
                .top(px(34.))
                .right_0()
                .min_w(px(240.))
                .max_w(px(360.))
                .p(px(4.))
                .flex()
                .flex_col()
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::surface2())
                .shadow_lg()
                .children(options.map(|(label, key)| {
                    let checked = key == selected;
                    let id = format!("gitops-owner-option:{}", key.as_deref().unwrap_or("all"));
                    div()
                        .id(SharedString::from(id))
                        .test_support()
                        .aria_label(label.clone())
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|row| row.bg(Tokens::muted()))
                        .child(ui::xs(label))
                        .when(checked, |row| {
                            row.child(ui::icon("check", 14., Tokens::foreground()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.choose_gitops_owner(key.clone(), cx);
                        }))
                })),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn choose_gitops_owner(&mut self, owner: Option<String>, cx: &mut Context<Self>) {
        self.filters.gitops_owner = owner;
        self.filters.owner_open = false;
        self.table_page = 0;
        self.filter(cx);
    }

    /// One health summary button, pressed while its filter is active.
    pub(super) fn health_stat(
        &self,
        filter: HealthFilter,
        label: &'static str,
        value: usize,
        tone: Option<Hsla>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.filters.health == filter;
        let color = if value == 0 {
            Tokens::muted_foreground().opacity(0.7)
        } else {
            tone.unwrap_or(Tokens::foreground())
        };
        div()
            .id(SharedString::from(format!("health:{label}")))
            .test_support()
            .aria_label(label)
            .aria_description(if active { "pressed" } else { "unpressed" })
            .cursor_pointer()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(2.))
            .min_w_0()
            .min_h(px(48.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .border_1()
            .border_color(if active {
                Tokens::primary().opacity(0.5)
            } else {
                Tokens::border()
            })
            // The outline button's `dark:bg-input/30` wins over its base background.
            .bg(Tokens::input().opacity(0.3))
            // The button's `text-xs/relaxed` leading (1.625) also applies to the 10px label.
            .child(ui::caps(label, 10.).line_height(px(16.25)).truncate())
            .child(
                ui::xs(value.to_string())
                    .line_height(px(19.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(color),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.choose_health(filter, cx);
            }))
            .into_any_element()
    }

    fn choose_health(&mut self, filter: HealthFilter, cx: &mut Context<Self>) {
        self.filters.health = filter;
        self.table_page = 0;
        self.filter(cx);
    }

    pub(super) fn clear_filters_button(&self, cx: &mut Context<Self>) -> AnyElement {
        ui::outline_button()
            .id("clear-resource-filters")
            .test_support()
            .aria_label("Clear")
            .h(px(32.))
            .px(px(8.))
            .bg(Tokens::background().opacity(0.7))
            .child(ui::icon("x", 12., Tokens::foreground()))
            .child(ui::medium(ui::xs("Clear")))
            .on_click(cx.listener(|this, _, window, cx| {
                this.clear_resource_filters(window, cx);
            }))
            .into_any_element()
    }

    /// Clears the search, GitOps owner and health filters, as `clearFilters`
    /// does on the web. The namespace and kind selections stay.
    fn clear_resource_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.filters.clear_owner_and_health();
        if let Some(state) = &mut self.gitops_page.workspace {
            state.filter = None;
        }
        self.table_page = 0;
        self.filter(cx);
        cx.notify();
    }

    /// The historical ReplicaSet checkbox. See `ResourceFilters` for why it
    /// currently hides nothing.
    pub(super) fn historical_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let checked = self.filters.hide_historical_replicasets;
        div()
            .flex()
            .child(
                div()
                    .id("hide-historical-replicasets")
                    .test_support()
                    .role(gpui_kit::Role::CheckBox)
                    .aria_label("Hide historical ReplicaSets")
                    .aria_description(if checked { "checked" } else { "unchecked" })
                    .flex_none()
                    .mt(px(8.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(Tokens::border())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(check_box(checked))
                    .child(ui::muted(ui::xs("Hide historical ReplicaSets")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filters.hide_historical_replicasets = !checked;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
