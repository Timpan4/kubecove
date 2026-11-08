//! ResourceBrowser.svelte's stale-data notice and expandable refresh error.
use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{theme::Palette, ui};
use gpui_kit::{AnyElement, Context, div, prelude::*, px};

impl Inspector {
    pub(super) fn live_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let failed = self.live.label() == "Refresh failed";
        let label = match self.live.label() {
            "Live" | "Refreshing…" | "Not connected" => return None,
            "Refresh failed" => {
                "Refresh failed. Showing last observed state. Activity indicators are paused until live updates recover."
            }
            "Polling (limited)" => {
                "Live updates are unavailable. Readable resources refresh periodically. Activity indicators are paused."
            }
            "Live (limited)" => {
                "Live updates are limited. Readable resources without watch access refresh periodically. Activity indicators are paused."
            }
            _ => {
                "Showing last observed state. Activity indicators are paused until live updates recover."
            }
        };
        Some(
            div()
                .id("live-notice")
                .test_support()
                .aria_label(label)
                .role(gpui_kit::Role::Status)
                .flex()
                .flex_col()
                .flex_none()
                .rounded(px(6.))
                .border_1()
                .border_color(Palette::amber500(0.3))
                .text_color(Palette::amber300())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(12.))
                        .py(px(8.))
                        .when(failed, |row| {
                            row.child(ui::icon(
                                if self.live.error_open {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                },
                                12.,
                                Palette::amber300(),
                            ))
                        })
                        .child(ui::xs(label)),
                )
                .when(failed, |notice| {
                    notice
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.live.error_open = !this.live.error_open;
                            cx.notify();
                        }))
                })
                .when(failed && self.live.error_open, |notice| {
                    notice.child(
                        div().px(px(12.)).pb(px(12.)).children(
                            self.live
                                .messages()
                                .next()
                                .map(|message| ui::xs(message.clone())),
                        ),
                    )
                })
                .into_any_element(),
        )
    }
}
