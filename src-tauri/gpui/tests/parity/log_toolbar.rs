use super::*;

/// Whether two element boxes are apart, so neither draws over the other.
fn apart(a: gpui_kit::Bounds<gpui_kit::Pixels>, b: gpui_kit::Bounds<gpui_kit::Pixels>) -> bool {
    a.right() <= b.left() || b.right() <= a.left() || a.bottom() <= b.top() || b.bottom() <= a.top()
}

pub(super) fn act(window: &mut gpui_kit::Window, _cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectLogToolbarFit" => {
            // The status text may shrink, but its box must not overlap the picker or the
            // buttons, and every button stays inside the detail panel.
            let state = string(step, "state");
            let status = window.find("log-status");
            assert!(
                status.label().unwrap_or_default().starts_with(state),
                "status {:?} does not start with {state}",
                status.label()
            );
            let status = status.bounds();
            let panel = window.find("detail-body").bounds();
            for id in ["log-container", "log-tail", "log-pause"] {
                let other = window.find(id).bounds();
                assert!(
                    apart(status, other),
                    "log status {status:?} overlaps {id} {other:?}"
                );
                assert!(
                    other.left() >= panel.left() && other.right() <= panel.right(),
                    "{id} {other:?} leaves the detail panel {panel:?}"
                );
            }
        }
        _ => return false,
    }
    true
}
