//! `CopyableText.svelte`: a copy icon beside a value. Each copy writes the
//! clipboard and shows `Copied {label}: {value}` in the shared copy toast.
use super::Inspector;
use crate::{theme::Tokens, ui};
use gpui_kit::{
    ClipboardItem, Context, IntoElement, SharedString, Styled, TestSupportExt, div, prelude::*, px,
};

impl Inspector {
    pub(super) fn copy_labeled(&mut self, label: &str, value: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
        self.show_copy_toast(format!("Copied {label}: {value}"), cx);
    }
}

/// The 20 px copy button from `CopyableText.svelte`. Its accessible name is
/// `Copy {label}: {value}`, as on the web.
pub(super) fn copy_button(
    id: String,
    label: &'static str,
    value: String,
    cx: &mut Context<Inspector>,
) -> impl IntoElement {
    let name = format!("Copy {label}: {value}");
    div()
        .id(SharedString::from(id))
        .test_support()
        .aria_label(name)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(20.))
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|button| button.bg(Tokens::muted()).text_color(Tokens::foreground()))
        .child(ui::icon("copy", 12., Tokens::muted_foreground()))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.copy_labeled(label, value.clone(), cx);
        }))
}
