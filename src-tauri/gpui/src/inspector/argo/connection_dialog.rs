//! The Argo CD connection dialog: the modal overlay, its focus trap, Escape and
//! outside-press dismissal, and its Close button.
use super::super::{
    Inspector,
    controls::{Variant, button},
};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use gpui_kit::{
    Context,
    base::FocusTrapElement,
    div,
    prelude::*,
    px, relative,
};

impl Inspector {
    pub(in crate::inspector) fn argo_connection_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let dialog_focus = self.argo.connections.dialog_focus.clone();
        self.argo.connections.dialog.then(|| {
            let overlay = div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Tokens::background().opacity(0.8))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation()
                })
                .child(
                    div()
                        .id("argo-connection-dialog")
                        .test_support()
                        .role(gpui_kit::Role::Dialog)
                        .aria_label("Argo CD connection")
                        .track_focus(&dialog_focus)
                        // Like the web's `w-full max-w-2xl`: a narrow window narrows the dialog instead of clipping its left edge.
                        .w_full()
                        .max_w(px(672.))
                        .max_h(relative(0.9))
                        .overflow_y_scroll()
                        .on_key_down(cx.listener(
                            |this, event: &gpui_kit::KeyDownEvent, _, cx| {
                                if event.keystroke.key == "escape" {
                                    this.cancel_argo_connection(cx);
                                    cx.stop_propagation();
                                }
                            },
                        ))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.cancel_argo_connection(cx)
                        }))
                        .child(self.argo_connection_content(cx))
                        .child(
                            button(Variant::Outline, 24., false)
                                .id("argo-connection-close")
                                .test_support()
                                .child("Close connection")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_argo_connection(cx)
                                })),
                        ),
                );
            overlay
                .id("argo-connection-overlay")
                .focus_trap("argo-connection-dialog", &dialog_focus)
                .into_any_element()
        })
    }
}
