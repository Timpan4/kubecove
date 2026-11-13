//! The delete target and what follows it, mirroring `DeleteReviewSummary.svelte`:
//! shown above the folded dry-run diff so the user can confirm the target
//! without typing its name.

use crate::targets::TestSupportExt;
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use gpui_kit::{Div, FontWeight, div, prelude::*, px};
use kubecove_lib::models::ResourceActionPreview;

/// The `What happens` line about the controller, shared with the web text.
pub(super) fn consequence(preview: &ResourceActionPreview) -> String {
    let kind = &preview.target.kind;
    match &preview.controller {
        Some(controller) if controller.recreates => format!(
            "{} {} normally creates a replacement {kind}.",
            controller.kind, controller.name
        ),
        Some(controller) => format!(
            "{} {} controls this {kind}, but KubeCove cannot confirm it will create a replacement.",
            controller.kind, controller.name
        ),
        None => "No controlling owner was found. The delete is permanent unless other automation, such as GitOps, recreates it.".to_owned(),
    }
}

fn row(label: &'static str, id: &'static str, value: String, mono: bool) -> Div {
    div()
        .flex()
        .gap(px(12.))
        .child(ui::muted(ui::xs(label)).w(px(112.)).flex_none())
        .child(
            ui::xs(value.clone())
                .min_w_0()
                .when(mono, |value| value.font_family(MONO))
                .id(id)
                .test_support()
                .aria_label(value),
        )
}

fn dot(color: gpui_kit::Hsla) -> Div {
    div()
        .mt(px(5.))
        .size(px(6.))
        .flex_none()
        .rounded_full()
        .bg(color)
}

pub(super) fn delete_summary(preview: &ResourceActionPreview) -> Div {
    let target = &preview.target;
    let mut rows = vec![row(
        "Context",
        "delete-target-context",
        target.cluster_context.clone(),
        false,
    )];
    if let Some(namespace) = &target.namespace {
        rows.push(row(
            "Namespace",
            "delete-target-namespace",
            namespace.clone(),
            true,
        ));
    }
    if let Some(controller) = &preview.controller {
        rows.push(row(
            "Controlled by",
            "delete-target-controller",
            format!("{} {}", controller.kind, controller.name),
            true,
        ));
    }
    let recreates = preview
        .controller
        .as_ref()
        .is_some_and(|controller| controller.recreates);
    let consequence = consequence(preview);
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .mt(px(16.))
                .px(px(16.))
                .py(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::card())
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(ui::status_badge(ui::Tone::Neutral, target.kind.clone()))
                        .child(
                            ui::outline_badge(target.cluster_context.clone())
                                .text_color(Palette::amber300()),
                        ),
                )
                .child(
                    ui::sm(target.name.clone())
                        .mt(px(8.))
                        .font_family(MONO)
                        .font_weight(FontWeight::SEMIBOLD)
                        .id("delete-target-name")
                        .test_support()
                        .aria_label(target.name.clone()),
                )
                .child(
                    div()
                        .mt(px(8.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(rows),
                ),
        )
        .child(
            div()
                .mt(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::muted().opacity(0.15))
                .child(
                    ui::medium(ui::xs("What happens"))
                        .px(px(12.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(Tokens::border()),
                )
                .child(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(dot(Tokens::destructive()))
                                .child(ui::xs(format!(
                                    "The {} is removed from the cluster.",
                                    target.kind
                                ))),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(dot(if recreates {
                                    Palette::emerald400()
                                } else {
                                    Palette::amber400()
                                }))
                                .child(
                                    ui::xs(consequence.clone())
                                        .id("delete-consequence")
                                        .test_support()
                                        .aria_label(consequence),
                                ),
                        ),
                ),
        )
}
