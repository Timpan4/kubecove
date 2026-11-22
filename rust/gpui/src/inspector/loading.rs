//! Show delay and minimum visible time for loading indicators, matching
//! `src/lib/loading-delay.svelte.ts`.
//!
//! Reads start at once. An indicator appears only while its read is still
//! pending `SHOW_DELAY` after it started, then stays for at least
//! `MIN_VISIBLE`, and finished content waits until that time is up. Render code
//! asks for each indicator's phase; the frame then schedules one one-shot timer
//! for the earliest change, so waiting never redraws continuously.
use super::Inspector;
use crate::motion::Timing;
use gpui_kit::{Context, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The user chose this value.
pub const SHOW_DELAY: Duration = Duration::from_millis(300);
/// The user chose this value.
pub const MIN_VISIBLE: Duration = Duration::from_millis(300);
/// How content fades in after a shown indicator: the web's
/// `animate-in fade-in duration-150`.
const REVEAL: Timing = Timing::FADE_IN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing is pending or held: render the content.
    Ready,
    /// A read is pending but the show delay has not passed: render neither.
    Waiting,
    /// Render the indicator instead of the content.
    Shown,
}

impl Phase {
    pub fn shown(self) -> bool {
        self == Self::Shown
    }

    pub fn ready(self) -> bool {
        self == Self::Ready
    }
}

/// One pending read, from its first pending render to its first settled render.
#[derive(Clone, Copy)]
struct Episode {
    started: Instant,
    /// The first frame that rendered the indicator. The minimum visible time
    /// counts from here, as the web counts from its show timer, so a late
    /// first frame still shows the indicator for the whole time.
    shown: Option<Instant>,
    finished: Option<Instant>,
}

impl Episode {
    fn new(started: Instant) -> Self {
        Self {
            started,
            shown: None,
            finished: None,
        }
    }

    /// The phase at `now` and the instant it next changes.
    fn phase(self, now: Instant, show_delay: Duration) -> (Phase, Option<Instant>) {
        let shown_at = self.started + show_delay;
        match (self.shown, self.finished) {
            // Settled before any frame rendered its indicator.
            (None, Some(_)) => (Phase::Ready, None),
            (None, None) if now < shown_at => (Phase::Waiting, Some(shown_at)),
            (_, None) => (Phase::Shown, None),
            (Some(shown), Some(_)) => {
                let hidden_at = shown + MIN_VISIBLE;
                if now >= hidden_at {
                    (Phase::Ready, None)
                } else {
                    (Phase::Shown, Some(hidden_at))
                }
            }
        }
    }
}

#[derive(Default)]
struct Frame {
    now: Option<Instant>,
    reduce_motion: bool,
    /// Episodes and whether this frame observed them.
    episodes: HashMap<String, (Episode, bool)>,
    next: Option<Instant>,
    /// When content replaced each indicator that had shown.
    reveals: HashMap<String, Instant>,
    /// Whether this frame drew an indicator or content that still fades in.
    fading: bool,
}

#[derive(Default)]
pub struct LoadingDelays {
    frame: RefCell<Frame>,
    wake: Option<(Instant, Task<()>)>,
}

impl LoadingDelays {
    /// Returns what the indicator `key` renders this frame. `pending` is true
    /// while the read it reports has not settled. Keys name one indicator,
    /// such as `"resources"` or `"sidebar-namespace-kinds:default"`.
    pub fn phase(&self, key: &str, pending: bool) -> Phase {
        self.phase_after(key, pending, SHOW_DELAY)
    }

    pub fn phase_after(&self, key: &str, pending: bool, show_delay: Duration) -> Phase {
        let mut frame = self.frame.borrow_mut();
        let Some(now) = frame.now else {
            return if pending {
                Phase::Waiting
            } else {
                Phase::Ready
            };
        };
        let previous = frame.episodes.get(key).map(|(episode, _)| *episode);
        let mut episode = match (previous, pending) {
            (None, false) => return Phase::Ready,
            (None, true) => Episode::new(now),
            // A new read during the minimum visible time keeps the indicator.
            (Some(episode), true) if episode.finished.is_some() => {
                if episode.phase(now, show_delay).0.shown() {
                    Episode {
                        finished: None,
                        ..episode
                    }
                } else {
                    Episode::new(now)
                }
            }
            (Some(episode), false) if episode.finished.is_none() => Episode {
                finished: Some(now),
                ..episode
            },
            (Some(episode), _) => episode,
        };
        let (phase, next) = episode.phase(now, show_delay);
        if phase.shown() && episode.shown.is_none() {
            episode.shown = Some(now);
        }
        if phase.ready() {
            frame.episodes.remove(key);
            if episode.shown.is_some() {
                frame.reveals.insert(key.to_owned(), now);
            }
        } else if let Some(entry) = frame.episodes.get_mut(key) {
            *entry = (episode, true);
        } else {
            frame.episodes.insert(key.to_owned(), (episode, true));
        }
        if let Some(next) = next {
            frame.next = Some(frame.next.map_or(next, |current| current.min(next)));
        }
        phase
    }

