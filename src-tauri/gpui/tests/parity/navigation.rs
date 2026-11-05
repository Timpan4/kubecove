//! Namespace list, compact navigation drawer and sidebar tree keys.
//! `resizeViewport` resizes the window in `parity.rs`; this module ignores it.

use super::sidebar_live::{click_sidebar, sidebar_row_id};
use super::*;

fn native_key(key: &str) -> &'static str {
    match key {
        "Enter" => "enter",
        "Space" => "space",
        "ArrowLeft" => "left",
        "ArrowRight" => "right",
        other => panic!("unknown sidebar key: {other}"),
    }
}

pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openNamespaceList" => window.click("view-all-namespaces", cx),
        "expectNamespaceRow" => {
            let row = window.find(format!("namespace-row:{}", string(step, "name")));
            assert_eq!(row.description(), Some(string(step, "age")), "{step}");
        }
        "resizeViewport" => {}
        "openNavigationDrawer" => window.click("open-navigation", cx),
        "expectNavigationDrawer" => assert_eq!(
            window.try_find("workspace-navigation").is_some(),
            step["open"] == true,
            "{step}"
        ),
        "sidebarKey" => {
            let path = strings(step, "path");
            let parts: Vec<&str> = path.iter().map(String::as_str).collect();
            let id = sidebar_row_id(&parts);
            reveal(window, "sidebar-tree", &id, cx);
            // The pointer press focuses the row, as a user does before pressing keys.
            click_sidebar(window, cx, id);
            window.press(native_key(string(step, "key")), cx);
        }
        _ => return false,
    }
    true
}
