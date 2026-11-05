//! Discovered kinds retain their API identity even before they contain objects.
use super::{Inspector, Page};
use crate::{theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, SharedString, TestSupportExt, div, prelude::*, px};

impl Inspector {
    pub(super) fn custom_kind_rows(&self, picker: bool, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if !self.persisted.preferences.show_custom_resources {
            return Vec::new();
        }
        if !picker {
            let mut groups = self
                .live
                .kinds
                .iter()
                .map(|kind| kind.group.clone())
                .collect::<Vec<_>>();
            groups.sort();
            groups.dedup();
            let mut rows = Vec::new();
            for group in groups {
                let expanded = self.live.groups.contains(&group);
                let selected = group.clone();
                rows.push(
                    div()
                        .id(SharedString::from(format!("custom-group:{group}")))
                        .test_support()
                        .aria_label(group.clone())
                        .flex()
                        .items_center()
                        .h(px(26.))
                        .pl(px(28.))
                        .gap(px(4.))
                        .cursor_pointer()
                        .child(ui::icon(
                            if expanded {
                                "chevron-down"
                            } else {
                                "chevron-right"
                            },
                            12.,
                            Tokens::muted_foreground(),
                        ))
                        .child(ui::xs(group.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.live.groups.remove(&selected) {
                                this.live.groups.insert(selected.clone());
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                if expanded {
                    rows.extend(self.kind_rows(false, Some(&group), cx));
                }
            }
            return rows;
        }
        self.kind_rows(true, None, cx)
    }

    fn kind_rows(
        &self,
        picker: bool,
        group: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.live
            .kinds
            .iter()
            .filter(|kind| group.is_none_or(|group| kind.group == group))
            .map(|kind| {
                let kind = kind.clone();
                let label = kind.kind.clone();
                div()
                    .id(SharedString::from(format!(
                        "{}:{}:{}",
                        if picker { "pick-kind" } else { "custom-kind" },
                        kind.api_version,
                        kind.kind
                    )))
                    .test_support()
                    .aria_label(label.clone())
                    .aria_description(format!("{} / {}", kind.api_version, kind.plural))
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .pl(px(36.))
                    .pr(px(8.))
                    .cursor_pointer()
                    .hover(|row| row.bg(Tokens::muted()))
                    .child(ui::xs(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !this.can_leave(cx) {
                            return;
                        }
                        this.close_detail(cx);
                        if !picker && this.scope.namespace.is_some() {
                            this.select_namespace(None, window, cx);
                        }
                        this.page = Page::Resources;
                        this.palette.kind = None;
                        this.live.selected = Some(kind.clone());
                        this.live.kind_locked = !picker;
                        this.live.picker_open = false;
                        this.filter(cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn kinds_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.live.kind_locked
            && let Some(kind) = &self.live.selected
        {
            return ui::outline_badge(format!("Kind {}", kind.kind)).into_any_element();
        }
        div()
            .relative()
            .child(
                ui::outline_button()
                    .id("kinds-picker")
                    .test_support()
                    .aria_label("Kinds")
                    .h(px(32.))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(ui::muted(ui::xs("Kinds")))
                    .child(ui::xs(self.live.selected.as_ref().map_or_else(
                        || crate::model::kinds_label(&self.scope_kinds()),
                        |kind| kind.kind.clone(),
                    )))
                    .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.live.picker_open = !this.live.picker_open;
                        cx.notify();
                    })),
            )
            .when(self.live.picker_open, |picker| {
                picker.child(
                    div()
                        .absolute()
                        .top(px(34.))
                        .left_0()
                        .min_w(px(240.))
                        .p(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::card())
                        .shadow_lg()
                        .children(self.custom_kind_rows(true, cx)),
                )
            })
            .into_any_element()
    }
}
