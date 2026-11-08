use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "failWorkspaceContext" => {}
        "expectWorkspaceReadWarning" => assert!(
            window
                .try_find_by_label(&format!("{}: Context unavailable", string(step, "context")))
                .is_some()
        ),
        "editSavedWorkspace" => locate::click_label(
            window,
            "launcher-body",
            &format!("Edit {}", string(step, "name")),
            cx,
        ),
        "expectNoSavedWorkspace" => assert!(
            window
                .within("launcher")
                .try_find_by_label(&format!("Open {}", string(step, "name")))
                .is_none()
        ),
        "exportSavedWorkspace" => locate::click_label(
            window,
            "launcher-body",
            &format!("Export {}", string(step, "name")),
            cx,
        ),
        "saveWorkspace" => window.click("workspace-create", cx),
        "cancelWorkspaceEdit" => window.click("workspace-cancel-edit", cx),
        "toggleWorkspaceContext" => locate::click_in(
            window,
            "workspace-groups",
            format!("workspace-group:{}", string(step, "context")).into(),
            cx,
        ),
        "expectOverviewHealth" => assert_eq!(
            window.find("overview-metric:Total").label(),
            Some(format!("Total: {}", number(step, "total")).as_str())
        ),
        "expectDetailContext" => assert_eq!(
            window.find("detail-context").label(),
            Some(string(step, "context"))
        ),
        _ => return false,
    }
    true
}
