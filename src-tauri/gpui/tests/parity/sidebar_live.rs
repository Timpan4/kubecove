use super::*;

pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "refreshPage" => window.click(
            if step["page"] == "RBAC" {
                "rbac-refresh"
            } else {
                "incidents-refresh"
            },
            cx,
        ),
        "replayPageReads" | "focusStalePage" => {}
        "expectPageText" => {
            if string(step, "text") == "Scripted page refresh failure" {
                assert_eq!(
                    window.find("surface-error").description(),
                    Some(string(step, "text"))
                );
            } else {
                window.find_by_label(string(step, "text"));
            }
        }
        "searchCustomKinds" => {
            if window.try_find("custom-resource-search").is_none() {
                reveal(window, "sidebar-tree", "nav:Custom Resources", cx);
                window.click("nav:Custom Resources", cx);
                window.render_frame(cx);
            }
            replace_text(window, cx, "custom-resource-search", string(step, "value"));
        }
        "searchKindPicker" => replace_text(window, cx, "kind-search", string(step, "value")),
        "expectCustomGroup" => assert_eq!(
            window
                .try_find(format!("custom-group:{}", string(step, "group")))
                .is_some(),
            step["visible"] == true
        ),
        "openKindPicker" | "closeKindPicker" => window.click("kinds-picker", cx),
        "selectAllKinds" => window.click("select-all-kinds", cx),
        "toggleKind" | "expectKindChecked" => {
            let option = locate::described(
                window,
                "kind-options",
                string(step, "kind"),
                if step["checked"] == true {
                    "checked"
                } else {
                    "unchecked"
                },
            );
            if step["action"] == "expectKindChecked" {
                assert!(option.is_some(), "wrong kind selection: {step}");
            } else if option.is_none() {
                let found = window.find_by_label(string(step, "kind"));
                window.click(locate::id(&found), cx);
            }
        }
        _ => return false,
    }
    true
}
