//! Multi-context Resources steps: the contexts of a row identity, the Context
//! filter, the table's status line and the ownership map's "Map context" select.

use super::*;

/// The contexts of every rendered row with the step's identity, top to bottom.
/// Row IDs end in their context (`table_columns::resource_id`).
fn row_contexts(window: &mut gpui_kit::Window, step: &Value, cx: &mut gpui_kit::App) -> Vec<String> {
    let identity = row(step);
    if !detail::reveal_resource(window, &identity, cx) {
        return Vec::new();
    }
    let prefix = format!("{identity}:");
    detail::rendered_rows(window, &identity)
        .into_iter()
        .map(|id| {
            id.strip_prefix(&prefix)
                .unwrap_or_else(|| panic!("row {id} has no context"))
                .to_owned()
        })
        .collect()
}

/// Sets the Context filter to exactly `wanted`. Checks the wanted contexts
/// first, so the selection never passes through an empty set.
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
        for context in options.iter().filter(|context| wanted.contains(context) == check) {
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
        "selectMapContext" => {
            window.click("map-context", cx);
            window.render_frame(cx);
            window.click(format!("map-context-option:{}", string(step, "context")), cx);
        }
        "expectMapContext" => {
            if step["shown"] == true {
                assert!(window.find("map-context").visible(), "Map context is hidden");
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
