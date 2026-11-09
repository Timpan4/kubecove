//! The exec terminal's right-click menu. The web shows the platform webview
//! menu here, and these items are its terminal entries: Copy, Paste and Select all.
//! Each item calls the method its keyboard gesture uses in `exec_keys`.
use super::Inspector;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::{Div, Entity, IntoElement, ParentElement, Stateful};

/// Puts the terminal in `container` with a context menu. Copy is disabled while nothing is selected.
pub(super) fn with_terminal_menu(
    container: Stateful<Div>,
    terminal: impl IntoElement,
    owner: Entity<Inspector>,
    has_selection: bool,
) -> impl IntoElement {
    container.child(terminal).context_menu(move |menu, _, _| {
        let copy = owner.clone();
        let paste = owner.clone();
        let select_all = owner.clone();
        menu.item(
            PopupMenuItem::new("Copy")
                .disabled(!has_selection)
                .on_click(move |_, _, cx| copy.update(cx, |this, cx| this.copy_exec(cx))),
        )
        .item(
            PopupMenuItem::new("Paste")
                .on_click(move |_, _, cx| paste.update(cx, |this, cx| this.paste_clipboard(cx))),
        )
        .separator()
        .item(
            PopupMenuItem::new("Select all").on_click(move |_, _, cx| {
                select_all.update(cx, |this, cx| this.select_all_exec(cx))
            }),
        )
    })
}
