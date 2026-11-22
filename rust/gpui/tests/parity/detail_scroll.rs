//! A wheel input must move detail content, rather than only draw the same scene.
use gpui_kit::test::{TestEventExt, TestWindowExt};
use gpui_kit::{App, InputEvent, ScrollDelta, ScrollEvent, Window, point, px};
use serde_json::Value;

pub(super) fn verify(window: &mut Window, cx: &mut App, step: &Value) {
    let anchor = if super::string(step, "view") == "events" {
        "events-search"
    } else {
        "incident-timeline"
    };
    let before = window.find(anchor).bounds().top();
    let bounds = window.find("detail-body").bounds();
    let position = point(
        bounds.left() + px(4.),
        bounds.top() + bounds.size.height / 2.,
    );
    window.pointer_move(position, None, cx);
    window.dispatch_event(
        ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), -bounds.size.height)),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    assert!(
        window.find(anchor).bounds().top() < before,
        "detail content did not move after wheel input"
    );
}
