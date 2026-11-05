//! Scrolling for the parity driver: bring an element into view before a
//! click, and find rows a virtualized page list has not mounted yet.

use gpui_kit::test::{TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};

/// Scrolls an element into view before a click, as Playwright does. The wheel
/// goes to the top-left corner of `anchor`, a visible element at the top of
/// the same scroll area, so a nested scroll area such as the ownership map
/// does not take it. The element lands mid-window.
pub fn reveal(window: &mut gpui_kit::Window, anchor: &str, id: &str, cx: &mut gpui_kit::App) {
    let id = gpui_kit::ElementId::Name(id.to_owned().into());
    window.render_frame(cx);
    let target = window.find(id.clone());
    let anchor = if target
        .path()
        .iter()
        .any(|part| part == &gpui_kit::ElementId::Name("argo-connection-dialog".into()))
    {
        "argo-connection-dialog"
    } else {
        anchor
    };
    let scroll_container = matches!(
        anchor,
        "detail-body" | "argo-connection-dialog" | "actions-scroll"
    );
    let anchor = window.find(gpui_kit::ElementId::Name(anchor.to_owned().into()));
    if target.visible()
        && (!scroll_container
            || (target.bounds().top() >= anchor.bounds().top()
                && target.bounds().bottom()
                    <= anchor.bounds().bottom().min(window.viewport_size().height)))
    {
        return;
    }
    assert!(
        anchor.visible(),
        "scroll anchor {:?} is not visible",
        anchor.path()
    );
    let position = anchor.bounds().origin + point(px(4.), px(4.));
    let delta = window.viewport_size().height / 2. - target.bounds().center().y;
    window.pointer_move(position, None, cx);
    window.dispatch_event(
        ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), delta)),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

/// What a scroll can change: every rendered element and where it is.
fn scene(window: &gpui_kit::Window) -> Vec<(String, gpui_kit::Bounds<gpui_kit::Pixels>)> {
    window
        .elements()
        .iter()
        .map(|element| (format!("{:?}", element.path()), element.bounds()))
        .collect()
}

/// Scrolls a page's virtualized `list`, down and then up, until `id` renders,
/// as Playwright waits for a row a virtual collection mounts on scroll. A
/// direction ends when a wheel step no longer changes the scene. Returns
/// whether `id` rendered; a found target is then scrolled into view.
pub fn seek(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, list: &str, id: &str) -> bool {
    let target = gpui_kit::ElementId::Name(id.to_owned().into());
    for direction in [-1., 1.] {
        loop {
            window.render_frame(cx);
            if window.try_find(target.clone()).is_some() {
                reveal(window, list, id, cx);
                return true;
            }
            let before = scene(window);
            let anchor = gpui_kit::ElementId::Name(list.to_owned().into());
            let position = window.find(anchor).bounds().origin + point(px(4.), px(4.));
            window.pointer_move(position, None, cx);
            window.dispatch_event(
                ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(
                        px(0.),
                        window.viewport_size().height / 2. * direction,
                    )),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            if scene(window) == before {
                break;
            }
        }
    }
    false
}
