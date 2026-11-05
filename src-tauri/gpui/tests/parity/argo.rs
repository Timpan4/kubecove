use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "connectArgoProfile" => {
            let anchor = if step["surface"] == "page" {
                "argo-connection-dialog"
            } else {
                "detail-body"
            };
            if step["surface"] == "page" {
                assert!(seek(window, cx, "gitops-list", "argo-connect-toolbar"));
                window.click("argo-connect-toolbar", cx);
                window.render_frame(cx);
            }
            reveal(window, anchor, "argo-connect-open", cx);
            window.click("argo-connect-open", cx);
            window.render_frame(cx);
            reveal(window, anchor, "argo-url", cx);
            replace_text(window, cx, "argo-url", string(step, "url"));
        }
        "disconnectArgoProfile" => {
            let id =
                "argo-profile:argo:native-session:mock-dev:external:https://argocd.example.test";
            reveal(window, "detail-body", id, cx);
            window.click(id, cx);
        }
        "reviewArgoOperation" => {
            let id = format!("argo-operation:{}", string(step, "operation"));
            reveal(window, "detail-body", &id, cx);
            window.click(id, cx);
        }
        "expectArgoReview" => {
            reveal(window, "detail-body", "argo-review", cx);
            assert_eq!(
                window.find("argo-review").label(),
                Some(
                    format!(
                        "{} {} mock-dev Application {} {}",
                        string(step, "operation"),
                        string(step, "transport"),
                        string(step, "namespace"),
                        string(step, "name")
                    )
                    .as_str()
                )
            );
        }
        "confirmArgoReview" => {
            reveal(window, "detail-body", "argo-review-confirm", cx);
            window.click("argo-review-confirm", cx);
        }
        "cancelArgoReview" => {
            reveal(window, "detail-body", "argo-review-cancel", cx);
            window.click("argo-review-cancel", cx);
        }
        "expectArgoText" => {
            let expected = string(step, "text");
            assert!(
                window
                    .elements()
                    .iter()
                    .any(|e| e.label().is_some_and(|l| l.contains(expected))),
                "missing {expected}"
            );
        }
        "argoDocument" => {
            let id = format!("argo-view:{}", string(step, "view"));
            reveal(window, "detail-body", &id, cx);
            window.click(id, cx);
            window.render_frame(cx);
            let expected = string(step, "contains");
            assert!(
                window
                    .elements()
                    .iter()
                    .any(|e| e.label().is_some_and(|l| l.contains(expected))),
                "missing document {expected}"
            );
        }
        _ => return false,
    }
    true
}
