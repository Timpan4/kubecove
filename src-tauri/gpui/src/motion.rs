//! CSS timing for motion ported from the web client, a value that moves to
//! its target the way a CSS transition does, and the clock of motion that
//! repeats. Time comes from the executor clock, so test drivers move it.
use gpui_kit::{Animation, App, EntityId, Global, Task, Window, base::animation::cubic_bezier};
use std::time::{Duration, Instant};

/// The clock of motion that repeats for as long as a state lasts, such as a
/// spinner. A frame renders the whole view and a pending state has no fixed
/// end, so this motion runs only in the active window, and each step wakes
/// from a timer instead of asking for every frame. One wake is pending at a
/// time, for every moving element.
#[derive(Default)]
struct Repeating {
    /// The first use. Spinners count their turn from it, so all are in step.
    epoch: Option<Instant>,
    /// When the pending wake fires, and the wake.
    wake: Option<(Instant, Task<()>)>,
    /// When an element last asked for a wake.
    asked: Option<Instant>,
    /// The views the pending wake renders.
    views: Vec<EntityId>,
}

impl Global for Repeating {}

/// Whether repeating motion runs in `window`: it rests while another window is
/// active and under reduced motion.
pub fn repeats(window: &Window, cx: &App) -> bool {
    window.is_window_active() && !cx.reduce_motion()
}

/// Renders the view being rendered again `step` from now, unless a wake is
/// already pending by then. `step` is the time the caller's motion takes to
/// move one device pixel, so no frame repeats the one before it.
pub fn wake_in(step: Duration, window: &Window, cx: &mut App) {
    let now = cx.background_executor().now();
    let at = now + step;
    let view = window.current_view();
    let state = cx.default_global::<Repeating>();
    state.asked = Some(now);
    if !state.views.contains(&view) {
        state.views.push(view);
    }
    if state
        .wake
        .as_ref()
        .is_some_and(|(pending, _)| *pending > now && *pending <= at)
    {
        return;
    }
    let task = cx.spawn(async move |cx| {
        cx.background_executor().timer(step).await;
        cx.update(|cx| {
            for view in std::mem::take(&mut cx.global_mut::<Repeating>().views) {
                cx.notify(view);
            }
        });
    });
    cx.global_mut::<Repeating>().wake = Some((at, task));
}

/// The share of `period` that has passed on the clock every spinner shares.
pub fn shared_phase(period: Duration, cx: &mut App) -> f32 {
    let now = cx.background_executor().now();
    let epoch = *cx.default_global::<Repeating>().epoch.get_or_insert(now);
    phase(now.saturating_duration_since(epoch), period)
}

/// The share of `period` that `elapsed` has reached in its current run.
pub fn phase(elapsed: Duration, period: Duration) -> f32 {
    (elapsed.as_secs_f64() % period.as_secs_f64() / period.as_secs_f64()) as f32
}

/// Whether a frame rendered at the clock's current time asked for a wake. The
/// wake is a timer and asks for no frame, so UI tests ask here. A wake that an
/// earlier frame armed still fires once after its motion stopped, and does not
/// count.
#[doc(hidden)]
pub fn wake_pending(cx: &App) -> bool {
    let now = cx.background_executor().now();
    cx.try_global::<Repeating>()
        .is_some_and(|state| state.asked == Some(now) && state.wake.is_some())
}

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function.
#[derive(Clone, Copy)]
pub struct Bezier(f32, f32, f32, f32);

impl Bezier {
    /// The CSS keyword `ease`.
    pub const EASE: Self = Self(0.25, 0.1, 0.25, 1.);
    /// The CSS keyword `ease-in-out`.
    pub const EASE_IN_OUT: Self = Self(0.42, 0., 0.58, 1.);
    /// The CSS keyword `ease-out`.
    pub const EASE_OUT: Self = Self(0., 0., 0.58, 1.);

    /// The eased progress at `progress` of the duration, both from 0 to 1.
    pub fn ease(self, progress: f32) -> f32 {
        cubic_bezier(self.0, self.1, self.2, self.3)(progress)
    }
}

/// A duration and its easing.
#[derive(Clone, Copy)]
pub struct Timing {
    pub duration: Duration,
    pub easing: Bezier,
}

impl Timing {
    /// A Tailwind `transition-*` utility without a duration or easing class, from
    /// `node_modules/tailwindcss/theme.css`: `--default-transition-duration: 150ms`
    /// and `--default-transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1)`.
    pub const TRANSITION: Self = Self {
        duration: Duration::from_millis(150),
        easing: Bezier(0.4, 0., 0.2, 1.),
    };

    /// `animate-in fade-in duration-150`: tw-animate-css runs it with `ease`.
    pub const FADE_IN: Self = Self {
        duration: Duration::from_millis(150),
        easing: Bezier::EASE,
    };

    /// The eased progress `elapsed` after the start, from 0 to 1.
    pub fn progress(self, elapsed: Duration) -> f32 {
        if elapsed >= self.duration {
            return 1.;
        }
        self.easing
            .ease(elapsed.as_secs_f32() / self.duration.as_secs_f32())
    }

    /// A one-shot GPUI animation with this timing. It renders its end state
    /// under reduced motion.
    pub fn animation(self) -> Animation {
        Animation::new(self.duration).with_easing(move |progress| self.easing.ease(progress))
    }
}

/// A value on its way to a target. A new target starts from the value shown,
/// as a CSS transition restarts from the computed value.
#[derive(Clone, Copy)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
}

impl Tween {
    /// A value already at `to`.
    pub fn at(to: f32, now: Instant) -> Self {
        Self {
            from: to,
            to,
            start: now,
        }
    }

    pub fn value(&self, now: Instant, timing: Timing) -> f32 {
        let elapsed = now.saturating_duration_since(self.start);
        if self.from == self.to || elapsed >= timing.duration {
            return self.to;
        }
        let progress = elapsed.as_secs_f32() / timing.duration.as_secs_f32();
        self.from + (self.to - self.from) * timing.easing.ease(progress)
    }

    /// Whether the value still moves at `now`.
    pub fn running(&self, now: Instant, timing: Timing) -> bool {
        self.from != self.to && now.saturating_duration_since(self.start) < timing.duration
    }

    /// Moves to `to` from the value shown at `now`. The same target changes nothing.
    pub fn retarget(&mut self, to: f32, now: Instant, timing: Timing) {
        if to != self.to {
            *self = Self {
                from: self.value(now, timing),
                to,
                start: now,
            };
        }
    }
}
