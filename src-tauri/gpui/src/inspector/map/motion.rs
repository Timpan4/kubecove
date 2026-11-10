//! The viewport transition between fits. `OwnershipMapViewport.svelte` hands each
//! new fit to xyflow's `setViewport` with a duration, and xyflow runs it as a
//! d3-zoom transition (`setTransform` in `node_modules/@xyflow/system/dist/esm/index.js`):
//! d3-zoom's `schedule` in `node_modules/d3-zoom/src/zoom.js` moves along
//! `interpolateZoom` from `node_modules/d3-interpolate/src/zoom.js`, eased by
//! xyflow's `defaultEase`, a copy of d3-ease's `cubicInOut`.
//!
//! A selection change animates. Other fit changes, such as a resize or new data,
//! show the new fit at once, unless a transition still runs: the new fit then
//! becomes its target from where the map is, as a second `setViewport` interrupts
//! the first on the web. Time comes from the executor clock, so test drivers
//! move it.
use super::Inspector;
use crate::topology::{self, Viewport};
use gpui_kit::{Context, SharedString, Window};
use std::time::{Duration, Instant};

/// `FIT_VIEW_DURATION_MS` in `src/features/resources/OwnershipMapViewport.svelte`.
const FIT_DURATION: Duration = Duration::from_millis(260);
/// `SELECTED_FIT_VIEW_DURATION_MS` in the same file, for a fit to a selected path.
const SELECTED_FIT_DURATION: Duration = Duration::from_millis(120);

/// d3-ease's `cubicInOut`, which xyflow's `defaultEase` copies.
fn ease(t: f64) -> f64 {
    let t = t * 2.;
    if t <= 1. {
        t * t * t / 2.
    } else {
        let t = t - 2.;
        (t * t * t + 2.) / 2.
    }
}

/// `interpolateZoom`'s `rho` and its powers, with d3's default `rho` of √2.
const RHO: f64 = std::f64::consts::SQRT_2;
const RHO2: f64 = 2.;
const RHO4: f64 = 4.;
/// `interpolateZoom`'s `epsilon2`: below it the two centers coincide.
const EPSILON2: f64 = 1e-12;

/// `interpolateZoom` between the views `[ux, uy, w]` that d3-zoom's `schedule`
/// builds: the flow point under the viewport's center and the flow length that
/// the viewport's longer side spans.
#[derive(Clone, Copy)]
struct ZoomPath {
    center: (f64, f64),
    side: f64,
    start: [f64; 3],
    delta: (f64, f64),
    shape: Shape,
}

#[derive(Clone, Copy)]
enum Shape {
    /// The centers coincide, so only the zoom changes.
    Scale {
        s: f64,
    },
    General {
        d1: f64,
        r0: f64,
        s: f64,
    },
}

impl ZoomPath {
    fn new(from: Viewport, to: Viewport, (width, height): (f32, f32)) -> Self {
        let center = (f64::from(width) / 2., f64::from(height) / 2.);
        let side = f64::from(width.max(height));
        let view = |viewport: Viewport| {
            let k = f64::from(viewport.zoom);
            [
                (center.0 - f64::from(viewport.x)) / k,
                (center.1 - f64::from(viewport.y)) / k,
                side / k,
            ]
        };
        let [ux0, uy0, w0] = view(from);
        let [ux1, uy1, w1] = view(to);
        let (dx, dy) = (ux1 - ux0, uy1 - uy0);
        let d2 = dx * dx + dy * dy;
        let shape = if d2 < EPSILON2 {
            Shape::Scale {
                s: (w1 / w0).ln() / RHO,
            }
        } else {
            let d1 = d2.sqrt();
            let b0 = (w1 * w1 - w0 * w0 + RHO4 * d2) / (2. * w0 * RHO2 * d1);
            let b1 = (w1 * w1 - w0 * w0 - RHO4 * d2) / (2. * w1 * RHO2 * d1);
            let r0 = ((b0 * b0 + 1.).sqrt() - b0).ln();
            let r1 = ((b1 * b1 + 1.).sqrt() - b1).ln();
            Shape::General {
                d1,
                r0,
                s: (r1 - r0) / RHO,
            }
        };
        Self {
            center,
            side,
            start: [ux0, uy0, w0],
            delta: (dx, dy),
            shape,
        }
    }

    /// The viewport at eased progress `t`, as `schedule`'s tween turns the view back
    /// into a transform.
    fn at(&self, t: f64) -> Viewport {
        let [ux0, uy0, w0] = self.start;
        let (dx, dy) = self.delta;
        let (ux, uy, w) = match self.shape {
            Shape::Scale { s } => (ux0 + t * dx, uy0 + t * dy, w0 * (RHO * t * s).exp()),
            Shape::General { d1, r0, s } => {
                let s = t * s;
                let u = w0 / (RHO2 * d1) * (r0.cosh() * (RHO * s + r0).tanh() - r0.sinh());
                (
                    ux0 + u * dx,
                    uy0 + u * dy,
                    w0 * r0.cosh() / (RHO * s + r0).cosh(),
                )
            }
        };
        let k = self.side / w;
        Viewport {
            x: (self.center.0 - ux * k) as f32,
            y: (self.center.1 - uy * k) as f32,
            zoom: k as f32,
        }
    }
}

