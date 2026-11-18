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
        "log-compact-wrap" => {
            let row = ("log-row", 0usize);
            let summary = ("log-summary", 0usize);
            let bounds = window.find(row).bounds();
            assert!(
                bounds.right() <= window.find("log-viewport").bounds().right(),
                "{bounds:?}"
            );
            let summary_bounds = window.find(summary).bounds();
            assert!(summary_bounds.right() <= bounds.right());
            let collapsed = summary_bounds.size.height;
            let short = window.find(("log-summary", 1usize)).bounds().size.height;
            assert_eq!(collapsed, short);
            let structured = window.find(("log-summary", 2usize)).bounds().size.height;
            assert_eq!(
                structured, short,
                "a decoded line break must leave the row compact"
            );
            window.click(summary, cx);
            window.render_frame(cx);
            window.find(("log-copy", 0usize));
            assert!(window.find(summary).bounds().size.height > collapsed);
        }
        "log-compact-source" => {
            assert_eq!(
                window.find(("log-row", 0usize)).label(),
                Some("boot payments-api")
            );
            let source = window.find(("log-source", 0usize)).bounds();
            let headline = window.find(("log-summary", 0usize)).bounds();
            let viewport = window.find("log-viewport").bounds();
            assert!(source.right() <= headline.left());
            assert!(source.size.width < headline.size.width);
            assert!(headline.right() <= viewport.right());
            window.click(("log-summary", 0usize), cx);
            window.render_frame(cx);
            let expanded_source = window.find(("log-source", 0usize)).bounds();
            let expanded = window.find(("log-summary", 0usize)).bounds();
            assert!(expanded_source.bottom() <= expanded.top());
            assert_eq!(expanded.left(), source.left());
            assert_eq!(expanded.right(), headline.right());
        }
        "log-expanded-field" => {
            let row = ("log-row", 0usize);
            assert_eq!(window.find(row).label(), Some("JSON row"));
            window.click(row, cx);
            window.render_frame(cx);
            assert!(window.find(("log-details", 0usize)).visible());
            assert!(window.find(("log-copy", 0usize)).visible());
            let details = window.within(("log-details", 0usize));
            let short = details.find(("log-field", 0usize)).bounds();
            let long = details.find(("log-field", 1usize)).bounds();
            assert!(long.right() <= window.find("log-viewport").bounds().right());
            assert!(long.size.height > short.size.height);
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
