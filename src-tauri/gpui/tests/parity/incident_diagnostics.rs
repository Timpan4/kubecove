//! Incident handoffs to Resources, the header update control, and latency report copy.
use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openIncidentResources" => window.click("incidents-open-resources", cx),
        "expectHealthFilter" => {
            let id = format!("health-filter:{}", string(step, "label"));
            assert_eq!(
                window.find(id).description(),
                Some("pressed"),
                "wrong health filter for {step}"
            );
        }
        "updateStatusControl" => {
            if string(step, "control") == "Escape" {
                window.press("escape", cx);
                return true;
            }
            let id = match string(step, "control") {
                "Open" => "header-update",
                "Check" => "header-update-check",
                "Install" => "header-update-install",
                "Relaunch" => "header-update-relaunch",
                _ => "header-update-later",
            };
            window.click(id, cx);
        }
        "expectUpdateControl" => {
            assert_eq!(
                window.find("header-update").label(),
                Some(string(step, "label"))
            );
        }
        "copyLatencyReport" => {
            reveal(window, "settings-heading", "latency-copy", cx);
            window.click("latency-copy", cx);
        }
        "expectLatencyReportCopy" => {
            let copied = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default();
            assert!(
                copied.contains(string(step, "contains")),
                "wrong latency report copy"
            );
        }
        _ => return false,
    }
    true
}
