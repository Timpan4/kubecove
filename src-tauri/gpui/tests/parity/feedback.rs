//! Shared failure and copy feedback: the foreground loading bar, the friendly
//! error panel and its permission handoff, and the copy toast. The clipboard
//! is GPUI's in-memory test clipboard, never the system clipboard.
use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectForegroundLoading" => {
            window.render_frame(cx);
            let expected = if step["active"] == true {
                "active"
            } else {
                "idle"
            };
            assert_eq!(
                window.find("foreground-loading-bar").description(),
                Some(expected)
            );
        }
        "expectFriendlyError" => {
            window.render_frame(cx);
            let alert = window.find(friendly_panel(string(step, "title")));
            assert_eq!(alert.label(), Some(string(step, "title")));
            assert!(
                alert
                    .description()
                    .is_some_and(|text| text.contains(string(step, "next"))),
                "friendly error next step missing"
            );
        }
        "inspectRequiredPermission" => {
            window.render_frame(cx);
            window.click("detail-error:permission", cx);
        }
        "expectPermissionTarget" => {
            window.render_frame(cx);
            assert_eq!(window.find("rbac-verb").value(), Some(string(step, "verb")));
            assert_eq!(
                window.find("rbac-resource").value(),
                Some(string(step, "resource"))
            );
        }
        "returnToSource" => window.click("permission-return", cx),
        "copyResourceName" => {
            let id = format!(
                "resource-copy:{}:{}:{}",
                string(step, "kind"),
                string(step, "namespace"),
                string(step, "name")
            );
            reveal(window, "page", &id, cx);
            locate::click_in(window, "page", id.into(), cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(string(step, "name").to_owned())
            );
        }
        "expectCopyToast" => {
            window.render_frame(cx);
            assert_eq!(
                window.find("copy-toast").label(),
                Some(string(step, "text"))
            );
        }
        "dismissCopyToast" => window.click("copy-toast-dismiss", cx),
        _ => return false,
    }
    true
}

/// The native panel for each friendly error the shared scenarios show.
fn friendly_panel(title: &str) -> &'static str {
    match title {
        "Failed to load contexts" => "launcher-context-error",
        "KubeCove does not have permission for this" => "detail-error",
        other => panic!("no friendly error panel titled {other:?}"),
    }
}
