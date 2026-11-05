use super::*;

/// Steps for the Details metadata controls. Labels match the visible text the
/// web driver clicks, and the scrolled detail body is revealed before a click.
pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "clickDetailControl" => {
            locate::click_label(window, "detail-body", string(step, "label"), cx)
        }
        "expectNoPageText" => assert!(window.try_find_by_label(string(step, "text")).is_none()),
        _ => return false,
    }
    true
}
