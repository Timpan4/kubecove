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
            let title = string(step, "title");
            let alert = match title {
                "Failed to load contexts" => window.find("launcher-context-error"),
                "KubeCove does not have permission for this" => window.find("detail-error"),
                // Page panels keep their page ID, so the classified title finds them.
                _ => {
                    let mut alerts = window.find_all_by_label(title);
                    alerts.retain(|element| element.role() == Some(gpui_kit::Role::Alert));
                    assert_eq!(alerts.len(), 1, "one alert titled {title:?}");
                    alerts.remove(0)
                }
            };
            assert_eq!(alert.label(), Some(title));
            let description = alert.description().unwrap_or_default();
            if let Some(next) = step["next"].as_str() {
                assert!(
                    description.contains(next),
                    "friendly error next step missing"
                );
            }
            if let Some(summary) = step["summary"].as_str() {
                assert!(
                    description.contains(summary),
                    "friendly error summary missing"
                );
            }
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
