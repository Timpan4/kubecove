use super::*;

/// Steps for the Details metadata controls. Labels match the visible text the
/// web driver clicks, and the scrolled detail body is revealed before a click.
pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "clickDetailControl" => {
            let target = locate::id(
                &window
                    .within("detail-body")
                    .find_by_label(string(step, "label")),
            );
            let gpui_kit::ElementId::Name(name) = &target else {
                panic!("detail control must have a named ID");
            };
            reveal(window, "detail-body", name.as_ref(), cx);
            window.click(target, cx);
        }
        _ => return false,
    }
    true
}
