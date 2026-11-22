use super::*;

/// Generic Details status sections: the health evidence disclosure and the
/// text checks for conditions, metadata and raw status.
pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "toggleDetailDisclosure" => {
            let id = match string(step, "label") {
                "Health evidence" => "details-health-evidence",
                other => panic!("unknown detail disclosure: {other}"),
            };
            reveal(window, "detail-body", id, cx);
            window.click(id, cx);
        }
        "expectNoPageText" => assert!(
            window.try_find_by_label(string(step, "text")).is_none(),
            "{} is still visible",
            string(step, "text")
        ),
        _ => return false,
    }
    true
}