    /// Ends a settled episode's hold when another indicator stays visible.
    pub fn release_hold(&self, key: &str) {
        let mut frame = self.frame.borrow_mut();
        if frame
            .episodes
            .get(key)
            .is_some_and(|(episode, _)| episode.finished.is_some())
        {
            frame.episodes.remove(key);
        }
    }

    /// Whether the indicator `key` rendered this frame in place of its
    /// content: its read is pending, or the indicator is held for its minimum
    /// visible time. Ready episodes are removed as they settle.
    pub fn holds(&self, key: &str) -> bool {
        self.frame.borrow().episodes.contains_key(key)
    }

    /// The opacity of the content that replaced the indicator `key`, while it
    /// fades in. Content fades only after an indicator that had shown: a read
    /// that settles within the show delay swaps at once, and so does every
    /// read under reduced motion. Call it after `phase` for the same key. The
    /// web swaps at once; the user chose this fade for this client on 2026-11-17.
    pub fn reveal(&self, key: &str) -> Option<f32> {
        let mut frame = self.frame.borrow_mut();
        let elapsed = frame
            .now?
            .saturating_duration_since(*frame.reveals.get(key)?);
        if frame.reduce_motion || elapsed >= REVEAL.duration {
            return None;
        }
        frame.fading = true;
        Some(REVEAL.progress(elapsed))
    }

    pub(super) fn begin_frame(&mut self, now: Instant, reduce_motion: bool) {
        let frame = self.frame.get_mut();
        frame.now = Some(now);
        frame.reduce_motion = reduce_motion;
        frame.fading = false;
        frame
            .reveals
            .retain(|_, since| now < *since + REVEAL.duration);
        frame.next = None;
        for (_, seen) in frame.episodes.values_mut() {
            *seen = false;
        }
    }

    /// Forgets indicators this frame did not render, so a later read starts a
    /// fresh delay, and arms one timer for the earliest phase change. Returns
    /// whether a fade this frame drew is still running.
    pub(super) fn end_frame(&mut self, cx: &mut Context<Inspector>) -> bool {
        let frame = self.frame.get_mut();
        frame.episodes.retain(|_, (_, seen)| *seen);
        let fading = frame.fading;
        let (Some(now), Some(next)) = (frame.now, frame.next) else {
            return fading;
        };
        if self.wake.as_ref().is_some_and(|(at, _)| *at == next) {
            return fading;
        }
        let delay = next.saturating_duration_since(now);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        });
        self.wake = Some((next, task));
        fading
    }
}

impl Inspector {
    /// The opacity the last frame drew the content that replaced the indicator
    /// `key` with, or None when it does not fade. Opacity is painted, so UI
    /// tests ask here.
    #[doc(hidden)]
    pub fn content_reveal(&self, key: &str) -> Option<f32> {
        self.loading.reveal(key)
    }

    /// Whether a frame rendered at the clock's current time arranged the next
    /// step of a spinner or of the foreground bar's slide. That wake is a
    /// timer and asks for no frame, so UI tests ask here. To judge one frame,
    /// call `forget_repeating_motion` before rendering it.
    #[doc(hidden)]
    pub fn repeating_motion(cx: &gpui_kit::App) -> bool {
        crate::motion::wake_pending(cx)
    }

    /// Drops what earlier frames told `repeating_motion`.
    #[doc(hidden)]
    pub fn forget_repeating_motion(cx: &mut gpui_kit::App) {
        crate::motion::forget_ask(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The shared `loading-delay.` parity scenario covers the show delay and the
    // minimum visible time. This covers what it cannot observe: an episode
    // left over from an indicator that stopped rendering must not show a later
    // read's indicator at once.
    fn frame(delays: &mut LoadingDelays, now: Instant, pending: bool) -> Phase {
        delays.begin_frame(now, false);
        let phase = delays.phase("read", pending);
        let frame = delays.frame.get_mut();
        frame.episodes.retain(|_, (_, seen)| *seen);
        phase
    }

    #[test]
    fn unrendered_indicator_restarts_its_delay() {
        let start = Instant::now();
        let mut delays = LoadingDelays::default();
        frame(&mut delays, start, true);
        delays.begin_frame(start + Duration::from_secs(5), false);
        delays.frame.get_mut().episodes.retain(|_, (_, seen)| *seen);
        let later = start + Duration::from_secs(10);
        assert_eq!(frame(&mut delays, later, true), Phase::Waiting);
    }

    // Failure mode: the first frame after the show delay comes late, and the
    // minimum visible time, counted from the scheduled show time, hides the
    // indicator almost as soon as it first draws.
    #[test]
    fn a_late_first_indicator_frame_stays_for_the_minimum_time() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut delays = LoadingDelays::default();
        assert_eq!(frame(&mut delays, at(0), true), Phase::Waiting);
        assert_eq!(frame(&mut delays, at(550), true), Phase::Shown);
        assert_eq!(frame(&mut delays, at(560), false), Phase::Shown);
        assert_eq!(frame(&mut delays, at(849), false), Phase::Shown);
        assert_eq!(frame(&mut delays, at(850), false), Phase::Ready);
    }
}
