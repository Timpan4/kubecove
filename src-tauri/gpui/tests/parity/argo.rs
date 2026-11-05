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
            if step["tunnel"] != true {
                reveal(window, anchor, "argo-url", cx);
                replace_text(window, cx, "argo-url", string(step, "url"));
            }
            if step["local"] == true {
                reveal(window, anchor, "argo-auth:Local login", cx);
                window.click("argo-auth:Local login", cx);
            }
            if step["remember"] == true {
                reveal(window, anchor, "argo-remember", cx);
                window.click("argo-remember", cx);
            }
            if step["insecure"] == true {
                reveal(window, anchor, "argo-insecure", cx);
                window.click("argo-insecure", cx);
            }
        }
        "disconnectArgoProfile" => {
            reveal(window, "detail-body", "argo-connect-open", cx);
            window.click("argo-connect-open", cx);
            window.render_frame(cx);
            locate::click_label(window, "root", "Disconnect", cx);
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
        "selectArgoPreference" => {
            reveal(window, "detail-body", "argo-inspection-preference", cx);
            window.click("argo-inspection-preference", cx);
            window.render_frame(cx);
            window.press("up", cx);
        }
        "expectSavedArgoProfile" => {
            reveal(window, "detail-body", "argo-connect-open", cx);
            window.click("argo-connect-open", cx);
            window.render_frame(cx);
            let id = format!("argo-saved:{}", string(step, "url"));
            reveal(window, "detail-body", &id, cx);
            assert!(
                window
                    .find(id.clone())
                    .label()
                    .unwrap()
                    .contains("reconnects automatically")
            );
        }
        "configureArgoSync" => {
            reveal(window, "detail-body", "argo-sync-options", cx);
            window.click("argo-sync-options", cx);
            window.render_frame(cx);
            reveal(window, "detail-body", "argo-sync-revision", cx);
            replace_text(window, cx, "argo-sync-revision", string(step, "revision"));
            for id in [
                "argo-sync:Prune",
                "argo-sync:Dry run",
                "argo-sync:Force / replace",
            ] {
                reveal(window, "detail-body", id, cx);
                window.click(id, cx);
            }
            reveal(window, "detail-body", "argo-sync-name", cx);
            replace_text(window, cx, "argo-sync-name", string(step, "name"));
        }
        "selectArgoHistory" => {
            reveal(window, "detail-body", "argo-history-picker", cx);
            window.click("argo-history-picker", cx);
            window.render_frame(cx);
            window.press("home", cx);
            if string(step, "revision") == "3d91a7b" {
                window.press("down", cx);
            }
            window.press("enter", cx);
        }
        "confirmArgoTunnel" => {
            reveal(window, "detail-body", "argo-tunnel-target", cx);
            assert_eq!(
                window.find("argo-tunnel-target").label(),
                Some("argocd argocd-server argocd-server-7d9f8b6c5-x2k4q")
            );
            reveal(window, "detail-body", "argo-tunnel-confirm", cx);
            window.click("argo-tunnel-confirm", cx);
        }
        "expectArgoWorkspaceTransport" => assert_eq!(
            window.find("argo-workspace-transport").label(),
            Some(string(step, "text"))
        ),
        "completeArgoOperation" => {}
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
