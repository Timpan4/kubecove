//! Resource table steps: sortable headers, collapsible group rows, whole-row
//! opening and the text each cell shows.

use super::*;
use gpui_kit::test::TestEventExt;
use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};

fn reveal_cell(window: &mut gpui_kit::Window, id: &str, cx: &mut gpui_kit::App) {
    reveal(window, "resources", "resource-table", cx);
    assert!(
        scroll::seek(window, cx, "resource-table", id),
        "resource table did not render {id} after scrolling"
    );
    let target = window.find(id.to_owned());
    if !target.visible() {
        let bounds = window.find("resource-table").bounds();
        let position = bounds.origin + point(px(4.), px(4.));
        window.pointer_move(position, None, cx);
        window.dispatch_event(
            ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(
                    bounds.center().x - target.bounds().center().x,
                    px(0.),
                )),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
}

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
            reveal_cell(window, &id, cx);
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
            reveal_cell(window, &cell, cx);
            window.click(cell, cx);
        }
        "expectResourceCell" => {
            let cell = format!("cell:{}:{}", string(step, "column"), row(step));
            reveal_cell(window, &cell, cx);
            assert_eq!(window.find(cell).label(), Some(string(step, "text")));
        }
        "expectResourceCellFits" => {
            let column = string(step, "column");
            let cell = format!("cell:{column}:{}", row(step));
            let value = format!("value:{column}:{}", row(step));
            reveal_cell(window, &cell, cx);
            let outer = window.find(cell);
            let inner = window.find(value);
            assert_eq!(outer.label(), Some(string(step, "text")));
            assert_eq!(inner.description(), Some(string(step, "text")));
            let (cell_bounds, value_bounds) = (outer.bounds(), inner.bounds());
            assert!(
                value_bounds.left() >= cell_bounds.left()
                    && value_bounds.right() <= cell_bounds.right(),
                "{column} value escapes its cell"
            );
            assert!(
                value_bounds.size.width < cell_bounds.size.width,
                "{column} value fills its cell's padding"
            );
            // `truncated` is not checked here. This driver shapes text with GPUI's
            // `NoopTextSystem`, where every glyph is 0.6 em wide, so its widths say nothing about
            // the bundled fonts. `tests/text_fit.rs` checks truncation with the real text system.
        }
        _ => return false,
    }
    true
}
