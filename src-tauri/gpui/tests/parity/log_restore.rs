//! Log row expansion, Copy line and the saved forward restore prompt. A log row
//! is labeled with its headline. Its details exist only while it is open.

use super::string;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{App, ClipboardItem, Window};
use serde_json::Value;

pub fn apply(window: &mut Window, cx: &mut App, step: &Value) -> bool {
    match string(step, "action") {
        "toggleLogRow" => {
            let position = row_position(window, string(step, "line"));
            window.click(("log-summary", position), cx);
        }
        "toggleLogExpandAll" => window.click("log-expand-all", cx),
        "expectLogRowOpen" => {
            let position = row_position(window, string(step, "line"));
            let open = step["open"].as_bool().expect("open flag");
            assert_eq!(
                window.try_find(("log-details", position)).is_some(),
                open,
                "open state of {}",
                string(step, "line")
            );
        }
        "copyLogLine" => {
            let position = row_position(window, string(step, "line"));
            // The test clipboard starts empty, so a copy must write the text.
            cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            window.click(("log-copy", position), cx);
            assert!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .is_some_and(|text| text.contains(string(step, "contains"))),
                "wrong clipboard for {}",
                string(step, "line")
            );
        }
        "expectRestorePrompt" => {
            let visible = step["visible"].as_bool().expect("visible flag");
            let prompt = window.try_find("restore-prompt");
            assert_eq!(prompt.is_some(), visible, "restore prompt visibility");
            if let (Some(prompt), Some(count)) = (prompt, step["count"].as_u64()) {
                let noun = if count == 1 { "forward" } else { "forwards" };
                let text =
                    format!("This workspace has {count} saved Service {noun} ready to start.");
                assert_eq!(prompt.label(), Some(text.as_str()));
            }
        }
        "restorePromptControl" => window.click(prompt_control(string(step, "control")), cx),
        _ => return false,
    }
    true
}

/// The buffer's rendered rows are positions `0..` in the list. Rows that the
/// list virtualizes away are not found, as the shared `expectLogLines` relies on.
fn row_position(window: &Window, line: &str) -> usize {
    (0usize..)
        .map_while(|index| window.try_find(("log-row", index)))
        .position(|row| row.label() == Some(line))
        .unwrap_or_else(|| panic!("no rendered log row labeled {line}"))
}

fn prompt_control(control: &str) -> &'static str {
    match control {
        "Start saved" => "restore-prompt-start",
        "Review" => "restore-prompt-review",
        "Skip" => "restore-prompt-skip",
        "Auto-start" => "restore-prompt-auto-start",
        other => panic!("unknown restore prompt control: {other}"),
    }
}
