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
        "addPastedKubeconfigPaths" | "fillKubeconfigPathDraft" => {
            locate::click_in(window, "page", "kubeconfig-paths-input".into(), cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
            if string(step, "action") == "addPastedKubeconfigPaths" {
                locate::click_in(
                    window,
                    "page",
                    "settings-button:Add pasted paths".into(),
                    cx,
                );
            }
        }
        "expectKubeconfigPathDraft" => {
            window.render_frame(cx);
            // The field of the current mode: the server path when remote, else the pasted paths.
            let (label, id) = if window
                .try_find_by_label("Kubeconfig path on server")
                .is_some()
            {
                ("Kubeconfig path on server", "kubeconfig-server-path")
            } else {
                ("Paste kubeconfig paths", "kubeconfig-paths-input")
            };
            let draft = |window: &mut Window| {
                window
                    .find_by_label(label)
                    .value()
                    .unwrap_or_default()
                    .to_owned()
            };
            let value = string(step, "value");
            assert_eq!(draft(window), value);
            // GPUI inputs expose no disabled flag, so typing tells: a disabled field keeps its text.
            let probe = format!("{value}#");
            locate::click_in(window, "page", id.into(), cx);
            window.press("secondary-a", cx);
            window.input(&probe, cx);
            window.render_frame(cx);
            let accepted = draft(window) == probe;
            if accepted {
                window.press("secondary-a", cx);
                if value.is_empty() {
                    window.press("backspace", cx);
                } else {
                    window.input(value, cx);
                }
                window.render_frame(cx);
            }
            let enabled = step["enabled"].as_bool().expect("missing enabled");
            assert_eq!(accepted, enabled, "{label} accepts input");
            assert_eq!(draft(window), value);
        }
        "expectAccessibleName" => {
            let name = string(step, "name");
            assert!(
                !window.find_all_by_label(name).is_empty(),
                "no element is labeled {name:?}"
            );
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
        | "dropRemoteConnection"
        | "remoteSessionSignal" => {}
        "expectRemoteNotice" => {
            let texts: Vec<String> = [
                "remote-content-notice-title",
                "remote-content-notice-message",
            ]
            .into_iter()
            .filter_map(|id| {
                window
                    .try_find(id)
                    .and_then(|element| element.label().map(str::to_owned))
            })
            .collect();
            assert!(
                texts
                    .iter()
                    .any(|text| text.contains(string(step, "value"))),
                "Missing remote notice {:?}; notice: {texts:?}",
                string(step, "value")
            );
        }
        "expectNoRemoteNotice" => {
            assert!(window.try_find("remote-content-notice-message").is_none());
        }
        "clickRemoteNoticeButton" => window.click(
            format!("remote-content-notice-button:{}", string(step, "label")),
            cx,
        ),
        "reopenStoredSettings" => {
            window.click("settings-back", cx);
        }
        _ => return false,
    }
    true
}
