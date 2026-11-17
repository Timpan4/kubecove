//! CSS timing for motion ported from the web client, and a value that moves to
//! its target the way a CSS transition does. Callers read time from the
//! executor clock, so test drivers move it.
use gpui_kit::{Animation, base::animation::cubic_bezier};
use std::time::{Duration, Instant};

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
