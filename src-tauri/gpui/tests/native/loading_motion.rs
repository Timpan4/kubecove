//! The foreground loading bar between its ends. The shared `loading-motion.`
//! parity scenarios cover that the bar slides while a read is pending, rests
//! after its fade-out and stays still under reduced motion. The web runs the
//! fade and the slide on the browser's own clock, so only the native clock can
//! stop inside them. Failure modes, written before the code:
//!
//! - the strip appears or leaves at once instead of fading over 150 ms;
//! - the fade follows another curve than Tailwind's default transition;
//! - the bar starts inside the strip instead of off its left edge, or misses the
//!   140 % keyframe at 55 % of a period;
//! - the bar stops when the read settles, before the fade-out ends;
//! - the strip is still drawn, or a frame still requested, after the fade-out.
use super::*;
use std::time::Duration;

/// `cubic-bezier(0.4, 0, 0.2, 1)` at half its duration, from Chromium's
/// `getComputedTiming().progress`.
const HALF_FADE: f32 = 0.775_561_3;
/// f32 easing against a reference rounded to seven digits.
const TOLERANCE: f32 = 1e-4;

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.background_executor.advance_clock(by);
    cx.run_until_parked();
}

/// The strip's opacity and the bar's offset in bar widths, and whether the
/// frame that drew them asked for another.
fn bar(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> (Option<(f32, f32)>, bool) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        let drawn = gpui_kit::base::Root::read(window, cx)
            .view()
            .clone()
            .downcast::<Inspector>()
            .expect("the Inspector is the window's content")
            .read(cx)
            .foreground_bar_motion();
        (drawn, window.simulate_next_frame(cx) > 0)
    })
    .unwrap()
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < TOLERANCE
}

#[gpui_kit::test]
fn the_foreground_bar_fades_and_slides_until_its_fade_out_ends(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    assert_eq!(bar(cx, handle).0, None);
    // The first read for the other context stays pending.
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(bar(cx, handle).0, None, "the show delay");

    // The show delay in `loading.rs`.
    advance(cx, Duration::from_millis(300));
    let (drawn, moving) = bar(cx, handle);
    assert_eq!(drawn, Some((0., -1.1)), "the fade and the slide start");
    assert!(moving);

    advance(cx, Duration::from_millis(75));
    let (drawn, moving) = bar(cx, handle);
    let (opacity, offset) = drawn.expect("the strip shows while its read is pending");
    assert!(close(opacity, HALF_FADE), "half the fade-in drew {opacity}");
    assert!(offset > -1.1 && offset < 1.4, "the bar moved to {offset}");
    assert!(moving);

    // 55 % of the 1150 ms period after the slide started.
    advance(cx, Duration::from_micros(632_500 - 75_000));
    let (drawn, _) = bar(cx, handle);
    let (opacity, offset) = drawn.expect("the strip shows while its read is pending");
    assert_eq!(opacity, 1.);
    assert!(close(offset, 1.4), "the 55 % keyframe drew {offset}");

    // The indicator has shown for longer than its minimum time, so it leaves
    // on the frame after the read settles.
    send.send(()).unwrap();
    cx.run_until_parked();
    let (drawn, moving) = bar(cx, handle);
    assert_eq!(drawn.map(|(opacity, _)| opacity), Some(1.));
    assert!(moving);

    advance(cx, Duration::from_millis(75));
    let (drawn, moving) = bar(cx, handle);
    let (opacity, later) = drawn.expect("the strip shows until its fade-out ends");
    assert!(
        close(opacity, 1. - HALF_FADE),
        "half the fade-out drew {opacity}"
    );
    assert!(later != offset, "the bar slides during the fade-out");
    assert!(moving);

    advance(cx, Duration::from_millis(75));
    assert_eq!(bar(cx, handle), (None, false));
}
