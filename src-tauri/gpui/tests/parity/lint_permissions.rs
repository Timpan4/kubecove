//! Shared steps for the Kubernetes YAML lint shown in the editor and for the
//! permission read that locks the Actions tab.

use super::*;

/// The text the web and native rows share: `source - path`, or the source alone.
fn detail(step: &Value) -> String {
    match step["path"].as_str() {
        Some(path) if !path.is_empty() => format!("{} - {path}", string(step, "source")),
        _ => string(step, "source").to_owned(),
    }
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectYamlDiagnostic" => {
            reveal(window, "detail-body", "yaml-diagnostic:0", cx);
            let detail = detail(step);
            let row = window
                .find_all_by_label(string(step, "message"))
                .into_iter()
                .find(|row| row.description() == Some(detail.as_str()))
                .unwrap_or_else(|| panic!("no diagnostic row for {:?}", string(step, "message")));
            // The badge shares the row's index, so another finding's badge cannot answer.
            let row_id = locate::id(&row).to_string();
            let index = row_id
                .strip_prefix("yaml-diagnostic:")
                .unwrap_or_else(|| panic!("unexpected diagnostic row ID {row_id}"));
            assert_eq!(
                window
                    .find(format!("yaml-diagnostic-severity:{index}"))
                    .label(),
                Some(string(step, "severity")),
                "wrong severity badge for {detail:?}"
            );
        }
        "expectYamlApplyDescription" => {
            reveal(window, "detail-body", "yaml-apply", cx);
            assert_eq!(
                window.find("yaml-apply").description(),
                Some(string(step, "text"))
            );
        }
        "expectNoYamlApplyDescription" => {
            reveal(window, "detail-body", "yaml-apply", cx);
            assert_eq!(window.find("yaml-apply").description(), None);
        }
        "expectNoYamlDiagnostic" => assert!(
            window.find_all_by_label(string(step, "message")).is_empty(),
            "unexpected diagnostic {:?}",
            string(step, "message")
        ),
        "expectActionLoadError" => assert_eq!(
            window.find("action-load-error").label(),
            Some(string(step, "text"))
        ),
        "expectNoActionLoadError" => assert!(window.try_find("action-load-error").is_none()),
        "retryActionPermission" => window.click("action-load-retry", cx),
        "expectActionLock" => {
            let id = format!("action-lock:{}", string(step, "row"));
            if step["locked"] == true {
                reveal(window, "actions-scroll", &id, cx);
                let lock = window.find(id.clone());
                if let Some(reason) = step["reason"].as_str() {
                    assert!(
                        lock.label().is_some_and(|label| label.contains(reason)),
                        "{id} does not show {reason:?}"
                    );
                }
            } else {
                assert!(window.try_find(id).is_none());
            }
        }
        _ => return false,
    }
    true
}
