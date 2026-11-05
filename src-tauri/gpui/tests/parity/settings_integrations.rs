//! User actions for the settings host adapters. No dialog or external server opens.
use super::{replace_text, string};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{App, Window};
use serde_json::Value;

pub fn act(window: &mut Window, cx: &mut App, step: &Value) -> bool {
    match string(step, "action") {
        "fillSettingsField" => replace_text(
            window,
            cx,
            if string(step, "field") == "Server URL" {
                "remote-server-url"
            } else {
                "remote-server-token"
            },
            string(step, "value"),
        ),
        "clickSettingsButton" => {
            window.click(format!("settings-button:{}", string(step, "label")), cx)
        }
        "expectSettingsText" => {
            assert!(
                window.elements().iter().any(|element| element
                    .label()
                    .is_some_and(|text| text.contains(string(step, "value")))),
                "Missing settings text {:?}; rendered labels: {:?}",
                string(step, "value"),
                window
                    .elements()
                    .iter()
                    .filter_map(|element| element.label().map(str::to_owned))
                    .collect::<Vec<_>>()
            );
        }
        "expectNoSettingsText" => {
            assert!(
                !window.elements().iter().any(|element| element
                    .label()
                    .is_some_and(|text| text.contains(string(step, "value")))),
                "Unexpected settings text"
            );
        }
        "expectMaskedToken" => {
            // GPUI Kit excludes password values from accessibility and test snapshots.
            assert!(!window.elements().iter().any(|element| {
                element
                    .value()
                    .is_some_and(|text| text.contains("parity-controlled-credential"))
            }));
        }
        "configureSettingsMock" | "expectRuntimeEndpoint" | "expectWorkspaceExport" => {}
        "reopenStoredSettings" => {
            window.click("settings-back", cx);
        }
        "attemptUnverifiedUpdate" => {
            let install = window.find("settings-button:Install");
            assert!(
                install
                    .description()
                    .is_some_and(|text| text.contains("signed GPUI"))
            );
            window.click("settings-button:Install", cx);
        }
        _ => return false,
    }
    true
}
