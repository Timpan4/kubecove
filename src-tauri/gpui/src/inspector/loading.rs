//! Show delay and minimum visible time for loading indicators, matching
//! `src/lib/loading-delay.svelte.ts`.
//!
//! Reads start at once. An indicator appears only while its read is still
//! pending `SHOW_DELAY` after it started, then stays for at least
//! `MIN_VISIBLE`, and finished content waits until that time is up. Render code
//! asks for each indicator's phase; the frame then schedules one one-shot timer
//! for the earliest change, so waiting never redraws continuously.
use super::Inspector;
use gpui_kit::{Context, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The user chose this value.
pub const SHOW_DELAY: Duration = Duration::from_millis(300);
/// The user chose this value.
pub const MIN_VISIBLE: Duration = Duration::from_millis(300);

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
    finished: Option<Instant>,
}

impl Episode {
    /// The phase at `now` and the instant it next changes.
    fn phase(self, now: Instant) -> (Phase, Option<Instant>) {
        let shown_at = self.started + SHOW_DELAY;
        let hidden_at = shown_at + MIN_VISIBLE;
        match self.finished {
            None if now < shown_at => (Phase::Waiting, Some(shown_at)),
            None => (Phase::Shown, None),
            Some(finished) if finished < shown_at || now >= hidden_at => (Phase::Ready, None),
            Some(_) => (Phase::Shown, Some(hidden_at)),
        }
    }
}

#[derive(Default)]
struct Frame {
    now: Option<Instant>,
    /// Episodes and whether this frame observed them.
    episodes: HashMap<String, (Episode, bool)>,
    next: Option<Instant>,
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
        let mut frame = self.frame.borrow_mut();
        let Some(now) = frame.now else {
            return if pending {
                Phase::Waiting
            } else {
                Phase::Ready
            };
        };
        let previous = frame.episodes.get(key).map(|(episode, _)| *episode);
        let episode = match (previous, pending) {
            (None, false) => return Phase::Ready,
            (None, true) => Episode {
                started: now,
                finished: None,
            },
            // A new read during the minimum visible time keeps the indicator.
            (Some(episode), true) if episode.finished.is_some() => {
                if episode.phase(now).0.shown() {
                    Episode {
                        finished: None,
                        ..episode
                    }
                } else {
                    Episode {
                        started: now,
                        finished: None,
                    }
                }
            }
            (Some(episode), false) if episode.finished.is_none() => Episode {
                finished: Some(now),
                ..episode
            },
            (Some(episode), _) => episode,
        };
        let (phase, next) = episode.phase(now);
        if phase.ready() {
            frame.episodes.remove(key);
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

    pub(super) fn begin_frame(&mut self, now: Instant) {
        let frame = self.frame.get_mut();
        frame.now = Some(now);
        frame.next = None;
        for (_, seen) in frame.episodes.values_mut() {
            *seen = false;
        }
    }

    /// Forgets indicators this frame did not render, so a later read starts a
    /// fresh delay, and arms one timer for the earliest phase change.
    pub(super) fn end_frame(&mut self, cx: &mut Context<Inspector>) {
        let frame = self.frame.get_mut();
        frame.episodes.retain(|_, (_, seen)| *seen);
        let (Some(now), Some(next)) = (frame.now, frame.next) else {
            return;
        };
        if self.wake.as_ref().is_some_and(|(at, _)| *at == next) {
            return;
        }
        let delay = next.saturating_duration_since(now);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        });
        self.wake = Some((next, task));
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
        delays.begin_frame(now);
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
        delays.begin_frame(start + Duration::from_secs(5));
        delays.frame.get_mut().episodes.retain(|_, (_, seen)| *seen);
        let later = start + Duration::from_secs(10);
        assert_eq!(frame(&mut delays, later, true), Phase::Waiting);
    }
}
