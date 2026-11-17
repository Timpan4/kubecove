//! Overlay enter and exit motion (`src/inspector/overlay_motion.rs`).
//!
//! The motion reads the executor clock, which moves only when a step advances
//! it. `holdOverlayMotion` hands the clock to the scenario and
//! `advanceOverlayMotion` moves it; `runner.rs` does both, because the clock
//! cannot advance inside the window update where step handlers run. In every
//! other scenario `settle` ends each run before a step acts, where the web
//! driver's clicks and waits let it finish.

use super::*;

/// The scenario gives opacities to four decimals, and gpui-base's
/// `cubic_bezier` tests bound its distance from CSS at 5e-4.
const OPACITY_TOLERANCE: f64 = 6e-4;

fn inspector(
    window: &gpui_kit::Window,
    cx: &gpui_kit::App,
) -> gpui_kit::Entity<kubecove_gpui::Inspector> {
    gpui_kit::base::Root::read(window, cx)
        .view()
        .clone()
        .downcast::<kubecove_gpui::Inspector>()
        .expect("the Inspector is the window's content")
}

/// Ends running overlay motion at its target and draws the result.
pub(super) fn settle(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    let moved =
        inspector(window, cx).update(cx, |inspector, cx| inspector.finish_overlay_motion(cx));
    if moved {
        window.render_frame(cx);
    }
}

/// An element inside each named overlay's surface.
fn surface(overlay: &str) -> &'static str {
    match overlay {
        "update-status" => "header-update-check",
        other => panic!("unknown overlay: {other}"),
    }
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "holdOverlayMotion" | "advanceOverlayMotion" => {}
        "expectNoOverlay" => {
            window.render_frame(cx);
            let overlay = string(step, "overlay");
            let drawn = inspector(window, cx).read(cx).overlay_opacity(overlay);
            assert_eq!(drawn, None, "{step}");
            assert!(window.try_find(surface(overlay)).is_none(), "{step}");
        }
        "expectOverlayOpacity" => {
            window.render_frame(cx);
            let overlay = string(step, "overlay");
            let drawn = inspector(window, cx)
                .read(cx)
                .overlay_opacity(overlay)
                .unwrap_or_else(|| panic!("{overlay} is not drawn: {step}"));
            let expected = step["opacity"].as_f64().expect("an opacity");
            assert!(
                (f64::from(drawn) - expected).abs() <= OPACITY_TOLERANCE,
                "{overlay} drew at opacity {drawn}: {step}"
            );
            assert!(window.try_find(surface(overlay)).is_some(), "{step}");
        }
        _ => return false,
    }
    true
}
