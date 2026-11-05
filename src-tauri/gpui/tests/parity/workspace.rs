use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openLauncher" => window.click("header-workspaces", cx),
        "expectSavedWorkspace" => assert_eq!(
            window
                .within("launcher")
                .find_by_label(string(step, "name"))
                .description(),
            Some(string(step, "summary"))
        ),
        "openSavedWorkspace" => {
            let label = format!("Open {}", string(step, "name"));
            locate::click_label(window, "launcher-body", &label, cx);
        }
        "nameWorkspace" => replace_text(window, cx, "workspace-name", string(step, "value")),
        "selectWorkspaceContext" => {
            window.click("workspace-context", cx);
            window.render_frame(cx);
            window.click(format!("workspace-context:{}", string(step, "context")), cx);
        }
        "toggleWorkspaceNamespace" => {
            let id = format!("workspace-namespace:{}", string(step, "namespace"));
            locate::click_in(window, "workspace-namespaces", id.into(), cx);
        }
        "createWorkspace" => window.click("workspace-create", cx),
        "expectOpenWorkspace" => {
            let title = window.find("workspace-title");
            assert_eq!(title.label(), Some(string(step, "name")));
            let context = window.find("scope-context");
            assert_eq!(context.label(), Some(string(step, "context")));
        }
        "selectRbacCategory" => locate::click_in(
            window,
            "page",
            format!("rbac-category:{}", string(step, "category")).into(),
            cx,
        ),
        "expectRbacCategory" => assert_eq!(
            window
                .find(format!("rbac-category:{}", string(step, "category")))
                .selected(),
            Some(true)
        ),
        "filterRbacRisk" => {
            window.click(format!("rbac-risk-filter:{}", string(step, "filter")), cx)
        }
        "expectRbacItem" => assert_eq!(
            window
                .within("rbac-queue")
                .find_by_label(string(step, "title"))
                .description(),
            Some(string(step, "risk"))
        ),
        "expectNoRbacItem" => assert!(
            window
                .within("rbac-queue")
                .try_find_by_label(string(step, "title"))
                .is_none()
        ),
        "selectRbacItem" => {
            let target = locate::id(
                &window
                    .within("rbac-queue")
                    .find_by_label(string(step, "title")),
            );
            locate::click_in(window, "page", target, cx);
        }
        "editRbacReviewNote" => {
            locate::click_in(window, "page", "rbac-review-editor".into(), cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
        }
        "expectRbacReviewNote" => assert_eq!(
            window.find_by_label("RBAC review note").value(),
            Some(string(step, "value"))
        ),
        "saveRbacReview" => locate::click_in(
            window,
            "page",
            format!("rbac-mark-{}", string(step, "disposition")).into(),
            cx,
        ),
        "expectRbacReview" => assert_eq!(
            window.find("rbac-review-status").label(),
            Some(string(step, "label"))
        ),
        "clearRbacReview" => locate::click_in(window, "page", "rbac-clear-review".into(), cx),
        "investigateRbacSubject" => {
            window.click(format!("rbac-subject:{}", string(step, "subject")), cx)
        }
        "expectRbacIdentity" => assert_eq!(
            window.find("rbac-identity").label(),
            Some(string(step, "identity"))
        ),
        "expectRbacGrant" => assert!(
            !window
                .within("rbac-grants")
                .find_all_by_label(string(step, "grant"))
                .is_empty()
        ),
        "expectNoRbacGrant" => assert!(
            window
                .within("rbac-grants")
                .find_all_by_label(string(step, "grant"))
                .is_empty()
        ),
        "verifyRbacAccess" => {
            // The verifier sits below the fold of the page's scroll area. The
            // click follows in `finish`, after the edit's change events apply.
            reveal(window, RBAC_TOP, "rbac-verb", cx);
            replace_text(window, cx, "rbac-verb", string(step, "verb"));
        }
        "expectRbacVerdict" => assert_eq!(
            window.find("rbac-verdict").label(),
            Some(string(step, "outcome"))
        ),
        "openSettings" => window.click("open-settings", cx),
        "closeSettings" => window.click("settings-back", cx),
        "showSettingsSection" => {
            window.click(format!("settings-section:{}", string(step, "section")), cx)
        }
        "searchSettings" => replace_text(window, cx, "settings-search", string(step, "value")),
        "expectSettingsHeading" => assert_eq!(
            window.find("settings-heading").label(),
            Some(string(step, "heading"))
        ),
        "expectSettingRow" => assert!(
            window
                .try_find(format!("setting-row:{}", string(step, "title")))
                .is_some()
        ),
        "expectNoSettingRow" => assert!(
            window
                .try_find(format!("setting-row:{}", string(step, "title")))
                .is_none()
        ),
        "expectSettingsEmpty" => assert_eq!(
            window.find("settings-empty").label(),
            Some(format!("No settings match \"{}\".", string(step, "value")).as_str())
        ),
        "setKubeconfigEnvVar" => {
            replace_text(window, cx, "kubeconfig-env-var", string(step, "value"));
            window.click("kubeconfig-env-save", cx);
        }
        "expectKubeconfigEnvVar" => assert_eq!(
            window.find("kubeconfig-env-var").value(),
            Some(string(step, "value"))
        ),
        "toggleSetting" => window.click(format!("setting:{}", string(step, "setting")), cx),
        "expectSetting" => assert_eq!(
            window
                .find(format!("setting:{}", string(step, "setting")))
                .checked(),
            Some(step["checked"].as_bool().expect("missing checked"))
        ),
        "openPalette" => {
            match string(step, "via") {
                // Cmd on macOS, Ctrl elsewhere, like the web's `ControlOrMeta+K`.
                "shortcut" => window.press("secondary-k", cx),
                "button" => window.click("open-palette", cx),
                other => panic!("unknown palette opener: {other}"),
            }
            window.render_frame(cx);
            assert!(window.find("palette-dialog").visible());
        }
        "searchPalette" => replace_text(window, cx, "palette-query", string(step, "value")),
        "expectPaletteResource" => assert!(palette_resource(window, step).is_some()),
        "expectNoPaletteResource" => assert!(palette_resource(window, step).is_none()),
        "choosePaletteResource" => {
            let target = locate::id(&palette_resource(window, step).expect("palette resource"));
            locate::click_in(window, "palette-list", target, cx);
        }
        "expectPaletteEntry" => assert!(
            window
                .within("palette-list")
                .try_find_by_label(string(step, "label"))
                .is_some()
        ),
        "expectNoPaletteEntry" => assert!(
            window
                .within("palette-list")
                .try_find_by_label(string(step, "label"))
                .is_none()
        ),
        "choosePaletteEntry" => {
            locate::click_label(window, "palette-list", string(step, "label"), cx)
        }
        _ => return false,
    }
    true
}
