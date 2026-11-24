//! The "Latest verification" panel from `RbacView.svelte`: the newest
//! exact-action review, its verdict and the request it sent.

use super::Review;
use crate::inspector::Inspector;
use crate::inspector::rbac::inspector::caps;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, div, prelude::*, px};
use kubecove_lib::models::{RbacAccessReviewOutcome, RbacInspectionSummary};

impl Inspector {
    pub(in crate::inspector::rbac) fn rbac_latest(
        &self,
        data: &RbacInspectionSummary,
    ) -> Option<AnyElement> {
        let (submitted, result, failure) = match &self.rbac.verifier.review {
            Review::None => return None,
            Review::Checking { submitted, .. } => (submitted, None, None),
            Review::Done(submitted, result) => (submitted, Some(result), None),
            Review::Failed(submitted, message) => (submitted, None, Some(message)),
        };
        let outcome = result.map(|result| match result.outcome {
            RbacAccessReviewOutcome::Allowed => ("allowed", "Allowed", Palette::emerald300()),
            RbacAccessReviewOutcome::Denied => ("denied", "Denied", Tokens::destructive()),
            RbacAccessReviewOutcome::NoOpinion => ("noOpinion", "No opinion", Palette::amber300()),
        });
        let evidence: gpui_kit::SharedString = format!(
            "{} · {}\n{}\n{}",
            data.cluster, submitted.timestamp, submitted.identity, submitted.target
        )
        .into();
        Some(
            div()
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .child(caps("Latest verification"))
                .when_some(outcome, |panel, (value, label, tone)| {
                    panel.child(
                        ui::semibold(ui::sm(label))
                            .id("rbac-verdict")
                            .test_support()
                            .aria_label(value)
                            .mt(px(8.))
                            .text_color(tone),
                    )
                })
                .when_some(
                    result.and_then(|result| result.reason.clone()),
                    |panel, reason| panel.child(ui::muted(ui::xs(reason)).mt(px(4.))),
                )
                .when_some(
                    result.and_then(|result| result.evaluation_error.clone()),
                    |panel, error| {
                        panel.child(ui::xs(error).mt(px(4.)).text_color(Tokens::destructive()))
                    },
                )
                .when_some(failure.cloned(), |panel, message| {
                    panel.child(
                        ui::xs(format!("Verifier unavailable · {message}"))
                            .id("rbac-verifier-error")
                            .test_support()
                            .aria_label(format!("Verifier unavailable · {message}"))
                            .mt(px(8.))
                            .text_color(Tokens::destructive()),
                    )
                })
                .child(
                    ui::muted(ui::text(evidence.clone(), 10., 14.))
                        .id("rbac-submitted")
                        .test_support()
                        .aria_label(evidence)
                        .mt(px(8.)),
                )
                .into_any_element(),
        )
    }
}
