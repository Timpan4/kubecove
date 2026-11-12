use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "failWorkspaceContext" | "holdWorkspaceContext" | "releaseWorkspaceContext" => {}
        "expectContextsLoading" => {
            let contexts = strings(step, "contexts");
            let notice = window.try_find("contexts-waiting");
            if contexts.is_empty() {
                assert!(notice.is_none(), "contexts still shown as loading");
            } else {
                let names = contexts
                    .iter()
                    .map(|context| format!("\"{context}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                let plural = if contexts.len() > 1 { "s" } else { "" };
                let expected = format!(
                    "Waiting for context{plural} {names}. Showing resources from the contexts that answered."
                );
                let notice = notice.expect("no loading contexts notice");
                assert_eq!(notice.label(), Some(expected.as_str()));
                assert!(notice.visible(), "loading contexts notice is hidden");
            }
        }
        // The Overview labels its warning "<context>: <message>"; the Resources
        // page names the context in quotes, as `Context "<context>": <message>`.
        "expectWorkspaceReadWarning" => {
            let context = string(step, "context");
            let overview = format!("{context}: Context unavailable");
            let resources = format!("Context \"{context}\": Context unavailable");
            assert!(
                window.try_find_by_label(&overview).is_some()
                    || window.try_find_by_label(&resources).is_some(),
                "no read warning for {context}"
            );
        }
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
        // A multi-context workspace shows the row's context as a header badge.
        // A one-context workspace has no badge; its detail belongs to the
        // workspace context the sidebar names.
        "expectDetailContext" => {
            let context = string(step, "context");
            assert!(
                window.try_find("detail-identity").is_some(),
                "no open detail"
            );
            match window.try_find("detail-context") {
                Some(badge) => {
                    assert_eq!(badge.label(), Some(format!("Context: {context}").as_str()));
                    assert!(badge.visible(), "context badge is hidden");
                }
                None => assert_eq!(window.find("scope-context").label(), Some(context)),
            }
        }
        _ => return false,
    }
    true
}
