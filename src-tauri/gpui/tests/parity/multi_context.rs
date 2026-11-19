//! Multi-context Resources steps: the contexts of a row identity, the Context
//! filter, the table's status line and the ownership map's "Map context" select.

use super::*;

/// The contexts of every row with the step's identity on the current table
/// page, top to bottom. Row IDs end in their context
/// (`table_columns::resource_id`).
fn row_contexts(
    window: &mut gpui_kit::Window,
    step: &Value,
    cx: &mut gpui_kit::App,
) -> Vec<String> {
    let identity = row(step);
    let prefix = format!("{identity}:");
    detail::table_rows(window, &identity, cx)
        .into_iter()
        .map(|id| {
            id.strip_prefix(&prefix)
                .unwrap_or_else(|| panic!("row {id} has no context"))
                .to_owned()
        })
        .collect()
}

/// Sets the Context filter to exactly `wanted`. Checks the wanted contexts
/// first, so the selection passes through an empty set only when `wanted` is
/// empty: unchecking the last context then clears the filter.
fn filter_contexts(window: &mut gpui_kit::Window, wanted: &[String], cx: &mut gpui_kit::App) {
    window.click("context-picker", cx);
    window.render_frame(cx);
    let options: Vec<String> = window
        .within("context-options")
        .elements()
        .iter()
        .filter_map(|element| {
            locate::id(element)
                .to_string()
                .strip_prefix("context-option:")
                .map(str::to_owned)
        })
        .collect();
    for context in wanted {
        assert!(options.contains(context), "no Context option {context}");
    }
    for check in [true, false] {
        for context in options
            .iter()
            .filter(|context| wanted.contains(context) == check)
        {
            let id = format!("context-option:{context}");
            let checked = window.find(id.clone()).description() == Some("checked");
            if checked != check {
                window.click(id, cx);
                window.render_frame(cx);
            }
        }
    }
    window.click("context-picker", cx);
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectRowContexts" => {
            assert_eq!(row_contexts(window, step, cx), strings(step, "contexts"))
        }
        "filterContexts" => filter_contexts(window, &strings(step, "contexts"), cx),
        "expectResourceSummary" => assert_eq!(
            window.find("resource-table-summary").label(),
            Some(string(step, "text"))
        ),
        "clickMapContextTrigger" => window.click("map-context", cx),
        "selectMapContext" => {
            window.click("map-context", cx);
            window.render_frame(cx);
            window.click(
                format!("map-context-option:{}", string(step, "context")),
                cx,
            );
        }
        "mapContextKey" => {
            // Focus stays on the trigger while the options are open.
            if window.try_find("map-context-options").is_none() {
                super::resource_table::tab_to(window, "map-context", cx);
            }
            let key = match string(step, "key") {
                "Enter" => "enter",
                "Space" => "space",
                "ArrowDown" => "down",
                "ArrowUp" => "up",
                "Home" => "home",
                "End" => "end",
                "Escape" => "escape",
                "Tab" => "tab",
                "Shift+Tab" => "shift-tab",
                other => panic!("unknown Map context key: {other}"),
            };
            window.within("map-context").press(key, cx);
        }
        "expectMapContextOptions" => {
            assert_eq!(
                window.try_find("map-context-options").is_some(),
                step["open"] == true,
                "{step}"
            );
            if let Some(focused) = step["focused"].as_bool() {
                assert_eq!(
                    locate::focus_state(&window.find("map-context")),
                    Some(focused),
                    "{step}"
                );
            }
        }
        "expectMapContext" => {
            if step["shown"] == true {
                assert!(
                    window.find("map-context").visible(),
                    "Map context is hidden"
                );
                assert_eq!(
                    window.find("map-context-value").label(),
                    Some(string(step, "context"))
                );
            } else {
                assert!(window.try_find("map-context").is_none());
            }
        }
        _ => return false,
    }
    true
}
