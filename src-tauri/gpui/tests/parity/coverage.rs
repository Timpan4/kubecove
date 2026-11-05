use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "coverageSelect" => {
            assert_eq!(string(step, "label"), "Log container");
            reveal(window, "detail-body", "log-container", cx);
            window.click("log-container", cx);
            window.press("down", cx);
        }
        "coverageCopy" => {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(String::new()));
            let label = string(step, "label");
            if label == "Copy evidence" {
                reveal(window, RBAC_TOP, "rbac-copy", cx);
                window.click("rbac-copy", cx);
            } else {
                let target = window.find_by_label(label);
                window.click(locate::id(&target), cx);
            }
            let copied = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default();
            match string(step, "match") {
                "exact" => assert_eq!(copied, string(step, "text")),
                "contains" => assert!(copied.contains(string(step, "text"))),
                other => panic!("unknown clipboard match {other}"),
            }
        }
        "coverageScript" => {}
        "coverageClick" => {
            if string(step, "label") == "Open events" {
                let target = locate::id(&window.find_by_label("Open events"));
                locate::click_in(window, "incident-guide", target, cx);
                return true;
            }
            let (id, anchor) = match string(step, "label") {
                "Resources" => ("open-resources", "open-resources"),
                "Expand all" => ("timeline-expand-all", "detail-body"),
                "Collapse all" => ("timeline-collapse-all", "detail-body"),
                "Pause" | "Resume" => ("log-pause", "detail-body"),
                "Clear" => ("log-clear", "detail-body"),
                "Latest first" | "Oldest first" => ("log-order", "detail-body"),
                "Wrap lines" => ("log-wrap", "detail-body"),
                "Log tail and time window" => ("log-tail", "detail-body"),
                "5m" => ("log-since-5m", "detail-body"),
                "Any" => ("log-since-Any", "detail-body"),
                "Allow force-conflicts for this resource" => ("yaml-allow-force", "detail-body"),
                "Clear filters" => ("helm-clear-filters", "helm-list"),
                "Clear Helm search" => ("helm-clear", "helm-list"),
                "Show context" | "Hide context" => ("incident-step-context", "incident-guide"),
                "Clear terminal" => ("exec-clear", "detail-body"),
                "Stop session" => ("exec-stop", "detail-body"),
                "Restart session" => ("exec-restart", "detail-body"),
                "Discard" => ("action-discard", "actions-scroll"),
                "Done" => ("action-replicas", "actions-scroll"),
                "Revert" => ("action-revert-replicas", "actions-scroll"),
                "Show full diff" | "Collapse diff" => ("yaml-diff-toggle", "detail-body"),
                "Investigate identity" => ("rbac-investigate", RBAC_TOP),
                "User" => ("rbac-identity-user", RBAC_TOP),
                "Group" => ("rbac-identity-group", RBAC_TOP),
                "ServiceAccount" => ("rbac-identity-sa", RBAC_TOP),
                "Resource" => ("rbac-target-resource", RBAC_TOP),
                "Non-resource" => ("rbac-target-non-resource", RBAC_TOP),
                "Verify access" => ("rbac-verify", RBAC_TOP),
                "Load 50 more" => ("rbac-more", RBAC_TOP),
                "Open in Resources" => ("incident-open-resource", "incident-guide"),
                other => panic!("unmapped coverage control {other}"),
            };
            reveal(window, anchor, id, cx);
            assert!(
                window.find(id).visible(),
                "control {id} remains offscreen: {:?}",
                window.find(id).bounds()
            );
            window.click(id, cx);
        }
        "coverageFill" => {
            let (id, anchor) = match string(step, "label") {
                "Resource action filter" => ("action-filter", "actions-scroll"),
                "Tail log lines" => ("log-tail-lines", "detail-body"),
                "Desired replicas" => ("action-replicas-input", "actions-scroll"),
                "Search RBAC objects" => ("rbac-search", RBAC_TOP),
                "Identity name" => ("rbac-identity-name", RBAC_TOP),
                "Groups" => ("rbac-identity-groups", RBAC_TOP),
                "ServiceAccount namespace" => ("rbac-identity-namespace", RBAC_TOP),
                "Verb" => ("rbac-verb", RBAC_TOP),
                "Resource" => ("rbac-resource", RBAC_TOP),
                "API group, blank for core" => ("rbac-api-group", RBAC_TOP),
                "Namespace, blank for cluster" => ("rbac-namespace", RBAC_TOP),
                "Subresource, optional" => ("rbac-subresource", RBAC_TOP),
                "Resource name, optional" => ("rbac-resource-name", RBAC_TOP),
                "Non-resource URL" => ("rbac-url", RBAC_TOP),
                other => panic!("unmapped coverage input {other}"),
            };
            reveal(window, anchor, id, cx);
            replace_text(window, cx, id, string(step, "value"));
        }
        "coverageText" => {
            let text = string(step, "text");
            if text == "Helm details unavailable" {
                assert!(seek(window, cx, "helm-list", "surface-error"));
            }
            if text == "Loading Helm release details..." {
                assert!(seek(
                    window,
                    cx,
                    "helm-list",
                    &format!("helm-pending:{text}")
                ));
            }
            assert_eq!(
                window
                    .elements()
                    .iter()
                    .any(|element| element.label() == Some(text)
                        || (matches!(text, "Log stream failed" | "Port-forward failed")
                            && element.label().is_some_and(|label| label
                                .strip_prefix(text)
                                .is_some_and(|tail| tail.starts_with(": "))))
                        || element
                            .description()
                            .is_some_and(|description| description.contains(text))),
                step["visible"] == true,
                "{step}"
            );
        }
        "paletteKey" => window.press(
            match string(step, "key") {
                "ArrowDown" => "down",
                "ArrowUp" => "up",
                "Enter" => "enter",
                "Escape" => "escape",
                other => panic!("unknown palette key {other}"),
            },
            cx,
        ),
        "expectPaletteOpen" => assert_eq!(
            window.try_find("palette-query").is_some(),
            step["open"] == true
        ),
        "deleteSavedWorkspace" => {
            let label = format!("Delete {}", string(step, "name"));
            locate::click_label(window, "launcher-body", &label, cx);
        }
        "importDecision" => {
            window.click(format!("workspace-import:0:{}", string(step, "choice")), cx)
        }
        "cancelImport" => window.click("workspace-import-cancel", cx),
        "searchEvents" => replace_text(window, cx, "events-search", string(step, "value")),
        _ => return false,
    }
    true
}

pub(super) fn finish(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    assert_eq!(window.find("log-container").expanded(), Some(true));
    let target = string(step, "value");
    if window.find("log-container").value() != Some(target) {
        window.press(
            match target {
                "app" => "up",
                "sidecar" => "down",
                other => panic!("unknown mock log container {other}"),
            },
            cx,
        );
    }
    window.press("enter", cx);
}
