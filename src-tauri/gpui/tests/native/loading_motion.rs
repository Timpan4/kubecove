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
//! - the strip is still drawn, or a frame still requested, after the fade-out;
//! - a pending read asks for every frame for as long as it lasts, which keeps
//!   the frames harness drawing, instead of arming the repeating clock's timer;
//! - the bar slides while another application's window is active, or stays
//!   still after the window returns.
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

/// How the last frame arranged the next: a fade asks for the next frame, and
/// the slide arms the repeating clock's timer.
#[derive(Debug, PartialEq)]
struct Next {
    frame: bool,
    timer: bool,
}

const REST: Next = Next {
    frame: false,
    timer: false,
};

/// The strip's opacity and the bar's offset in bar widths, and how the frame
/// that drew them arranged the next.
fn bar_next(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
) -> (Option<(f32, f32)>, Next) {
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
        let next = Next {
            frame: window.simulate_next_frame(cx) > 0,
            timer: Inspector::repeating_motion(cx),
        };
        (drawn, next)
    })
    .unwrap()
}

/// The strip's opacity and the bar's offset in bar widths, and whether the
/// frame that drew them arranged another.
fn bar(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> (Option<(f32, f32)>, bool) {
    let (drawn, next) = bar_next(cx, handle);
    (drawn, next != REST)
}

fn activate(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
}

/// Starts a resource read for the other context that stays pending until the
/// returned sender fires.
fn pending_read(
    cx: &mut TestAppContext,
    backend: &Controlled,
    handle: gpui_kit::AnyWindowHandle,
) -> tokio::sync::oneshot::Sender<()> {
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    send
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < TOLERANCE
}

#[gpui_kit::test]
fn the_foreground_bar_fades_and_slides_until_its_fade_out_ends(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    activate(cx, handle);
    assert_eq!(bar(cx, handle).0, None);
    let send = pending_read(cx, &backend, handle);
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
    let (drawn, next) = bar_next(cx, handle);
    let (opacity, offset) = drawn.expect("the strip shows while its read is pending");
    assert_eq!(opacity, 1.);
    assert!(close(offset, 1.4), "the 55 % keyframe drew {offset}");
    // The read has no fixed end, so its slide wakes from the timer alone.
    assert_eq!(
        next,
        Next {
            frame: false,
            timer: true
        }
    );

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
    assert_eq!(bar_next(cx, handle), (None, REST));
}

#[gpui_kit::test]
fn the_bar_rests_in_an_inactive_window_and_slides_when_it_returns(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    // The test window opens inactive.
    let handle = open(cx, backend.clone());
    let _send = pending_read(cx, &backend, handle);
    // The show delay, then the fade-in.
    advance(cx, Duration::from_millis(300));
    assert_eq!(bar(cx, handle).0, Some((0., 0.)));
    advance(cx, Duration::from_millis(150));
    assert_eq!(
        bar_next(cx, handle),
        (Some((1., 0.)), REST),
        "a still bar at the strip's start"
    );

    activate(cx, handle);
    let (drawn, next) = bar_next(cx, handle);
    assert_eq!(drawn, Some((1., -1.1)), "the slide starts");
    assert!(next.timer);

    gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
    // A later frame, so the step the active window's frame armed does not count.
    advance(cx, Duration::from_millis(75));
    assert_eq!(bar_next(cx, handle), (Some((1., 0.)), REST));
}
