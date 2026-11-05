//! The namespace rail in `HelmView.svelte`.
use crate::inspector::{Inspector, delivery_ui as dui};
use crate::theme::Tokens;
use gpui_kit::{Context, Div, SharedString, TestSupportExt, Window, prelude::*};

impl Inspector {
    pub(super) fn clear_helm_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.helm_page
            .search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.filter_helm(cx);
    }

    /// The empty state's Clear filters: no search and All releases.
    pub(in crate::inspector) fn clear_helm_filters(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.helm_page.namespace = None;
        self.clear_helm_search(window, cx);
    }
    pub(super) fn helm_rail(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let beside = page.layout.rail_beside;
        let releases = page.releases();
        let entries = std::iter::once((None, releases.len())).chain(
            page.namespace_counts
                .iter()
                .map(|(namespace, count)| (Some(namespace.clone()), *count)),
        );
        let rows = entries
            .map(|(namespace, count)| {
                let label = namespace.clone().unwrap_or_else(|| "All releases".into());
                dui::rail_item(&label, count, page.namespace == namespace, false)
                    .id(SharedString::from(format!("helm-namespace:{label}")))
                    .test_support()
                    .aria_label(format!("{label} {count}"))
                    .cursor_pointer()
                    .when(page.namespace != namespace, |row| {
                        row.hover(|row| row.bg(Tokens::muted().opacity(0.6)))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_helm_namespace(namespace.clone(), cx)
                    }))
                    .into_any_element()
            })
            .collect();
        dui::rail(beside, vec![dui::rail_section(beside, "Namespaces", rows)])
    }
}
