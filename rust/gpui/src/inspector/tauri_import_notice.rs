//! The Tauri import's report (ADR 0036) in Settings > General, kept in
//! `live-sessions.json` until the user dismisses it. Only the desktop client
//! imports Tauri snapshots, so the web client has no equivalent.
use super::{
    Inspector,
    controls::{Variant, button},
};
use crate::{targets::TestSupportExt, theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, div, prelude::*, px};

const TITLE: &str = "Tauri settings import";
const DISMISS: &str = "Dismiss Tauri import notice";

impl Inspector {
    pub(super) fn tauri_import_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.persisted.tauri_migration_notice.clone()?;
        Some(
            div()
                .id("settings-notice:tauri-import")
                .test_support()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(16.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::card())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(4.))
                        .child(ui::medium(ui::sm(TITLE)).line_height(px(20.)))
                        .child(
                            ui::muted(ui::xs(notice.clone()))
                                .id("settings-notice:tauri-import-message")
                                .test_support()
                                .aria_label(notice)
                                .line_height(px(18.)),
                        ),
                )
                .child(
                    button(Variant::Outline, 28., false)
                        .id("settings-button:Dismiss Tauri import notice")
                        .test_support()
                        .aria_label(DISMISS)
                        .child("Dismiss")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let notice = this.persisted.tauri_migration_notice.take();
                            this.save_preferences(cx);
                            // A failed save keeps the notice, so a later dismissal can retry.
                            if this.persistence_error.is_some() {
                                this.persisted.tauri_migration_notice = notice;
                            }
                        })),
                )
                .into_any_element(),
        )
    }
}
