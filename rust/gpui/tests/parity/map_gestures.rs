//! Pointer gestures on the ownership map: a drag that starts on a node pans
//! without selecting it, and one wheel step zooms about the cursor. The
//! `map-viewport-*` checks follow the viewport while a selection moves it.
//! Mirrors `e2e/parity/web-map-gestures.ts`.
use super::*;
use gpui_kit::ElementId;
use gpui_kit::test::TestEventExt;
use gpui_kit::{
    InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, px,
};
use std::cell::Cell;

/// Layout rounding allowed when comparing measured node boxes, in pixels.
const LAYOUT_ROUNDING: f32 = 1.;
/// One wheel notch of 100 px, which xyflow turns into a zoom step of `2 ** (100 * 0.002)`.
const WHEEL_PIXELS: f32 = 100.;

/// The `coverageCheck` names that `check` handles.
pub(super) const CHECKS: &[&str] = &[
    "map-drag-node",
    "map-wheel",
    "map-viewport-settled",
    "map-viewport-held",
    "map-viewport-moved",
];

thread_local! {
    /// The viewport the last `map-viewport-settled` or `map-viewport-moved` check saw.
    static RECORDED: Cell<Option<[f32; 3]>> = const { Cell::new(None) };
}

pub(super) fn check(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, check: &str) {
    match check {
        "map-drag-node" => drag_from_node(window, cx),
        "map-wheel" => wheel_at_node(window, cx),
        "map-viewport-settled" => {
            settle(window, cx);
            RECORDED.set(Some(viewport(window, cx)));
        }
        "map-viewport-held" => assert_eq!(
            Some(viewport(window, cx)),
            RECORDED.get(),
            "The map viewport moved"
        ),
        "map-viewport-moved" => {
            let now = viewport(window, cx);
            assert_ne!(Some(now), RECORDED.get(), "The map viewport did not move");
            RECORDED.set(Some(now));
        }
        other => panic!("unknown map gesture check {other}"),
    }
}

pub(super) fn inspector(
    window: &gpui_kit::Window,
    cx: &gpui_kit::App,
) -> gpui_kit::Entity<kubecove_gpui::Inspector> {
    gpui_kit::base::Root::read(window, cx)
        .view()
        .clone()
        .downcast::<kubecove_gpui::Inspector>()
        .expect("the Inspector is the window's content")
}

/// The viewport in use as `[x, y, zoom]`, as the last rendered frame showed it.
fn viewport(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) -> [f32; 3] {
    window.render_frame(cx);
    inspector(window, cx)
        .read(cx)
        .map_viewport_in_use()
        .expect("the map has a viewport")
}

/// Ends a running map transition. The web driver's clicks and waits let the
/// transition finish first; this clock moves only when a step advances it.
pub(super) fn settle(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    inspector(window, cx).update(cx, |inspector, cx| inspector.finish_map_transition(cx));
    window.render_frame(cx);
}

/// The map's resource nodes, in drawing order.
fn nodes(window: &gpui_kit::Window) -> Vec<gpui_kit::test::ElementSnapshot> {
    window
        .elements()
        .into_iter()
        .filter(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:")))
        .collect()
}

fn first_node(window: &gpui_kit::Window) -> ElementId {
    nodes(window)
        .first()
        .map(locate::id)
        .expect("the map shows a node")
}

/// The node that shows the selection, as the web's `data-selected` card does.
fn selected_node(window: &gpui_kit::Window) -> Option<ElementId> {
    nodes(window)
        .iter()
        .find(|element| element.selected() == Some(true))
        .map(locate::id)
}

fn drag_from_node(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    reveal(window, "page", "map-viewport", cx);
    // A press and release without movement still selects. Select the first node that way, then
    // drag from another node: the drag must pan and must leave that selection alone.
    let seeded = first_node(window);
    window.click(seeded.clone(), cx);
    settle(window, cx);
    assert_eq!(
        selected_node(window),
        Some(seeded.clone()),
        "A click on a node did not select it"
    );
    // The selection opens its detail and fits the map again, which can move the map.
    reveal(window, "page", "map-viewport", cx);
    let area = window.find("map-viewport").bounds();
    let height = window.viewport_size().height;
    let offset = |point: gpui_kit::Point<gpui_kit::Pixels>| {
        gpui_kit::point(point.x + px(80.), point.y + px(60.))
    };
    // The whole drag stays inside the map and the window, so it lands on the map.
    let inside =
        |point: gpui_kit::Point<gpui_kit::Pixels>| area.contains(&point) && point.y < height;
    let node = nodes(window)
        .iter()
        .filter(|element| {
            let center = element.bounds().center();
            inside(center) && inside(offset(center))
        })
        .map(locate::id)
        .find(|id| *id != seeded)
        .expect("the map shows a second node with room to drag");
    let before = window.find(node.clone()).bounds();
    let start = before.center();
    let end = offset(start);
    // A frame renders between events, as in the app, so each move lands on the node where it now sits.
    window.dispatch_event(
        MouseMoveEvent {
            position: start,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position: start,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    for step in 1..=8 {
        let fraction = step as f32 / 8.;
        let position = gpui_kit::point(
            start.x + (end.x - start.x) * fraction,
            start.y + (end.y - start.y) * fraction,
        );
        window.dispatch_event(
            MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
    // Measure the pan before the release: a click on release selects the node and re-lays out the graph.
    let panned = window.find(node.clone()).bounds();
    assert!(
        panned.origin.x > before.origin.x && panned.origin.y > before.origin.y,
        "Drag from a node did not pan the map: node {before:?} became {panned:?}"
    );
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position: end,
            modifiers: Modifiers::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    assert_eq!(
        selected_node(window),
        Some(seeded),
        "Dragging from a node changed the selection"
    );
}

fn wheel_at_node(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    reveal(window, "page", "map-viewport", cx);
    let node = first_node(window);
    let before = window.find(node.clone()).bounds();
    let center = before.center();
    window.pointer_move(center, None, cx);
    // Positive y is a wheel turn up, the same direction as the web's deltaY of -100.
    window.dispatch_event(
        gpui_kit::ScrollEvent {
            position: center,
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(WHEEL_PIXELS))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    let after = window.find(node).bounds();
    let step = 2f32.powf(WHEEL_PIXELS * 0.002);
    assert!(
        (f32::from(after.size.width) - f32::from(before.size.width) * step).abs()
            <= LAYOUT_ROUNDING,
        "Wheel zoom did not apply one wheel step"
    );
    let moved = after.center();
    assert!(
        (f32::from(moved.x) - f32::from(center.x)).abs() <= LAYOUT_ROUNDING
            && (f32::from(moved.y) - f32::from(center.y)).abs() <= LAYOUT_ROUNDING,
        "Wheel zoom was not anchored at the cursor"
    );
}
