//! Failure modes, written before these tests:
//!
//! 1. An overlay opens fully drawn, so its enter never shows.
//! 2. A closed overlay leaves the tree at once, so its exit never shows, or it
//!    never leaves.
//! 3. An overlay that closes part way through its enter jumps to fully drawn
//!    before it fades.
//! 4. The drawer's backdrop takes the drawer's 200 ms instead of its own
//!    100 ms, or the drawer slides in from the wrong side.
//! 5. A popover slides toward its trigger instead of from it.
//! 6. Reduced motion still animates.
//! 7. The detail panel replays its enter when its content changes.
//! 8. A closing overlay, or the container around it, takes a click meant for
//!    what is behind it.
//! 9. A closing overlay keeps focus, so a key press runs its handlers.
//!
//! The web has this motion only for popovers ("Overlay enter and exit timing"
//! in `e2e/parity/mock-timing.md`), so the shared scenarios cannot check the
//! rest. Opacities are the CSS curves at the given time, as in
//! `e2e/parity/scenarios/overlay-motion.json`.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{App, AppContext, TestAppContext, Window, px, size};
use kubecove_gpui::Inspector;
use std::{sync::Arc, time::Duration};

type Handle = gpui_kit::AnyWindowHandle;

/// gpui-base's `cubic_bezier` tests bound its distance from CSS at 5e-4.
const OPACITY_TOLERANCE: f32 = 6e-4;
/// Painted bounds land on whole pixels in this window, whose scale is 1.
const PIXEL: f32 = 0.5;

const ROW: &str = "resource:Deployment:payments:payments-api:mock-dev";

fn act(cx: &mut TestAppContext, handle: Handle, action: impl FnOnce(&mut Window, &mut App)) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        action(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn advance(cx: &mut TestAppContext, ms: u64) {
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

/// Draws a frame and reads the opacity it gave an overlay.
fn opacity(cx: &mut TestAppContext, handle: Handle, overlay: &str) -> Option<f32> {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        gpui_kit::base::Root::read(window, cx)
            .view()
            .clone()
            .downcast::<Inspector>()
            .expect("the Inspector is the window's content")
            .read(cx)
            .overlay_opacity(overlay)
    })
    .unwrap()
}

#[track_caller]
fn assert_opacity(cx: &mut TestAppContext, handle: Handle, overlay: &str, expected: f32) {
    let drawn = opacity(cx, handle, overlay).unwrap_or_else(|| panic!("{overlay} is not drawn"));
    assert!(
        (drawn - expected).abs() <= OPACITY_TOLERANCE,
        "{overlay} drew at opacity {drawn}, not {expected}"
    );
}

/// Draws a frame and reads where it put an element: its left and top edge.
fn origin(cx: &mut TestAppContext, handle: Handle, id: &'static str) -> (f32, f32) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find(id).bounds();
        (f32::from(bounds.left()), f32::from(bounds.top()))
    })
    .unwrap()
}

fn drawn(cx: &mut TestAppContext, handle: Handle, id: &'static str) -> bool {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

#[gpui_kit::test]
fn a_dialog_fades_in_and_out_over_100_ms(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| window.click("open-palette", cx));
    assert_opacity(cx, handle, "palette", 0.);
    advance(cx, 50);
    assert_opacity(cx, handle, "palette", 0.8024);
    advance(cx, 50);
    assert_opacity(cx, handle, "palette", 1.);

    act(cx, handle, |window, cx| window.press("escape", cx));
    assert_opacity(cx, handle, "palette", 1.);
    assert!(drawn(cx, handle, "palette-dialog"));
    advance(cx, 50);
    assert_opacity(cx, handle, "palette", 0.1976);
    advance(cx, 50);
    assert_eq!(opacity(cx, handle, "palette"), None);
    assert!(!drawn(cx, handle, "palette-dialog"));
}

#[gpui_kit::test]
fn a_dialog_closed_during_its_enter_fades_from_where_it_was(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| window.click("open-palette", cx));
    assert_opacity(cx, handle, "palette", 0.);
    advance(cx, 50);
    act(cx, handle, |window, cx| window.press("escape", cx));
    assert_opacity(cx, handle, "palette", 0.8024);
    // The exit covers the share that was drawn in the same share of 100 ms.
    advance(cx, 81);
    assert_eq!(opacity(cx, handle, "palette"), None);
}

#[gpui_kit::test]
fn a_click_during_an_exit_reaches_what_is_behind_the_overlay(cx: &mut TestAppContext) {
    let handle = super::argo::open_argo(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| {
        window.click("argo-connect-open", cx)
    });
    advance(cx, 100);
    act(cx, handle, |window, cx| window.press("escape", cx));
    // The dialog's backdrop covers the window and stops a press while it is open.
    assert!(drawn(cx, handle, "argo-connection-dialog"));
    act(cx, handle, |window, cx| window.click("close-detail", cx));
    assert!(drawn(cx, handle, "argo-connection-dialog"));
    assert!(!drawn(cx, handle, "close-detail"));
}