struct Transition {
    path: ZoomPath,
    from: Viewport,
    to: Viewport,
    start: Instant,
    duration: Duration,
}

impl Transition {
    fn running_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) < self.duration
    }

    /// d3-transition eases `elapsed / duration`, and once `duration` has passed the
    /// tween sets exactly the target.
    fn at(&self, now: Instant) -> Viewport {
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= self.duration {
            return self.to;
        }
        // The path starts at `from`; returning it avoids float rounding there.
        if elapsed.is_zero() {
            return self.from;
        }
        self.path
            .at(ease(elapsed.as_secs_f64() / self.duration.as_secs_f64()))
    }
}

#[derive(Default)]
pub(super) struct Motion {
    /// This frame's time.
    now: Option<Instant>,
    /// The fit and the selected node of the last frame.
    fit: Option<Viewport>,
    selected: Option<SharedString>,
    transition: Option<Transition>,
}

impl Motion {
    /// Where the map is on its way to `fit` this frame, or `fit` itself.
    fn viewport(&self, fit: Viewport) -> Viewport {
        match (&self.transition, self.now) {
            (Some(transition), Some(now)) if transition.to == fit => transition.at(now),
            _ => fit,
        }
    }

    /// The fit a running transition moves to.
    pub(super) fn target(&self) -> Option<Viewport> {
        self.transition
            .as_ref()
            .filter(|_| self.running())
            .map(|transition| transition.to)
    }

    pub(super) fn running(&self) -> bool {
        matches!((&self.transition, self.now), (Some(transition), Some(now)) if transition.running_at(now))
    }

    /// Ends a running transition, as a pan, zoom or fit interrupts it on the web.
    pub(super) fn stop(&mut self) {
        self.transition = None;
    }
}

impl Inspector {
    /// The fitted viewport and the viewport in use, which a pan, a zoom or a
    /// running transition may have moved.
    pub(super) fn map_viewports(
        &self,
        view: &topology::View,
    ) -> (Option<Viewport>, Option<Viewport>) {
        let (width, height) = self.map.size.get();
        let fit = topology::fit(&view.nodes, width, height);
        let current = match (self.map.adjusted, fit) {
            (Some((base, current)), Some(fit)) if base == fit => Some(current),
            _ => fit.map(|fit| self.map.motion.viewport(fit)),
        };
        (fit, current)
    }

