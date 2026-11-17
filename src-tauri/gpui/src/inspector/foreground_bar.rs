//! `ForegroundLoadingBar.svelte`: a 2 px bar across the top while a foreground
//! read runs. The web counts `withForegroundLoad` calls. Native derives the
//! same state from the resource list, topology and detail reads it owns.
//!
//! The strip fades in and out, and a third-width bar slides across it until
//! the fade-out ends, as on the web. Time comes from the executor clock, so
//! test drivers move it.
//!
//! The fades end by themselves and ask for every frame. A read can stay
//! pending without a fixed end, so the slide follows the repeating clock in
//! `motion.rs`: it wakes from a timer, and the bar rests at the strip's start
//! in an inactive window.
use super::{Inspector, Load, Page, detail::Detail, map::TopologyLoad};
use crate::motion::{self, Bezier, Timing, Tween};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use gpui_kit::{Context, IntoElement, Window, div, prelude::*, px, relative};
use std::time::{Duration, Instant};

/// The strip's `transition-opacity`, which sets no duration or easing class.
const FADE: Timing = Timing::TRANSITION;
/// `.foreground-loading-bar` in `src/App.css`: `1.15s ease-in-out infinite`.
const SLIDE_PERIOD: Duration = Duration::from_millis(1150);
const SLIDE_EASING: Bezier = Bezier::EASE_IN_OUT;
/// The `foreground-loading-bar` keyframes: the share of a period and the
/// bar's `translateX` in bar widths. CSS eases each interval on its own.
const SLIDE: [(f32, f32); 3] = [(0., -1.1), (0.55, 1.4), (1., 2.6)];
/// `w-1/3`: the bar's share of the strip.
const BAR_WIDTH: f32 = 1. / 3.;

/// The bar's `translateX` in bar widths at `turn` of a period.
fn slide(turn: f32) -> f32 {
    let (from, to) = if turn < SLIDE[1].0 {
        (SLIDE[0], SLIDE[1])
    } else {
        (SLIDE[1], SLIDE[2])
    };
    from.1 + (to.1 - from.1) * SLIDE_EASING.ease((turn - from.0) / (to.0 - from.0))
}

#[derive(Default)]
pub(super) struct ForegroundBar {
    /// The strip's opacity while it shows, on its way to 1 or back to 0.
    opacity: Option<Tween>,
    /// The first frame of the current run of the slide.
    epoch: Option<Instant>,
    /// What the last frame drew: the opacity and the bar's offset in bar widths.
    shown: Option<(f32, f32)>,
}

impl Inspector {
    fn foreground_loading(&self) -> bool {
        let list = matches!(self.page, Page::Resources | Page::ClusterOverview)
            && matches!(*self.observation.status(), Load::Loading);
        let topology = matches!(self.map.topology, TopologyLoad::Loading)
            && matches!(self.page, Page::Resources | Page::ClusterOverview);
        let detail = matches!(self.detail, Detail::Loading(_));
        list || topology || detail
    }

    /// Absolutely positioned over the shell's top padding, so the layout does
    /// not move when the bar appears.
    pub(super) fn foreground_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let active = self
            .loading
            .phase("foreground", self.foreground_loading())
            .shown();
        let now = cx.background_executor().now();
        let target = if active { 1. } else { 0. };
        let bar = &mut self.foreground;
        // Reduced motion shows and hides the still bar at once, as the web's
        // `animation: none` leaves it at the strip's start.
        let frame = if cx.reduce_motion() {
            bar.opacity = None;
            bar.epoch = None;
            active.then_some((1., 0.))
        } else {
            if active || bar.opacity.is_some() {
                bar.opacity
                    .get_or_insert(Tween::at(0., now))
                    .retarget(target, now, FADE);
            }
            match bar.opacity {
                Some(opacity) if active || opacity.running(now, FADE) => {
                    if opacity.running(now, FADE) {
                        cx.on_next_frame(window, |_, _, cx| cx.notify());
                    }
                    let offset = if motion::repeats(window, cx) {
                        let elapsed = now.saturating_duration_since(*bar.epoch.get_or_insert(now));
                        // The bar's mean time to move one device pixel.
                        let travel = (SLIDE[2].1 - SLIDE[0].1)
                            * BAR_WIDTH
                            * f32::from(window.viewport_size().width)
                            * window.scale_factor();
                        motion::wake_in(SLIDE_PERIOD.div_f32(travel.max(1.)), window, cx);
                        slide(motion::phase(elapsed, SLIDE_PERIOD))
                    } else {
                        bar.epoch = None;
                        0.
                    };
                    Some((opacity.value(now, FADE), offset))
                }
                _ => {
                    bar.opacity = None;
                    bar.epoch = None;
                    None
                }
            }
        };
        bar.shown = frame;
        div()
            .id("foreground-loading-bar")
            .test_support()
            .aria_description(if active { "active" } else { "idle" })
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(2.))
            .overflow_hidden()
            .when_some(frame, |strip, (opacity, offset)| {
                strip.opacity(opacity).child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(offset * BAR_WIDTH))
                        .w(relative(BAR_WIDTH))
                        .rounded_full()
                        .bg(Tokens::primary()),
                )
            })
    }

    /// The strip's opacity and the bar's offset in bar widths as the last frame
    /// drew them, or None while the strip is hidden. The bar is painted inside a
    /// clipped strip, so UI tests ask here.
    #[doc(hidden)]
    pub fn foreground_bar_motion(&self) -> Option<(f32, f32)> {
        self.foreground.shown
    }

    /// Whether the last frame moved the bar along its slide.
    #[doc(hidden)]
    pub fn foreground_bar_sliding(&self) -> bool {
        self.foreground.epoch.is_some()
    }
}