#[gpui_kit::test]
fn a_click_during_the_drawer_exit_reaches_the_page(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.simulate_window_resize(handle, size(px(1000.), px(800.)));
    cx.run_until_parked();
    act(cx, handle, |window, cx| window.click("open-navigation", cx));
    advance(cx, 200);
    act(cx, handle, |window, cx| {
        window.click("navigation-backdrop", cx)
    });
    // The drawer's container covers the window and takes every press while it is open.
    assert!(drawn(cx, handle, "navigation-panel"));
    act(cx, handle, |window, cx| window.click("kinds-picker", cx));
    assert!(drawn(cx, handle, "navigation-panel"));
    assert!(drawn(cx, handle, "kind-search"));
}

#[gpui_kit::test]
fn a_key_press_during_an_exit_does_not_reach_the_overlay(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| window.click("open-palette", cx));
    advance(cx, 100);
    act(cx, handle, |window, cx| window.press("escape", cx));
    assert!(drawn(cx, handle, "palette-dialog"));
    // Enter in an open palette runs its first result, which leaves this page.
    act(cx, handle, |window, cx| window.press("enter", cx));
    advance(cx, 100);
    assert!(drawn(cx, handle, "kinds-picker"));
}

#[gpui_kit::test]
fn the_drawer_slides_40_px_while_its_backdrop_fades_in_100_ms(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    // Below `sidebar::COMPACT_WIDTH` the sidebar becomes the drawer.
    cx.simulate_window_resize(handle, size(px(1000.), px(800.)));
    cx.run_until_parked();
    act(cx, handle, |window, cx| window.click("open-navigation", cx));
    assert_opacity(cx, handle, "navigation-drawer", 0.);
    assert_opacity(cx, handle, "navigation-backdrop", 0.);
    assert_eq!(origin(cx, handle, "navigation-panel").0, -40.);
    advance(cx, 100);
    assert_opacity(cx, handle, "navigation-drawer", 0.7756);
    assert_opacity(cx, handle, "navigation-backdrop", 1.);
    let left = origin(cx, handle, "navigation-panel").0;
    assert!((left + 8.9775).abs() <= PIXEL, "the drawer drew at {left}");
    advance(cx, 100);
    assert_opacity(cx, handle, "navigation-drawer", 1.);
    assert_eq!(origin(cx, handle, "navigation-panel").0, 0.);

    act(cx, handle, |window, cx| {
        window.click("navigation-backdrop", cx)
    });
    advance(cx, 100);
    assert_opacity(cx, handle, "navigation-drawer", 0.2244);
    assert_eq!(opacity(cx, handle, "navigation-backdrop"), None);
    let left = origin(cx, handle, "navigation-panel").0;
    assert!((left + 31.0225).abs() <= PIXEL, "the drawer drew at {left}");
    advance(cx, 100);
    assert_eq!(opacity(cx, handle, "navigation-drawer"), None);
    assert!(!drawn(cx, handle, "navigation-panel"));
}

#[gpui_kit::test]
fn a_popover_comes_8_px_from_its_trigger(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| window.click("kinds-picker", cx));
    assert_opacity(cx, handle, "kind-picker", 0.);
    let start = origin(cx, handle, "kind-search").1;
    advance(cx, 150);
    assert_opacity(cx, handle, "kind-picker", 1.);
    let rest = origin(cx, handle, "kind-search").1;
    // The picker opens below its trigger, so it starts nearer to it.
    assert_eq!(rest - start, 8.);

    act(cx, handle, |window, cx| window.click("kinds-picker", cx));
    advance(cx, 75);
    assert_opacity(cx, handle, "kind-picker", 0.1976);
    assert!(origin(cx, handle, "kind-search").1 < rest);
    advance(cx, 75);
    assert_eq!(opacity(cx, handle, "kind-picker"), None);
    assert!(!drawn(cx, handle, "kind-search"));
}

#[gpui_kit::test]
fn reduced_motion_opens_and_closes_overlays_at_once(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update(|cx| cx.set_reduce_motion(true));
    act(cx, handle, |window, cx| window.click("open-palette", cx));
    assert_opacity(cx, handle, "palette", 1.);
    act(cx, handle, |window, cx| window.press("escape", cx));
    assert_eq!(opacity(cx, handle, "palette"), None);
    assert!(!drawn(cx, handle, "palette-dialog"));
}

#[gpui_kit::test]
fn the_detail_panel_enters_once(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    act(cx, handle, |window, cx| window.click(ROW, cx));
    assert_opacity(cx, handle, "detail", 0.);
    let start = origin(cx, handle, "close-detail").0;
    advance(cx, 75);
    assert_opacity(cx, handle, "detail", 0.8024);
    advance(cx, 75);
    assert_opacity(cx, handle, "detail", 1.);
    // The panel comes from the window's right edge.
    assert_eq!(start - origin(cx, handle, "close-detail").0, 8.);

    act(cx, handle, |window, cx| window.click("show-yaml", cx));
    assert_opacity(cx, handle, "detail", 1.);
}
