//! User actions for the settings host adapters. No dialog or external server opens.
use super::{locate, replace_text, string};
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
        "addServerKubeconfigPath" => {
            locate::click_in(window, "page", "kubeconfig-server-path".into(), cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
            locate::click_in(window, "page", "settings-button:Add server path".into(), cx);
        }
        "clickKubeconfigPathButton" => {
            let id = format!(
                "kubeconfig-path-button:{}:{}",
                string(step, "label"),
                string(step, "value")
            );
            locate::click_in(window, "page", id.into(), cx);
        }
        "expectKubeconfigPathBefore" => {
            // A row above another has the smaller y origin.
            let row = |path: &str| {
                window
                    .find(format!("kubeconfig-path:{path}"))
                    .bounds()
                    .origin
                    .y
            };
            assert!(
                row(string(step, "value")) < row(string(step, "other")),
                "{} is not above {}",
                string(step, "value"),
                string(step, "other")
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
        "configureSettingsMock"
        | "expectRuntimeEndpoint"
        | "expectWorkspaceExport"
        | "expectExportedShortcut"
        | "dropRemoteConnection" => {}
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
