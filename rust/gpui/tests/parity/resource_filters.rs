use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openNamespacePicker" | "closeNamespacePicker" => window.click("namespace-picker", cx),
        "searchNamespacePicker" => {
            replace_text(window, cx, "namespace-search", string(step, "value"))
        }
        "selectAllNamespaces" => window.click("select-all-namespaces", cx),
        "toggleNamespace" | "expectNamespaceChecked" => {
            let namespace = string(step, "namespace");
            let desired = if step["checked"] == true {
                "checked"
            } else {
                "unchecked"
            };
            let option = locate::described(window, "namespace-options", namespace, desired);
            if string(step, "action") == "expectNamespaceChecked" {
                assert!(option.is_some(), "wrong namespace selection: {step}");
            } else if option.is_none() {
                locate::click_label(window, "namespace-options", namespace, cx);
            }
        }
        "expectNamespaceTrigger" => {
            window
                .within("namespace-picker")
                .find_by_label(string(step, "label"));
        }
        "selectHealthFilter" => window.click(format!("health:{}", string(step, "label")), cx),
        "expectHealthFilter" => {
            let element = window.find(format!("health:{}", string(step, "label")));
            let pressed = if step["pressed"] == true {
                "pressed"
            } else {
                "unpressed"
            };
            assert_eq!(element.description(), Some(pressed), "{step}");
        }
        "chooseGitOpsOwner" => {
            window.click("gitops-owner", cx);
            locate::click_label(window, "gitops-owner-options", string(step, "owner"), cx);
        }
        "clearResourceFilters" => window.click("clear-resource-filters", cx),
        "setHistoricalReplicaSets" => {
            let desired = if step["checked"] == true {
                "checked"
            } else {
                "unchecked"
            };
            if window.find("hide-historical-replicasets").description() != Some(desired) {
                window.click("hide-historical-replicasets", cx);
            }
            assert_eq!(
                window.find("hide-historical-replicasets").description(),
                Some(desired),
                "{step}"
            );
        }
        _ => return false,
    }
    true
}
