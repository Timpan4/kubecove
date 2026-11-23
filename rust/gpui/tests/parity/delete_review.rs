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

#[cfg(test)]
mod tests {
    use crate::common::DetailTabs;
    use kubecove_lib::models::{AppErrorKind, ResourceActionConfirmation, ResourceActionRequest};
    use serde_json::json;

    #[tokio::test]
    async fn repeated_preview_issues_a_fresh_id_and_rejects_the_superseded_one() {
        let request: ResourceActionRequest = serde_json::from_value(json!({
            "clusterContext": "kind-dev",
            "kind": "ConfigMap",
            "name": "settings",
            "namespace": "default",
            "apiVersion": "v1",
            "action": "delete"
        }))
        .unwrap();
        let target = json!({
            "clusterContext": "kind-dev",
            "kind": "ConfigMap",
            "name": "settings",
            "namespace": "default",
            "apiVersion": "v1"
        });
        let fixture: DetailTabs = serde_json::from_value(json!({
            "logs": { "pod": [], "aggregate": [] },
            "yamlReads": [],
            "revisions": [],
            "actions": {
                "states": [],
                "pausedState": null,
                "previews": [{
                    "request": request,
                    "preview": {
                        "reviewId": "fixture-review",
                        "target": target,
                        "effect": "Delete ConfigMap",
                        "beforeYaml": "",
                        "afterYaml": "",
                        "uid": "uid-1",
                        "resourceVersion": "7"
                    }
                }],
                "executions": [{
                    "confirmation": { "reviewId": "fixture-review" },
                    "action": "delete",
                    "result": { "effect": "Deleted ConfigMap", "target": target }
                }]
            },
            "yamlApply": []
        }))
        .unwrap();

        let first = fixture.preview_action(request.clone()).await.unwrap();
        let second = fixture.preview_action(request).await.unwrap();
        assert_ne!(first.review_id, second.review_id);
        assert_eq!(
            fixture
                .execute_action(ResourceActionConfirmation {
                    review_id: first.review_id,
                })
                .await
                .unwrap_err()
                .kind,
            AppErrorKind::ConfirmationRequired
        );
        assert!(
            fixture
                .execute_action(ResourceActionConfirmation {
                    review_id: second.review_id,
                })
                .await
                .is_ok()
        );
    }
}
