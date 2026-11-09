use super::*;
use gpui_kit::ElementId;

/// Whether any element in the window carries `expected` as its label or description.
fn argo_text_present(window: &gpui_kit::Window, expected: &str) -> bool {
    window.elements().iter().any(|e| {
        e.label().is_some_and(|l| l.contains(expected))
            || e.description().is_some_and(|l| l.contains(expected))
    })
}

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
        // The parity hub owns the native executor clock, so both actions are handled there.
        "controlClock" | "advanceClock" => {}
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
                "Close connection" => {
                    // The close button sits below the form, so a short window needs a scroll to reach it.
                    reveal(window, "argo-connection-dialog", "argo-connection-close", cx);
                    "argo-connection-close"
                }
                "Disconnect" => {
                    locate::click_label(window, "root", "Disconnect", cx);
                    return true;
                }
                "Forget" => {
                    reveal(window, "detail-body", "argo-connect-open", cx);
                    window.click("argo-connect-open", cx);
                    window.render_frame(cx);
                    // Profiles are keyed by the open workspace: its saved id, or
                    // `native-session` before the startup workspace is saved.
                    let target = format!(
                        "mock-dev:kubeconfigSource=browser-mock:{}",
                        string(step, "url")
                    );
                    let id = ["1", "native-session"]
                        .map(|workspace| format!("argo-forget:argo:{workspace}:{target}"))
                        .into_iter()
                        .find(|id| window.try_find(id.clone()).is_some())
                        .unwrap_or_else(|| panic!("no Forget control for {target}"));
                    reveal(window, "detail-body", &id, cx);
                    window.click(id, cx);
                    return true;
                }
                "Retry" => "argo-discovery-retry",
                "Confirm target again" => "argo-target-retry",
                "Retry operation" => "argo-operation-retry",
                "Retry state refresh" => "argo-state-retry",
                "Retry snapshot" => "argo-snapshot-retry",
                "Technical detail" => "argo-snapshot-error:detail",
                "All changes" => "argo-all-changes",
                "Show diff" => "argo-removal::secret:argocd:argocd-secret",
                "Show fewer lines" => "argo-diff-lines:0",
                "Sync options" => "argo-sync-options",
                "Refresh options" => "argo-refresh-options",
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
                // As on the web, choose the source: the dialog keeps the last one.
                reveal(window, anchor, "argo-external", cx);
                window.click("argo-external", cx);
                reveal(window, anchor, "argo-url", cx);
                replace_text(window, cx, "argo-url", string(step, "url"));
            }
            if step["local"] == true {
                reveal(window, anchor, "argo-auth:Local login", cx);
                window.click("argo-auth:Local login", cx);
            }
            // The dialog keeps the switch between connects, so turn it on only when it is off.
            if step["remember"] == true {
                reveal(window, anchor, "argo-remember", cx);
                if window.find("argo-remember").checked() != Some(true) {
                    window.click("argo-remember", cx);
                }
            }
            if step["insecure"] == true {
                reveal(window, anchor, "argo-insecure", cx);
                if window.find("argo-insecure").checked() != Some(true) {
                    window.click("argo-insecure", cx);
                }
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
            // Home reaches Automatic whatever the current choice is, so the result does not depend on it.
            window.press("home", cx);
            if string(step, "preference") == "Kubernetes" {
                window.press("down", cx);
            }
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
            // A rejected connect lands after this step. The following expectArgoText step
            // checks the failure text, as it does for the reconnect failure.
            window.click("argo-tunnel-confirm", cx);
        }
        "expectArgoPopover" => {
            let id = if string(step, "control") == "Sync options" {
                "argo-sync-panel"
            } else {
                "argo-refresh-panel"
            };
            window.render_frame(cx);
            assert_eq!(
                window.try_find(id).is_some(),
                step["open"].as_bool() == Some(true),
                "popover {id} open state"
            );
        }
        "dismissArgoPopover" => {
            if string(step, "how") == "Escape" {
                window.press("escape", cx);
            } else {
                reveal(window, "detail-body", "argo-operation-scope", cx);
                window.click("argo-operation-scope", cx);
            }
            window.render_frame(cx);
        }
        "expectArgoCheckbox" => {
            let id = format!("argo-sync:{}", string(step, "name"));
            let expected = if step["checked"].as_bool() == Some(true) {
                "checked"
            } else {
                "unchecked"
            };
            assert_eq!(window.find(id).description(), Some(expected));
        }
        "toggleArgoOption" => {
            let id = format!("argo-sync:{}", string(step, "name"));
            reveal(window, "detail-body", &id, cx);
            window.click(id, cx);
            window.render_frame(cx);
        }
        "expectNoArgoControl" => {
            window.render_frame(cx);
            let control = string(step, "control");
            let visible = match control {
                "Reconnect" => window
                    .elements()
                    .iter()
                    .any(|e| e.label().is_some_and(|l| l == control)),
                _ => window.try_find("argo-target-retry").is_some(),
            };
            assert!(!visible, "{control} is visible");
        }
        "expectArgoWorkspaceTransport" => assert_eq!(
            window.find("argo-workspace-transport").label(),
            Some(string(step, "text"))
        ),
        "completeArgoOperation" => {}
        "expectArgoText" => {
            let expected = string(step, "text");
            // web-argo.ts opens the collapsed technical-detail disclosure that
            // holds the text. Open error disclosures one at a time until it shows.
            if !argo_text_present(window, expected) {
                let toggles: Vec<ElementId> = window
                    .elements()
                    .iter()
                    .filter(|e| {
                        matches!(e.path().last(), Some(ElementId::Name(name)) if name.ends_with(":detail"))
                    })
                    .map(locate::id)
                    .collect();
                for toggle in toggles {
                    locate::click_in(window, "root", toggle, cx);
                    window.render_frame(cx);
                    if argo_text_present(window, expected) {
                        break;
                    }
                }
            }
            assert!(
                argo_text_present(window, expected),
                "missing {expected}; unavailable labels: {:?}",
                window
                    .elements()
                    .iter()
                    .filter_map(|e| e.label())
                    .filter(|l| l.contains("unavailable"))
                    .collect::<Vec<_>>()
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
        "selectArgoConnection" => {
            reveal(window, "detail-body", "argo-inspection-preference", cx);
            window.click("argo-inspection-preference", cx);
            window.render_frame(cx);
            // Choices are Automatic, Kubernetes, then each matching profile.
            window.press("down", cx);
            window.press("down", cx);
            window.press("enter", cx);
        }
        "expectArgoFocusInPopover" => {
            let id = match string(step, "control") {
                "Sync options" => "argo-sync-panel",
                _ => "argo-refresh-panel",
            };
            window.render_frame(cx);
            // Query focus only inside the popover: focus on an unobserved element panics.
            let focused = window.elements().iter().any(|e| {
                e.path()
                    .iter()
                    .any(|part| matches!(part, ElementId::Name(name) if name == id))
                    && super::locate::focus_state(e) == Some(true)
            });
            if !focused {
                let holders: Vec<_> = window
                    .elements()
                    .iter()
                    .filter(|e| super::locate::focus_state(e) == Some(true))
                    .map(|e| format!("{:?}", e.path()))
                    .collect();
                panic!("focus is outside {id}; focused: {holders:?}");
            }
        }
        "expectArgoOptionFocusable" => {
            // A tracked focus binding makes the option a keyboard focus target.
            let id = format!("argo-sync:{}", string(step, "name"));
            assert!(
                super::locate::focus_state(&window.find(id.clone())).is_some(),
                "{id} has no focus binding"
            );
        }
        "expectArgoTriggerFocus" => {
            let id = match string(step, "control") {
                "Sync options" => "argo-sync-options",
                _ => "argo-refresh-options",
            };
            window.render_frame(cx);
            assert_eq!(
                super::locate::focus_state(&window.find(id)),
                Some(true),
                "focus is not on {id}"
            );
        }
        "fillArgoConnectionField" | "expectArgoConnectionField" => argo_fields::act(window, cx, step),
        "argoConnectionDialog" => match string(step, "operation") {
            "open" => {
                assert!(seek(window, cx, "gitops-list", "argo-connect-toolbar"));
                window.click("argo-connect-toolbar", cx);
                window.render_frame(cx);
                // The page dialog opens on its saved connections, and the form is one press away.
                if window.try_find("argo-external").is_none() {
                    reveal(window, "argo-connection-dialog", "argo-connect-open", cx);
                    window.click("argo-connect-open", cx);
                    window.render_frame(cx);
                }
            }
            "openDetail" => {
                reveal(window, "detail-body", "argo-connect-open", cx);
                window.click("argo-connect-open", cx);
                window.render_frame(cx);
            }
            source @ ("chooseFoundInCluster" | "chooseExternalUrl") => {
                let id = if source == "chooseExternalUrl" {
                    "argo-external"
                } else {
                    "argo-server:service:argocd:argocd-server:443"
                };
                reveal(window, "detail-body", id, cx);
                window.click(id, cx);
            }
            "expectFoundInCluster" => {
                reveal(window, "detail-body", "argo-external", cx);
                assert_eq!(
                    window.find("argo-external").checked(),
                    step["checked"].as_bool().map(|found| !found),
                    "External URL choice"
                );
            }
            "expectFocusIn" => {
                window.render_frame(cx);
                // Query focus only inside the dialog: focus on an unobserved element panics.
                let focused = window.elements().iter().any(|e| {
                    e.path()
                        .iter()
                        .any(|part| matches!(part, ElementId::Name(name) if name == "argo-connection-dialog"))
                        && super::locate::focus_state(e) == Some(true)
                });
                assert!(focused, "focus is outside the Argo CD connection dialog");
            }
            "tabCycle" | "shiftTabCycle" => {
                let key = if string(step, "operation") == "tabCycle" {
                    "tab"
                } else {
                    "shift-tab"
                };
                let inside = |path: &[ElementId]| {
                    path.iter().any(
                        |part| matches!(part, ElementId::Name(name) if name == "argo-connection-dialog"),
                    )
                };
                let focused = |window: &gpui_kit::Window| {
                    window
                        .elements()
                        .iter()
                        .find(|e| inside(e.path()) && super::locate::focus_state(e) == Some(true))
                        .map(|e| e.path().to_vec())
                };
                window.render_frame(cx);
                // The dialog's element count bounds the cycle.
                let limit = window.elements().iter().filter(|e| inside(e.path())).count();
                let first = focused(window).expect("focus starts inside the Argo CD connection dialog");
                let mut stops = 0;
                loop {
                    stops += 1;
                    assert!(
                        stops <= limit,
                        "focus did not return to its first control within {limit} presses"
                    );
                    window.press(key, cx);
                    window.render_frame(cx);
                    let now = focused(window).unwrap_or_else(|| {
                        panic!("focus left the Argo CD connection dialog after {stops} presses")
                    });
                    if now == first {
                        break;
                    }
                }
            }
            "escape" => {
                window.press("escape", cx);
                window.render_frame(cx);
            }
            "outsideClick" => {
                window.click("argo-connect-toolbar", cx);
                window.render_frame(cx);
            }
            "expectConnectForm" => {
                window.render_frame(cx);
                assert!(
                    window.try_find("argo-connect-submit").is_some(),
                    "the Argo CD connection form is not shown"
                );
            }
            "expectOpen" => {
                window.render_frame(cx);
                assert_eq!(
                    window.try_find("argo-connection-dialog").is_some(),
                    step["open"].as_bool() == Some(true),
                    "Argo CD connection dialog open state"
                );
            }
            "expectInWindow" => {
                window.render_frame(cx);
                let dialog = window.find("argo-connection-dialog").bounds();
                let viewport = window.viewport_size();
                assert!(
                    dialog.left() >= gpui_kit::px(0.)
                        && dialog.top() >= gpui_kit::px(0.)
                        && dialog.right() <= viewport.width
                        && dialog.bottom() <= viewport.height,
                    "the Argo CD connection dialog {dialog:?} overflows the {viewport:?} window"
                );
            }
            "expectReachable" => {
                let id = match string(step, "control") {
                    "External URL" => "argo-external",
                    "Token" => "argo-token",
                    "Remember credential" => "argo-remember",
                    "Connect" => "argo-connect-submit",
                    control => panic!("unknown Argo connection dialog control {control}"),
                };
                reveal(window, "argo-connection-dialog", id, cx);
                window.render_frame(cx);
                let dialog = window.find("argo-connection-dialog").bounds();
                let control = window.find(id).bounds();
                assert!(
                    control.left() >= dialog.left()
                        && control.right() <= dialog.right()
                        && control.top() >= dialog.top()
                        && control.bottom() <= dialog.bottom(),
                    "{id} is not reachable inside the Argo CD connection dialog"
                );
            }
            operation @ ("expectRemember" | "expectLocalLogin" | "expectInsecure"
            | "chooseLocalLogin" | "chooseInsecure") => {
                let id = match operation {
                    "expectRemember" => "argo-remember",
                    "expectLocalLogin" | "chooseLocalLogin" => "argo-auth:Local login",
                    _ => "argo-insecure",
                };
                window.render_frame(cx);
                // The invalid-certificate switch belongs to the external URL, as on the web.
                if id == "argo-insecure" {
                    reveal(window, "detail-body", "argo-external", cx);
                    window.click("argo-external", cx);
                }
                reveal(window, "detail-body", id, cx);
                if operation.starts_with("choose") {
                    if window.find(id).checked() != Some(true) {
                        window.click(id, cx);
                    }
                    return true;
                }
                assert_eq!(
                    window.find(id).checked(),
                    step["checked"].as_bool(),
                    "{id} choice"
                );
            }
            operation => panic!("unknown Argo connection dialog operation {operation}"),
        },
        "tabArgoPopover" => {
            let presses = step["presses"].as_u64().expect("tab presses");
            for _ in 0..presses {
                window.press("tab", cx);
            }
            window.render_frame(cx);
        }
        "expectArgoOperationState" => {
            let id = match string(step, "control") {
                "Sync options" => "argo-sync-options",
                "Refresh options" => "argo-refresh-options",
                "Sync" => "argo-operation:Sync",
                _ => "argo-operation:Refresh",
            };
            let expected = if step["enabled"].as_bool() == Some(true) {
                "enabled"
            } else {
                "disabled"
            };
            window.render_frame(cx);
            assert_eq!(window.find(id).description(), Some(expected), "{id}");
        }
        _ => return false,
    }
    true
}