    /// Starts, retargets or ends the viewport transition for this frame, and asks
    /// for the next frame while one runs. Runs after `rebuild_map`, before the map
    /// renders.
    pub(in crate::inspector) fn sync_map_motion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let now = cx.background_executor().now();
        let size = self.map.size.get();
        let view = self.map.cache.clone();
        let fit = view
            .as_ref()
            .and_then(|view| topology::fit(&view.nodes, size.0, size.1));
        let selected = view
            .as_ref()
            .and_then(|view| view.nodes.iter().find(|node| node.selected))
            .map(|node| super::viewport::node_id(&node.item, &self.map));
        // `buildFlowTopologyFitPlan`'s `focused`: the fit covers a selected path.
        let focused = view.as_ref().is_some_and(|view| {
            view.nodes
                .iter()
                .any(|node| node.selected || node.connected)
        });
        // What the last frame showed, which a new transition starts from, as a new
        // d3 transition starts from the transform the last one set.
        let shown = self.map.motion.fit.map(|fit| match self.map.adjusted {
            Some((base, current)) if base == fit => current,
            _ => self.map.motion.viewport(fit),
        });
        let open = self.map_is_open();
        let motion = &mut self.map.motion;
        let running = motion
            .transition
            .as_ref()
            .is_some_and(|transition| transition.running_at(now));
        motion.now = Some(now);
        if fit != motion.fit {
            let animate = open && !cx.reduce_motion() && (selected != motion.selected || running);
            let duration = if focused {
                SELECTED_FIT_DURATION
            } else {
                FIT_DURATION
            };
            motion.transition = match (shown, fit) {
                (Some(from), Some(to)) if animate && from != to => Some(Transition {
                    path: ZoomPath::new(from, to, size),
                    from,
                    to,
                    start: now,
                    duration,
                }),
                _ => None,
            };
            motion.fit = fit;
            // A new fit replaces a pan or zoom, as `setViewport` does on the web.
            self.map.adjusted = None;
        } else if self.map.adjusted.is_some_and(|(base, _)| Some(base) == fit) {
            motion.transition = None;
        }
        motion.selected = selected;
        if motion.running() {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        } else {
            motion.transition = None;
        }
    }

    /// The viewport in use as `[x, y, zoom]`, for UI tests that follow a transition.
    #[doc(hidden)]
    pub fn map_viewport_in_use(&self) -> Option<[f32; 3]> {
        let view = self.map.cache.as_ref()?;
        let viewport = self.map_viewports(view).1?;
        Some([viewport.x, viewport.y, viewport.zoom])
    }

    /// Ends a running transition at its target, as waiting out its duration does.
    /// UI test drivers call this where the web driver waits for the map to settle.
    #[doc(hidden)]
    pub fn finish_map_transition(&mut self, cx: &mut Context<Self>) {
        self.map.motion.stop();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The shared `coverage-map.selection-pan` parity scenario covers that a
    // selection animates instead of jumping, and that it settles. It cannot see
    // the path between the endpoints. Ways that path could break:
    // - an easing other than `cubicInOut`, such as linear or cubic-in;
    // - the general `interpolateZoom` branch transcribed wrong, or about a point
    //   other than the viewport center, or with `w` from the shorter side;
    // - the same-center branch, which d3 takes when only the zoom changes;
    // - a long pan that should zoom out on the way, as `interpolateZoom` does;
    // - a transition that misses its target or keeps moving after `duration`.
    // Expected values come from the installed `d3-interpolate` and `d3-ease`,
    // through d3-zoom's `schedule` tween, at raw progress 0.25, 0.5 and 0.75.

    const SIZE: (f32, f32) = (800., 500.);
    // The result is f32: a few of its ulps at the largest coordinate below, 2,897.
    const TOLERANCE: f64 = 1e-3;

    fn viewport(x: f32, y: f32, zoom: f32) -> Viewport {
        Viewport { x, y, zoom }
    }

    /// Compares with `[x, y, zoom]` as JavaScript printed them.
    fn check(from: Viewport, to: Viewport, expected: [[f64; 3]; 3]) {
        let path = ZoomPath::new(from, to, SIZE);
        for (raw, [x, y, zoom]) in [0.25, 0.5, 0.75].into_iter().zip(expected) {
            let actual = path.at(ease(raw));
            assert!(
                (f64::from(actual.x) - x).abs() < TOLERANCE
                    && (f64::from(actual.y) - y).abs() < TOLERANCE
                    && (f64::from(actual.zoom) - zoom).abs() < TOLERANCE / 100.,
                "at {raw}: {actual:?} is not {x}, {y}, {zoom}"
            );
        }
    }

    #[test]
    fn eases_with_cubic_in_out() {
        assert_eq!(ease(0.), 0.);
        assert_eq!(ease(0.25), 0.0625);
        assert_eq!(ease(0.5), 0.5);
        assert_eq!(ease(0.75), 0.9375);
        assert_eq!(ease(1.), 1.);
    }

    #[test]
    fn pans_and_zooms_along_interpolate_zoom() {
        check(
            viewport(120., 40., 0.8),
            viewport(-300., 150., 1.25),
            [
                [101.4300074426996, 49.9706509380822, 0.8108839239051265],
                [-52.70108871221851, 106.7986352032778, 0.9469767672041306],
                [-263.7655139264167, 145.61709701094597, 1.2021648396577511],
            ],
        );
    }

    #[test]
    fn zooms_about_a_shared_center() {
        check(
            viewport(100., 50., 1.),
            viewport(-200., -150., 2.),
            [
                [86.71786527177585, 41.14524351451726, 1.0442737824274138],
                // JavaScript printed the zoom as 1.414213562373095, which is √2.
                [
                    -24.264068711928473,
                    -32.84271247461896,
                    std::f64::consts::SQRT_2,
                ],
                [-174.56196841914414, -133.0413122794294, 1.915206561397147],
            ],
        );
    }

    #[test]
    fn zooms_out_on_a_long_pan() {
        check(
            viewport(0., 0., 1.),
            viewport(-4000., 0., 1.),
            [
                [75.14242708767864, 61.334272722098575, 0.7546629091116057],
                [-70.67872433164075, 200.97096621545415, 0.19611613513818335],
                [-2897.524390823386, 61.33427272209917, 0.7546629091116033],
            ],
        );
    }

    #[test]
    fn starts_at_its_origin_and_ends_on_its_target() {
        let (from, to) = (viewport(120., 40., 0.8), viewport(-300., 150., 1.25));
        let start = Instant::now();
        let transition = Transition {
            path: ZoomPath::new(from, to, SIZE),
            from,
            to,
            start,
            duration: SELECTED_FIT_DURATION,
        };
        assert_eq!(transition.at(start), from);
        assert!(transition.running_at(start + SELECTED_FIT_DURATION / 2));
        assert_eq!(transition.at(start + SELECTED_FIT_DURATION), to);
        assert!(!transition.running_at(start + SELECTED_FIT_DURATION));
        assert_eq!(transition.at(start + FIT_DURATION), to);
    }
}
