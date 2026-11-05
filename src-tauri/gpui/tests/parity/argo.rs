use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "reconnectArgoProfile" => {
            reveal(window, "detail-body", "argo-connect-open", cx);
            window.click("argo-connect-open", cx);
            window.render_frame(cx);
            locate::click_label(window, "root", "Reconnect", cx);
        }
        "expandArgoDiff" => {
            reveal(window, "detail-body", "argo-diff-lines:0", cx);
            window.click("argo-diff-lines:0", cx);
        }
        "configureArgoInventory" => {
            assert!(seek(window, cx, "gitops-list", "live-status"));
            window.click("live-status", cx);
            window.render_frame(cx);
            window.click("refresh", cx);
        }
        "copyArgoValue" => {
            use gpui_kit::test::TestEventExt;
            use gpui_kit::{MouseButton, point, px};
            let control = string(step, "control");
            let position = match control {
                "source" | "revision" => {
                    let id = "gitops-card:argoApp:mock-dev:argocd:platform-cilium";
                    assert!(seek(window, cx, "gitops-list", id));
                    let bounds = window.find(id).bounds();
                    if control == "source" {
                        point(bounds.right() - px(26.), bounds.top() + px(26.))
                    } else {
                        point(bounds.left() + px(40.), bounds.top() + px(164.))
                    }
                }
                _ => {
                    let id = match control {
                        "repository value" => "argo-route-1-value",
                        "release value" => "argo-route-2-value",
                        "cluster target value" => "argo-route-3-value",
                        _ => unreachable!(),
                    };
                    let bounds = window.find(id).bounds();
                    point(bounds.right() + px(14.), bounds.center().y)
                }
            };
            window.pointer_move(position, None, cx);
            window.pointer_down(position, MouseButton::Left, cx);
            window.pointer_up(position, MouseButton::Left, cx);
            assert!(
                cx.read_from_clipboard()
                    .and_then(|c| c.text())
                    .is_some_and(|c| c.contains(string(step, "contains"))),
                "wrong clipboard for {control}"
            );
        }
        "configureArgoMock" => {}
        "focusArgoResource" => {
            use gpui_kit::test::TestEventExt;
            use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};
            let id = format!("argo-change:{}", string(step, "key"));
            reveal(window, "detail-body", "argo-navigator", cx);
            let navigator = window.find("argo-navigator").bounds();
            let target = window.find(id.clone()).bounds();
            let position = navigator.center();
            window.pointer_move(position, None, cx);
            window.dispatch_event(
                ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(
                        navigator.center().x - target.center().x,
                        px(0.),
                    )),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            assert_eq!(
                window.find("argo-navigator").bounds().top(),
                navigator.top(),
                "horizontal navigation must not scroll the detail vertically"
            );
            window.click(id, cx);
        }
        "expectNoSavedArgoProfile" => {
            reveal(window, "detail-body", "argo-connect-open", cx);
            window.click("argo-connect-open", cx);
            window.render_frame(cx);
            assert!(
                window
                    .try_find(format!("argo-saved:{}", string(step, "url")))
                    .is_none()
            );
            window.click("argo-connection-close", cx);
        }
        "argoControl" => {
            let id = match string(step, "control") {
                "Open connection" => "argo-connect-open",
                "Close connection" => "argo-connection-close",
                "Forget" => {
                    reveal(window, "detail-body", "argo-connect-open", cx);
                    window.click("argo-connect-open", cx);
                    window.render_frame(cx);
                    let id = format!(
                        "argo-forget:argo:native-session:mock-dev:kubeconfigSource=browser-mock:{}",
                        string(step, "url")
                    );
                    reveal(window, "detail-body", &id, cx);
                    window.click(id, cx);
                    return true;
                }
                "Retry" => "argo-discovery-retry",
                "Confirm target again" => "argo-target-retry",
                "Retry operation" => "argo-operation-retry",
                "Retry state refresh" => "argo-state-retry",
                "Retry snapshot" => "argo-snapshot-retry",
                "All changes" => "argo-all-changes",
                "Show diff" => "argo-removal::secret:argocd:argocd-secret",
                "Show fewer lines" => "argo-diff-lines:0",
                "Sync options" => "argo-sync-options",
                "Sync with these options" => "argo-sync-advanced",
                other => panic!("unknown Argo control {other}"),
            };
            reveal(window, "detail-body", id, cx);
            window.click(id, cx);
        }
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
            if string(step, "operation") == "Hard refresh" {
                reveal(window, "detail-body", "argo-refresh-options", cx);
                window.click("argo-refresh-options", cx);
                window.render_frame(cx);
            }
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
            reveal(window, "detail-body", "argo-sync-advanced", cx);
            window.click("argo-sync-advanced", cx);
            window.render_frame(cx);
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
                    .any(|e| e.label().is_some_and(|l| l.contains(expected))
                        || e.description().is_some_and(|l| l.contains(expected))),
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
