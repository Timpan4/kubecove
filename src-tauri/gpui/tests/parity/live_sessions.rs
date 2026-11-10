use super::{number, replace_text, string};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{App, Window};
use serde_json::Value;

pub fn apply(window: &mut Window, cx: &mut App, step: &Value) -> bool {
    match string(step, "action") {
        "configurePreset" => {
            for (id, key) in [
                ("preset-label", "label"),
                ("preset-context", "context"),
                ("preset-namespace", "namespace"),
                ("preset-service", "service"),
                ("preset-remote", "remote"),
                ("preset-local", "local"),
            ] {
                replace_text(window, cx, id, string(step, key));
            }
        }
        "presetControl" => match string(step, "operation") {
            "add" => window.click("save-forward", cx),
            "save" => window.click("preset-save", cx),
            "absent" => assert!(window.find_all_by_label(string(step, "label")).is_empty()),
            operation => {
                assert_eq!(window.find("preset:0").label(), Some(string(step, "label")));
                let prefix = match operation {
                    "edit" => "edit",
                    "start" => "start",
                    "remove" => "remove",
                    _ => panic!("unknown preset operation"),
                };
                window.click(format!("{prefix}-preset:0"), cx);
            }
        },
        "replaySessions" => {}
        "confirmExecLeave" => window.click(
            if string(step, "choice") == "stay" {
                "exec-leave-cancel"
            } else {
                "exec-leave-confirm"
            },
            cx,
        ),
        "configureForward" => {
            replace_text(window, cx, "forward-local", string(step, "local"));
            if window.try_find("forward-service-port").is_some() {
                window.within("forward-service-port").click("input", cx);
                window.input(string(step, "remote"), cx);
                window.press("enter", cx);
            } else {
                replace_text(window, cx, "forward-remote", string(step, "remote"));
            }
        }
        "forwardControl" => {
            let op = string(step, "operation");
            match op {
                "start" => window.click("forward-start", cx),
                "save" => window.click("forward-save", cx),
                "restore" => window.click("restore-forwards", cx),
                "stop" | "reconnect" => {
                    let target = string(step, "target");
                    window.click(format!("{op}-forward:{target}"), cx);
                }
                _ => panic!("unknown forward operation"),
            }
        }
        "expectForwardNotice" => {
            assert!(
                window.try_find("forward-error").is_none(),
                "Forward tab shows an error: {:?}",
                window
                    .try_find("forward-error")
                    .and_then(|node| node.label().map(str::to_owned))
            );
            let notice = window.find("forward-notice");
            assert_eq!(notice.label(), Some(string(step, "text")));
            assert_eq!(notice.description(), Some(string(step, "title")));
        }
        "expectForwardState" => {
            let target = string(step, "target");
            let mut matches = window.find_all_by_label(target);
            if matches.is_empty() {
                let suffix = format!("/{target}");
                matches = window
                    .elements()
                    .iter()
                    .filter(|node| node.label().is_some_and(|label| label.ends_with(&suffix)))
                    .cloned()
                    .collect();
            }
            assert_eq!(
                matches.len() as u64,
                number(step, "count"),
                "Session action: {:?}",
                window
                    .try_find("session-action-message")
                    .and_then(|node| node.label().map(str::to_owned))
            );
            if number(step, "count") > 0 {
                let description = matches[0].description().unwrap();
                assert!(description.contains(string(step, "local")));
                assert!(description.contains(string(step, "status")));
                if let Some(resolved) = step["resolved"].as_str() {
                    assert!(description.contains(resolved));
                }
            }
        }
        "execControl" => match string(step, "operation") {
            "stop" => window.click("exec-stop", cx),
            "custom" => {
                window.click("exec-other", cx);
                window.render_frame(cx);
                window.click("exec-custom", cx);
                window.render_frame(cx);
                window.within("exec-custom-container").click("input", cx);
                window.press("down", cx);
                window.press("enter", cx);
                window.render_frame(cx);
            }
            "arguments" => {
                let input = window
                    .find_by_label("Exec arguments")
                    .path()
                    .iter()
                    .find(|id| matches!(id, gpui_kit::ElementId::NamedInteger(_, _)))
                    .unwrap()
                    .clone();
                window.within(input).click("input-state", cx);
                assert_eq!(
                    window.find_by_label("Exec arguments").focused(),
                    Some(true),
                    "{:?}",
                    window.find_by_label("Exec arguments")
                );
                window.input(string(step, "command"), cx);
                assert_eq!(
                    window.find_by_label("Exec arguments").value(),
                    Some(string(step, "command"))
                );
            }
            // Cmd on macOS, Ctrl elsewhere, like the web's `ControlOrMeta+Enter`.
            "shortcut" => {
                window.press("secondary-enter", cx);
                window.render_frame(cx);
            }
            "run" => {
                assert_eq!(
                    window.find("exec-custom-run").label(),
                    Some(
                        format!(
                            "Run {} in {}",
                            string(step, "command"),
                            string(step, "container")
                        )
                        .as_str()
                    )
                );
                window.click("exec-custom-run", cx);
            }
            "start" => {
                let container = string(step, "container");
                if container == "app" && window.try_find("exec-start").is_some() {
                    window.click("exec-start", cx);
                } else {
                    window.click(
                        if window.try_find("exec-other").is_some() {
                            "exec-other"
                        } else {
                            "exec-new"
                        },
                        cx,
                    );
                    window.render_frame(cx);
                    window.click(
                        format!("exec-open:{container}:{}", string(step, "command")),
                        cx,
                    );
                }
            }
            "select" => window.click(format!("exec-tab:{}", step["tab"].as_u64().unwrap()), cx),
            "input" => {
                window.click("exec-terminal", cx);
                for part in string(step, "data").split_inclusive('\r') {
                    window.input(part.trim_end_matches('\r'), cx);
                    if part.ends_with('\r') {
                        window.press("enter", cx);
                    }
                }
            }
            "close" => window.click(
                format!("exec-close:{}", step["tab"].as_u64().unwrap_or(0)),
                cx,
            ),
            _ => panic!("unknown exec operation"),
        },
        "expectExecState" => {
            if string(step, "status") == "closed" {
                assert!(window.try_find("exec-terminal").is_none());
            } else {
                assert_eq!(
                    window
                        .find_all_by_label(&format!(
                            "{} · {}, {}",
                            step["command"].as_str().unwrap_or("sh"),
                            string(step, "container"),
                            string(step, "status")
                        ))
                        .len(),
                    1,
                    "exec terminal: {:?}",
                    window
                        .try_find("exec-terminal")
                        .and_then(|e| e.label().map(str::to_owned))
                );
                assert!(
                    window
                        .find("exec-terminal")
                        .label()
                        .unwrap()
                        .contains(string(step, "contains"))
                );
                if let Some(absent) = step["absent"].as_str() {
                    assert!(
                        !window
                            .find("exec-terminal")
                            .label()
                            .unwrap()
                            .contains(absent)
                    );
                }
            }
        }
        _ => return false,
    }
    true
}
