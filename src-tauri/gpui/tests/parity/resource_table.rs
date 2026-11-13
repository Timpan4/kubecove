//! Resource table steps: sortable headers, collapsible group rows, whole-row
//! opening, the columns a page admits, the text each cell shows and row activity.

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

/// The element ID of the row a step names, scrolled into the table.
fn resolved_row(window: &mut gpui_kit::Window, step: &Value, cx: &mut gpui_kit::App) -> String {
    let target = row(step);
    detail::resolve_resource(window, &target, cx)
        .unwrap_or_else(|| panic!("resource table did not render {target}"))
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

/// Presses Tab until the control `id` has keyboard focus, as a keyboard user
/// reaches it. The web driver's `press` focuses the name button first; GPUI's
/// scoped `press` dispatches to the current focus and does not move it.
pub(crate) fn tab_to(window: &mut gpui_kit::Window, id: &str, cx: &mut gpui_kit::App) {
    let mut first: Option<gpui_kit::FocusHandle> = None;
    loop {
        window.press("tab", cx);
        if locate::focus_state(&window.find(id.to_owned())) == Some(true) {
            return;
        }
        let focused = window.focused(cx).expect("Tab focused no control");
        match &first {
            None => first = Some(focused),
            Some(first) => assert!(*first != focused, "Tab cycled without reaching {id}"),
        }
    }
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "pressResourceRow" => {
            let id = resolved_row(window, step, cx);
            reveal_cell(window, &id, cx);
            tab_to(window, &id, cx);
            let key = if string(step, "key") == "Enter" {
                "enter"
            } else {
                "space"
            };
            window.within(id).press(key, cx);
        }
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
            // The table mounts only the group headers near its viewport.
            detail::scan_table(window, cx, |window| {
                use gpui_kit::test::TestQueryExt;
                window
                    .elements()
                    .iter()
                    .any(|element| element.label() == Some(string(step, "label")))
            });
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
            let cell = format!(
                "cell:{}:{}",
                string(step, "column"),
                resolved_row(window, step, cx)
            );
            reveal_cell(window, &cell, cx);
            window.click(cell, cx);
        }
        "expectResourceCell" => {
            let cell = format!(
                "cell:{}:{}",
                string(step, "column"),
                resolved_row(window, step, cx)
            );
            reveal_cell(window, &cell, cx);
            assert_eq!(window.find(cell).label(), Some(string(step, "text")));
        }
        "expectResourceCellFits" => {
            let column = string(step, "column");
            let row = resolved_row(window, step, cx);
            let cell = format!("cell:{column}:{row}");
            let value = format!("value:{column}:{row}");
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
        "expectResourceColumns" => {
            reveal(window, "resources", "resource-table", cx);
            // Sortable headers are `sort:<label>` buttons; the others are `head:<label>`.
            let mut headers: Vec<_> = window
                .within("resource-table")
                .elements()
                .iter()
                .filter_map(|element| {
                    let id = locate::id(element).to_string();
                    let label = id
                        .strip_prefix("sort:")
                        .or_else(|| id.strip_prefix("head:"))?;
                    (!label.is_empty()).then(|| (element.bounds().left(), label.to_owned()))
                })
                .collect();
            headers.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("header positions are finite"));
            let labels: Vec<String> = headers.into_iter().map(|(_, label)| label).collect();
            assert_eq!(labels, strings(step, "columns"));
        }
        "expectResourceActivity" => {
            let id = format!(
                "activity:status:{}:{}:{}",
                string(step, "kind"),
                string(step, "namespace"),
                string(step, "name")
            );
            let state = string(step, "state");
            assert_eq!(window.find(id.clone()).description(), Some(state));
            // An active arc asks for the next frame; a paused one leaves the window idle.
            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            let scheduled = window.simulate_next_frame(cx);
            if state == "active" {
                assert!(scheduled > 0, "active activity scheduled no frame");
            } else {
                assert_eq!(scheduled, 0, "paused activity scheduled frames");
            }
        }
        _ => return false,
    }
    true
}
