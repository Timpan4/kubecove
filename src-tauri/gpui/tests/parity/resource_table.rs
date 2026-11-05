//! Resource table steps: sortable headers, collapsible group rows, whole-row
//! opening and the text each cell shows.

use super::*;

/// The table's visible resource names in visual order, limited to `expected`.
fn visible_names(window: &mut gpui_kit::Window, expected: &[String]) -> Vec<String> {
    window
        .within("resource-table")
        .elements()
        .iter()
        .filter_map(|element| {
            element
                .label()?
                .strip_prefix("Open resource ")
                .map(str::to_owned)
        })
        .filter(|name| expected.contains(name))
        .collect()
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "sortResourceTable" => {
            let id = format!("sort:{}", string(step, "column"));
            reveal(window, "resource-table", &id, cx);
            window.click(id, cx);
        }
        "expectResourceOrder" => {
            let expected = strings(step, "names");
            assert_eq!(visible_names(window, &expected), expected);
        }
        "toggleResourceGroup" => {
            let target = locate::id(
                &window
                    .within("resource-table")
                    .find_by_label(string(step, "label")),
            );
            locate::click_in(window, "resource-table", target, cx);
        }
        "expectResourceGroup" => {
            let group = window
                .within("resource-table")
                .find_by_label(string(step, "label"));
            let expected = if step["expanded"] == true {
                "expanded"
            } else {
                "collapsed"
            };
            assert_eq!(group.description(), Some(expected));
        }
        "openResourceRow" => {
            let cell = format!("cell:{}:{}", string(step, "column"), row(step));
            reveal(window, "resources", &cell, cx);
            window.click(cell, cx);
        }
        "expectResourceCell" => {
            let cell = format!("cell:{}:{}", string(step, "column"), row(step));
            assert_eq!(window.find(cell).label(), Some(string(step, "text")));
        }
        _ => return false,
    }
    true
}
