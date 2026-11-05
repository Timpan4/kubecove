//! The Namespaces page, mirroring `NamespaceList.svelte`: a table of the
//! context's namespaces with ages and created times, and its empty, error and
//! loading states.

use super::{Inspector, Page, friendly_error::Operation};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, SharedString, Task, TestSupportExt, div, prelude::*, px,
};
use kubecove_lib::models::{AppError, NamespaceSummary};

#[derive(Default)]
enum NamespaceRead {
    #[default]
    Idle,
    Loading,
    Ready(Vec<NamespaceSummary>),
    Failed(AppError),
}

#[derive(Default)]
pub(super) struct NamespaceListState {
    read: NamespaceRead,
    task: Option<Task<()>>,
}

impl Inspector {
    /// `View all` opens the page and clears the tree's leaf selection, as the
    /// web's `selectFromTree({ type: "section", section: "namespaces" })` does.
    pub(super) fn open_namespace_list(&mut self, cx: &mut Context<Self>) {
        self.open_page(Page::Namespaces, cx);
        if self.page == Page::Namespaces {
            self.sidebar_tree.selected = None;
        }
    }

    /// Reads the namespaces each time the page loads and when the context changes.
    /// A new read drops the previous task, which cancels it.
    pub(super) fn load_namespace_list(&mut self, cx: &mut Context<Self>) {
        let context = self.scope.context.clone();
        if context.is_empty() {
            self.namespace_list = NamespaceListState::default();
            cx.notify();
            return;
        }
        self.namespace_list.read = NamespaceRead::Loading;
        let read = self.backend.namespaces(context);
        let read = self.timings.measure("list_namespaces", read);
        self.namespace_list.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                this.namespace_list.read = match result {
                    Ok(namespaces) => NamespaceRead::Ready(namespaces),
                    Err(error) => NamespaceRead::Failed(error),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn namespace_list_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = if self.scope.context.is_empty() {
            no_context()
        } else {
            match &self.namespace_list.read {
                NamespaceRead::Idle | NamespaceRead::Loading => loading(),
                NamespaceRead::Failed(error) => self.page_read_failure(
                    "Failed to load namespaces",
                    error,
                    Operation::Resources,
                    cx,
                ),
                NamespaceRead::Ready(namespaces) if namespaces.is_empty() => empty(),
                NamespaceRead::Ready(namespaces) => {
                    self.namespace_table(namespaces).into_any_element()
                }
            }
        };
        let count = match &self.namespace_list.read {
            NamespaceRead::Ready(namespaces) if !namespaces.is_empty() => Some(namespaces.len()),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .w_full()
            .max_w(px(1024.))
            .mx_auto()
            .p(px(16.))
            .when_some(count, |page, count| {
                page.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(ui::semibold(ui::sm("Namespaces")))
                        .child(ui::outline_badge(count.to_string())),
                )
            })
            .child(body)
            .into_any_element()
    }

    fn namespace_table(&self, namespaces: &[NamespaceSummary]) -> Div {
        let mut table = div()
            .flex()
            .flex_col()
            .w_full()
            .border_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .h(px(32.))
                    .px(px(12.))
                    .bg(Tokens::card())
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        ui::muted(ui::xs("Name"))
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(ui::muted(ui::xs("Age")).w(px(96.)))
                    .child(ui::muted(ui::xs("Created")).w(px(224.))),
            );
        for namespace in namespaces {
            let created = self.timestamp_label(&namespace.age, namespace.created_at.as_deref());
            table = table.child(
                div()
                    .id(SharedString::from(format!(
                        "namespace-row:{}",
                        namespace.name
                    )))
                    .test_support()
                    .aria_label(namespace.name.clone())
                    .aria_description(namespace.age.clone())
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .h(px(32.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        ui::xs(namespace.name.clone())
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM),
                    )
                    .child(ui::xs(namespace.age.clone()).w(px(96.)).truncate())
                    .child(ui::xs(created).w(px(224.)).truncate()),
            );
        }
        table
    }
}

fn no_context() -> AnyElement {
    empty_state(
        "No cluster context",
        "Select a workspace with a cluster context.",
    )
}

fn empty() -> AnyElement {
    empty_state(
        "No namespaces found",
        "No namespaces returned for this context.",
    )
}

fn empty_state(title: &'static str, description: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .min_h(px(160.))
        .border_1()
        .border_color(Tokens::border())
        .child(
            ui::semibold(ui::sm(title))
                .id(SharedString::from(format!("namespaces-empty:{title}")))
                .test_support()
                .aria_label(title),
        )
        .child(ui::muted(ui::xs(description)))
        .into_any_element()
}

fn loading() -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .min_h(px(160.))
        .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
        .child(ui::muted(ui::sm("Loading namespaces")))
        .into_any_element()
}
