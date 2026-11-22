//! Shared steps for the delete review: its target summary, the folded dry-run
//! diff, and absence of a name-entry requirement.

use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "showReviewDiff" => match step["key"].as_str() {
            Some(key) => {
                resource_table::tab_to(window, "review-diff-toggle", cx);
                window
                    .within("review-diff-toggle")
                    .press(if key == "Enter" { "enter" } else { "space" }, cx);
            }
            None => window.click("review-diff-toggle", cx),
        },
        "expectDeleteReview" => {
            let label = |id: &'static str| window.find(id).label().map(str::to_owned);
            assert_eq!(
                label("delete-target-name").as_deref(),
                Some(string(step, "name"))
            );
            assert_eq!(
                label("delete-target-context").as_deref(),
                Some(string(step, "context"))
            );
            assert_eq!(
                label("delete-target-namespace").as_deref(),
                Some(string(step, "namespace"))
            );
            let controller = window
                .try_find("delete-target-controller")
                .and_then(|row| row.label().map(str::to_owned))
                .unwrap_or_default();
            assert_eq!(controller, string(step, "controller"));
            assert_eq!(
                label("delete-consequence").as_deref(),
                Some(string(step, "consequence"))
            );
            assert!(
                window.try_find("review-confirmation").is_none(),
                "delete asks for typing"
            );
        }
        _ => return false,
    }
    true
}
