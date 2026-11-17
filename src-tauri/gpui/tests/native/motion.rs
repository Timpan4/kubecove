//! Failure modes of control motion that the shared scenarios cannot reach. They
//! see that a control moves and settles, not the path, and the browser driver has
//! no reduced-motion switch:
//! - a control mounts mid-transition instead of settled;
//! - an easing other than Tailwind's default, such as linear;
//! - a control that still asks for frames after its duration;
//! - reduced motion that still passes through the transition.

use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, ElementId, TestAppContext};
use kubecove_gpui::control_motion::{DURATION, Moving};
use std::sync::Arc;

const SWITCH: &str = "setting:Show exact timestamps";
const THUMB: &str = "setting-thumb:Show exact timestamps";
/// Chromium's progress for `cubic-bezier(0.4, 0, 0.2, 1)` at half its duration,
/// from a paused Web Animation. Chromium samples the curve from a table, so the
/// value is exact to about 1e-3.
const HALFWAY: f32 = 0.775_561;

/// The thumb's left edge and the progress of its transition, if it moved in the
/// frame this renders.
fn thumb(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> (f32, Option<f32>) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let moving = ElementId::Name("switch-thumb:Show exact timestamps".into());
        (
            f32::from(window.find(THUMB).bounds().origin.x),
            Moving::progress(&moving, cx),
        )
    })
    .unwrap()
}

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_switch_follows_the_web_curve_and_reduced_motion_skips_it(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    click(cx, handle, "open-settings");
    click(cx, handle, "settings-section:General");
    let (off, progress) = thumb(cx, handle);
    assert_eq!(progress, None, "the switch mounted mid-transition");

    // The first frame after the click still shows the old state.
    click(cx, handle, SWITCH);
    assert_eq!(thumb(cx, handle), (off, Some(0.)));
    cx.background_executor.advance_clock(DURATION / 2);
    let (_, progress) = thumb(cx, handle);
    let progress = progress.expect("the switch moves at half time");
    assert!((progress - HALFWAY).abs() < 1e-3, "{progress}");
    cx.background_executor.advance_clock(DURATION / 2);
    let (on, progress) = thumb(cx, handle);
    // `translate-x-4` in `Switch.svelte`.
    assert_eq!((on - off, progress), (16., None));

    cx.update(|cx| cx.set_reduce_motion(true));
    click(cx, handle, SWITCH);
    assert_eq!(thumb(cx, handle), (off, None));
}
