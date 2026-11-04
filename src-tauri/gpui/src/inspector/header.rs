//! Workspace header, mirroring `WorkspaceHeader.svelte` and `ClusterSelector.svelte`.

use super::{Inspector, Page};
use crate::theme::{MONO, Tokens};
use crate::ui;
use gpui_kit::component::select::Select;
use gpui_kit::{Context, FontWeight, div, prelude::*, px};

impl Inspector {
    pub(super) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = self.dirty(cx);
        let location = match self.page {
            Page::Overview => self.workspace_name.clone(),
            Page::Resources => "Resources".into(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.))
            .h(px(48.))
            .px(px(16.))
            .border_b_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                ui::muted(ui::xs("CLUSTER CONTEXT:"))
                                    .font_weight(FontWeight::BOLD)
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Select::new(&self.context_picker)
                                    .id("context")
                                    .disabled(dirty)
                                    // `SelectTrigger` default size is `h-7`; the call site adds `text-xs`.
                                    .h(px(28.))
                                    .text_xs()
                                    .w_auto()
                                    .min_w(px(94.))
                                    .bg(Tokens::background().opacity(0.5)),
                            ),
                    )
                    .child(ui::icon("chevron-right", 14., Tokens::muted_foreground()))
                    .child(
                        ui::muted(ui::xs(location))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .child(ui::ghost_icon("folder-open", 28., 14.))
                    .child(ui::ghost_icon("settings", 28., 14.))
                    .child(
                        ui::outline_button()
                            .h(px(24.))
                            .px(px(12.))
                            .gap(px(8.))
                            .bg(Tokens::card())
                            .child(ui::icon("search", 14., Tokens::muted_foreground()))
                            .child(ui::muted(ui::xs("Search resources...")))
                            .child(
                                div()
                                    .px(px(4.))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(Tokens::border())
                                    .bg(Tokens::muted())
                                    .child(ui::muted(ui::xs("Ctrl K").font_family(MONO))),
                            ),
                    ),
            )
    }
}
