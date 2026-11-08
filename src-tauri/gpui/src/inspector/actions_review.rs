//! Review dialog of the Actions tab, mirroring the server dry-run review in
//! `ResourceActionsTab.svelte` (ADR 0004): the exact target, the API server
//! result, the typed-name delete confirmation and the guarded execute button.

use super::Inspector;
use super::actions_state::{Review, ReviewStatus};
use super::controls::{Variant, button, diff_pre};
use super::detail::Detail;
use crate::actions;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::component::{WindowExt, input::Input};
use gpui_kit::{App, Context, Div, Entity, FontWeight, Window, div, prelude::*, px};
use kubecove_lib::models::ResourceActionKind;

/// `DialogDescription` for the review's current status.
fn review_description(review: &Review) -> String {
    if let Some(preview) = &review.preview {
        return preview.effect.clone();
    }
    match review.status {
        ReviewStatus::Executing => "Applying the reviewed action…",
        ReviewStatus::Checking => "Checking this exact resource with the API server…",
        ReviewStatus::Ready | ReviewStatus::Failed => {
            "The API server could not validate this action."
        }
    }
    .into()
}

fn review_body(review: &Review, entity: &Entity<Inspector>, cx: &App) -> Div {
    let busy = review.busy();
    let close = {
        let entity = entity.clone();
        move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
            entity.update(cx, |this, cx| this.close_review(window, cx));
        }
    };
    let executing = review.status == ReviewStatus::Executing;
    let mut body = div()
        .flex()
        .flex_col()
        .text_sm()
        .child(
            ui::muted(ui::sm(review_description(review)))
                .id("review-description")
                .test_support()
                .aria_label(review_description(review)),
        )
        .child(
            div().flex().justify_end().child(
                button(Variant::Ghost, 24., executing)
                    .id("review-close")
                    .test_support()
                    .aria_label("Close review")
                    .child(if busy { "Cancel check" } else { "Close review" })
                    .when(!executing, |button| button.on_click(close.clone())),
            ),
        );
    if let Some(error) = &review.error {
        let retry = if review.execution_attempted {
            "Refresh this resource before retrying. "
        } else {
            ""
        };
        let text = format!("Could not complete the check or confirm the action. {retry}{error}");
        body = body.child(
            ui::xs(text.clone())
                .id("review-error")
                .test_support()
                .aria_label(text)
                .role(gpui_kit::Role::Alert)
                .mt(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::destructive().opacity(0.4))
                .bg(Tokens::destructive().opacity(0.1))
                .text_color(Tokens::destructive()),
        );
    }
    if busy {
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(32.))
                .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
                .child(ui::muted(ui::sm(if executing {
                    "Applying the reviewed action"
                } else {
                    "Checking with the API server"
                }))),
        );
    } else if review.preview.is_some() {
        body = body.child(
            div()
                .mt(px(16.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::muted().opacity(0.15))
                .child(
                    ui::medium(ui::xs("API server result"))
                        .px(px(12.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(Tokens::border()),
                )
                .child(
                    div()
                        .id("review-diff")
                        .test_support()
                        .aria_label(review.diff_text.clone())
                        .max_h(px(450.))
                        .overflow_y_scroll()
                        .child(diff_pre("review-diff-lines", &review.diff)),
                ),
        );
        if review.action == ResourceActionKind::Delete {
            body = body.child(
                div()
                    .mt(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(ui::xs(format!("Type {} to confirm deletion", review.name)))
                    .child(
                        Input::new(&review.confirmation)
                            .id("review-confirmation")
                            .aria_label("Type resource name to confirm")
                            .text_sm()
                            .h(px(32.)),
                    ),
            );
        }
    }
    let can_execute = review.can_execute(cx);
    let execute = {
        let entity = entity.clone();
        move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
            entity.update(cx, |this, cx| this.execute_review(window, cx));
        }
    };
    let delete = review.action == ResourceActionKind::Delete;
    body.child(
        div()
            .mt(px(16.))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .child(
                button(Variant::Outline, 28., executing)
                    .id("review-cancel")
                    .test_support()
                    .child(if busy { "Cancel check" } else { "Cancel" })
                    .when(!executing, |button| button.on_click(close)),
            )
            .when(review.preview.is_some(), |footer| {
                let label = if executing {
                    "Applying…"
                } else if delete {
                    "Delete"
                } else {
                    "Apply"
                };
                let variant = if delete {
                    Variant::Destructive
                } else {
                    Variant::Default
                };
                footer.child(
                    button(variant, 28., !can_execute)
                        .id("review-execute")
                        .test_support()
                        .aria_label(label)
                        .font_weight(FontWeight::MEDIUM)
                        .child(label)
                        .when(can_execute, |button| button.on_click(execute)),
                )
            }),
    )
}

impl Inspector {
    /// The dialog reads the review from the inspector each time it renders,
    /// so review changes call `window.refresh()`.
    pub(super) fn open_review_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        let on_close = entity.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let Detail::Ready(detail) = &entity.read(cx).detail else {
                return dialog;
            };
            let Some(review) = detail
                .actions
                .as_ref()
                .and_then(|actions| actions.review.as_ref())
            else {
                return dialog;
            };
            let title = if review.action == ResourceActionKind::Delete {
                format!("Review deletion of {}", review.name)
            } else {
                format!(
                    "Review {} for {}",
                    actions::action_name(review.action),
                    review.name
                )
            };
            let close = on_close.clone();
            dialog
                .w(px(896.))
                .title(
                    ui::semibold(ui::text(title.clone(), 16., 24.))
                        .id("review-title")
                        .test_support()
                        .aria_label(title),
                )
                .on_close(move |_, window, cx| {
                    close.update(cx, |this, cx| this.close_review(window, cx));
                })
                .child(review_body(review, &entity, cx))
        });
    }
}
