//! The RBAC inspector's right column: `RbacReviewPanel.svelte`, the verifier,
//! the latest verification and input coverage.

use super::Inspector;
use super::controls::{Variant, button};
use super::rbac_inspector::caps;
use super::rbac_queue::review_badge;
use crate::rbac::permissions::Identity;
use crate::rbac::review::{self, Disposition, ReviewRecord};
use crate::rbac::{self, CockpitItem};
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::Textarea;
use gpui_kit::{Context, Div, Window, div, prelude::*, px};
use kubecove_lib::models::{RbacCoverageStatus, RbacInspectionSummary};

impl Inspector {
    /// `RbacReviewPanel` resets its editor when the object or saved review changes.
    pub(super) fn sync_rbac_review_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.rbac;
        let (Some(data), Some(item)) = (state.data(), state.selected_item()) else {
            return;
        };
        let record = review::find(&state.reviews, &data.cluster, &item.key);
        let key = (
            data.cluster.clone(),
            item.key.clone(),
            record.map(|record| record.reviewed_at.clone()),
        );
        if state.review_editor_key.as_ref() == Some(&key) {
            return;
        }
        let note = record.map_or_else(String::new, |record| record.note.clone());
        state.review_editor_key = Some(key);
        state.verifier.note_error = None;
        state
            .verifier
            .note
            .update(cx, |input, cx| input.set_value(note, window, cx));
    }

    /// `RbacReviewPanel`'s `save`.
    fn save_rbac_review(&mut self, disposition: Disposition, cx: &mut Context<Self>) {
        self.ensure_active_workspace();
        let state = &mut self.rbac;
        let (Some(data), Some(item)) = (state.data(), state.selected_item()) else {
            return;
        };
        let record = ReviewRecord {
            cluster_context: data.cluster.clone(),
            object_key: item.key.clone(),
            evidence_fingerprint: review::fingerprint(data, item).unwrap_or_default(),
            disposition,
            note: state.verifier.note.read(cx).value().to_string(),
            reviewed_at: chrono::Utc::now().to_rfc3339(),
        };
        state.verifier.note_error = review::upsert(&mut state.reviews, record).err();
        if state.verifier.note_error.is_none() {
            self.save_preferences(cx);
        }
        cx.notify();
    }

    fn clear_rbac_review(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.rbac;
        if let (Some(data), Some(item)) = (state.data(), state.selected_item()) {
            let (cluster, key) = (data.cluster.clone(), item.key.clone());
            review::remove(&mut state.reviews, &cluster, &key);
            self.save_preferences(cx);
        }
        cx.notify();
    }

    pub(super) fn rbac_aside(
        &self,
        data: &RbacInspectionSummary,
        item: &CockpitItem,
        identity: &Identity,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(gpui_kit::relative(0.34))
            .min_w(px(272.))
            .gap(px(12.))
            .children(self.rbac_review_panel(data, item, cx))
            .child(self.rbac_verifier(identity, cx))
            .children(self.rbac_latest(data))
            .child(
                div()
                    .p(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .child(caps("Input coverage"))
                    .child(div().flex().flex_col().gap(px(6.)).mt(px(8.)).children(
                        data.coverage.iter().map(|entry| {
                            let status = rbac::serde_label(&entry.status);
                            let tone = match entry.status {
                                RbacCoverageStatus::Complete => Palette::emerald300(),
                                RbacCoverageStatus::Partial => Palette::amber300(),
                                RbacCoverageStatus::Unavailable => Tokens::destructive(),
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    ui::xs(rbac::serde_label(&entry.family))
                                        .flex_1()
                                        .min_w_0()
                                        .truncate(),
                                )
                                .child(ui::xs(capitalize(&status)).text_color(tone))
                                .child(
                                    ui::muted(ui::xs(entry.count.to_string()))
                                        .w(px(24.))
                                        .text_right(),
                                )
                        }),
                    )),
            )
    }

    fn rbac_review_panel(
        &self,
        data: &RbacInspectionSummary,
        item: &CockpitItem,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !item.category.reviewable() {
            return None;
        }
        let state = &self.rbac;
        let subjects = review::subject_labels(data, item);
        let record = review::find(&state.reviews, &data.cluster, &item.key);
        let stale = review::status(record, data, item) == review::Status::Stale;
        let reviewed_at = record.map(|record| record.reviewed_at.clone());
        let empty = subjects.is_empty();
        Some(
            ui::card()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(10.))
                        .border_b_1()
                        .border_color(Tokens::border())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(ui::semibold(ui::sm("Reviewed context")))
                                .child(
                                    ui::muted(ui::xs(
                                        "Manual annotation. Risk and evidence stay unchanged.",
                                    ))
                                    .mt(px(4.)),
                                ),
                        )
                        .child(div().id("rbac-review-status").test_support()
                            .aria_label(match (review::status(record, data, item), record.map(|record| record.disposition)) {
                                (review::Status::Stale, _) => "Review stale",
                                (review::Status::Active, Some(Disposition::Expected)) => "Expected",
                                (review::Status::Active, Some(Disposition::Anomalous)) => "Anomalous",
                                _ => "Unreviewed",
                            })
                            .children(review_badge(data, &state.reviews, item))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .p(px(12.))
                        .child(
                            div().child(caps("Affected identities")).child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(4.))
                                    .mt(px(6.))
                                    .children(subjects.into_iter().map(|subject| {
                                        ui::outline_badge(subject).font_family(MONO)
                                    }))
                                    .when(empty, |list| {
                                        list.child(ui::muted(ui::xs(
                                            "No subjects found in loaded binding evidence.",
                                        )))
                                    }),
                            ),
                        )
                        .when(stale, |panel| {
                            panel.child(
                                ui::xs("Evidence changed after this review. Re-review before treating its disposition as active.")
                                    .p(px(8.))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(Palette::amber500(0.35))
                                    .bg(Palette::amber500(0.1))
                                    .text_color(Palette::amber300()),
                            )
                        })
                        .child(
                            div().id("rbac-review-editor").test_support().child(Textarea::new(&state.verifier.note)
                                .accessibility_id("rbac-review-note")
                                .aria_label("RBAC review note")
                                .text_xs()
                                .h(px(64.))),
                        )
                        .when_some(state.verifier.note_error.clone(), |panel, error| {
                            panel.child(
                                ui::xs(error)
                                    .id("rbac-review-error")
                                    .role(gpui_kit::Role::Alert)
                                    .text_color(Tokens::destructive()),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(8.))
                                .child(
                                    button(Variant::Outline, 24., false)
                                        .id("rbac-mark-expected")
                                        .test_support()
                                        .child("Mark expected")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.save_rbac_review(Disposition::Expected, cx)
                                        })),
                                )
                                .child(
                                    button(Variant::Destructive, 24., false)
                                        .id("rbac-mark-anomalous")
                                        .test_support()
                                        .child("Mark anomalous")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.save_rbac_review(Disposition::Anomalous, cx)
                                        })),
                                )
                                .when(record.is_some(), |row| {
                                    row.child(
                                        button(Variant::Ghost, 24., false)
                                            .id("rbac-clear-review")
                                            .test_support()
                                            .child("Clear review")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.clear_rbac_review(cx)
                                            })),
                                    )
                                }),
                        )
                        .when_some(reviewed_at, |panel, at| {
                            panel.child(ui::muted(ui::xs(format!("Reviewed {at}."))))
                        }),
                ),
        )
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or(String::new(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}
