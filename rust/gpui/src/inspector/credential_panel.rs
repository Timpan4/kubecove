//! `CredentialFailureDetails.svelte`: the failed local credential plugin's
//! redacted output, the copy button for its terminal command, and the
//! sign-in details (ADR 0037). `feedback.rs` places it in the error panel.

use super::{Inspector, friendly_error, port_forward_rows::xs_button};
use crate::targets::TestSupportExt;
use crate::{
    theme::{MONO, Tokens},
    ui,
};
use gpui_kit::{AnyElement, Context, SharedString, div, prelude::*, px};
use kubecove_lib::models::CredentialFailure;

pub(super) fn section(
    id: &'static str,
    failure: &CredentialFailure,
    copied: Option<bool>,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let (icon, label) = if copied == Some(true) {
        ("check", "Copied")
    } else {
        ("copy", "Copy terminal command")
    };
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .mt(px(8.))
        .when_some(failure.plugin_message.clone(), |section, message| {
            section.child(
                ui::xs(message.clone())
                    .id(SharedString::from(format!("{id}:plugin-output")))
                    .test_support()
                    .aria_label("Plugin output")
                    .aria_description(message)
                    .font_family(MONO)
                    .text_size(px(11.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(4.))
                    .bg(Tokens::destructive().opacity(0.1))
                    .text_color(Tokens::destructive()),
            )
        })
        // Absent when the kubeconfig files cannot be named exactly in a shell.
        .when_some(failure.reproduce.clone(), |section, reproduce| {
            section.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        xs_button(icon, label, false)
                            .id(SharedString::from(format!("{id}:copy-command")))
                            .test_support()
                            .aria_label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_command(id, reproduce.clone(), cx);
                            })),
                    )
                    .when(copied == Some(false), |row| {
                        row.child(ui::muted(ui::xs("Copy failed")))
                    }),
            )
        })
        .child(details(id, failure))
        .into_any_element()
}

fn details(id: &'static str, failure: &CredentialFailure) -> impl IntoElement {
    let rows = friendly_error::credential_details(failure);
    let description = rows
        .iter()
        .map(|(label, value)| format!("{label} {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    div()
        .id(SharedString::from(format!("{id}:sign-in-details")))
        .test_support()
        .aria_label("Sign-in details")
        .aria_description(description)
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .gap(px(12.))
                .child(ui::muted(ui::xs(label)).text_size(px(11.)).w(px(72.)))
                .child(
                    ui::xs(value)
                        .font_family(MONO)
                        .text_size(px(11.))
                        .min_w_0()
                        .flex_1(),
                )
        }))
}
