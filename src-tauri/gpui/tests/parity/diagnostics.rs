//! Latency report view timings and the interaction recorder's controls.
use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectViewTiming" => {
            // GPUI times `<view>.present`; the web times the view's `<view>.fetch.done`.
            let id = format!("latency-command:{}.present", string(step, "view"));
            reveal(window, "settings-heading", &id, cx);
            let label = window.find(id).label().unwrap_or_default().to_owned();
            assert!(label.contains("ms"), "missing measured duration in {label}");
        }
        "copyInteractions" | "clearInteractions" => {
            let id = if string(step, "action") == "copyInteractions" {
                // The test clipboard starts empty, so a copy must write the text.
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(String::new()));
                "interactions-copy"
            } else {
                "interactions-clear"
            };
            reveal(window, "settings-heading", id, cx);
            window.click(id, cx);
        }
        "expectInteractionCopy" => {
            let copied = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default();
            assert!(!copied.is_empty(), "nothing copied");
            if let Some(contains) = step["contains"].as_str() {
                assert!(
                    copied.contains(contains),
                    "interaction copy lacks {contains}"
                );
            }
            if let Some(excludes) = step["excludes"].as_str() {
                assert!(
                    !copied.contains(excludes),
                    "interaction copy includes {excludes}"
                );
            }
        }
        "typeSettingsField" => {
            assert_eq!(string(step, "field"), "Server token");
            replace_text(window, cx, "remote-server-token", string(step, "value"));
        }
        _ => return false,
    }
    true
}
