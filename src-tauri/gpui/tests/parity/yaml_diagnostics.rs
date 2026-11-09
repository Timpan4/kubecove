//! Shared step that compares every diagnostic the YAML editor marks, parser
//! errors and warnings and Kubernetes lint findings, with their ranges.

use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectYamlEditorDiagnostics" => {
            let inspector = window
                .root::<Inspector>()
                .flatten()
                .expect("the parity window shows the Inspector");
            let actual = inspector.read(cx).yaml_editor_diagnostics(cx);
            let mut actual = actual.as_array().expect("a diagnostic list").clone();
            // The web driver sorts CodeMirror's diagnostics the same way.
            actual.sort_by_key(|diagnostic| {
                ["from", "to"].map(|end| {
                    [0, 1].map(|index| diagnostic[end][index].as_u64().expect("a position"))
                })
            });
            let expected = step["diagnostics"].as_array().expect("missing diagnostics");
            let same = actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(got, want)| {
                    ["from", "to", "severity", "source", "message"]
                        .into_iter()
                        .all(|field| want.get(field).is_none() || got[field] == want[field])
                });
            assert!(
                same,
                "editor diagnostics {}, expected {}",
                Value::Array(actual.clone()),
                Value::Array(expected.clone())
            );
        }
        _ => return false,
    }
    true
}
