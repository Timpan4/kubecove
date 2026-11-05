use super::*;
use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};

pub(super) fn check(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, check: &str) {
    match check {
        "emit-log" => {}
        "log-wrap" => {
            let before = window.find(("log-row", 0usize)).bounds().size.height;
            window.click("log-wrap", cx);
            window.render_frame(cx);
            assert!(window.find(("log-row", 0usize)).bounds().size.height < before);
            window.click("log-wrap", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find(("log-row", 0usize)).bounds().size.height,
                before
            );
        }
        "log-scroll-back" => {
            let bounds = window.find("log-viewport").bounds();
            let elements = window.elements();
            let row = elements.iter().find(|element| matches!(element.path().last(), Some(gpui_kit::ElementId::NamedInteger(name, _)) if name.as_ref() == "log-row")).expect("a visible log row");
            // The scenario requests the production default tail of 200 lines.
            let delta = row.bounds().size.height * 200.;
            window.dispatch_event(
                ScrollEvent {
                    position: bounds.center(),
                    delta: ScrollDelta::Pixels(point(px(0.), delta)),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        }
        "log-jump" => {
            assert_eq!(window.find("log-jump").label(), Some("1 new line"));
            window.click("log-jump", cx);
            window.render_frame(cx);
            assert!(window.elements().iter().any(|element| {
                element.visible()
                    && element
                        .label()
                        .is_some_and(|label| label.contains("Newest controlled line"))
            }));
            assert!(window.try_find("log-jump").is_none());
        }
        _ => panic!("unknown log check {check}"),
    }
}
