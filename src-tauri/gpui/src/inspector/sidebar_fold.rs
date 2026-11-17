//! Sidebar branches open and close by height, as the `slide` transition on the
//! child list in `SidebarTreeNode.svelte` does on the web: the rows of a branch
//! the user toggled are revealed from the top over the control transition.
//!
//! The tree stays a flat list of rows. While a branch folds, each of its rows
//! shows the part of its height the fold has reached, and rows past that are
//! left out. Only a toggle starts a fold. Any other change of a branch's state,
//! such as "Collapse all", ends it at once. Time comes from the executor clock,
//! so test drivers move it. A frame asks for the next one only while a fold
//! runs. Reduced motion starts none, and neither does a test driver that asked
//! for `Moving::set_instant_layout`.
use crate::control_motion::{self, Moving};
use gpui_kit::{AnyElement, App, ElementId, div, prelude::*, px};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

/// One run toward open or closed.
struct Track {
    open: bool,
    /// The shown share when the run started: 0 is closed and 1 is open.
    from: f32,
    start: Instant,
    /// Whether this frame rendered the branch.
    seen: bool,
}

impl Track {
    /// The shown share at `now` and whether the run still moves. A run that
    /// starts part way, because the user toggled again, covers the share that
    /// is left in the same share of the duration.
    fn at(&self, now: Instant) -> (f32, bool) {
        let target = if self.open { 1. } else { 0. };
        let run = control_motion::DURATION.mul_f32((target - self.from).abs());
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= run {
            return (target, false);
        }
        let t = elapsed.as_secs_f32() / run.as_secs_f32();
        (
            self.from + (target - self.from) * control_motion::curve(t),
            true,
        )
    }
}

/// The branches that are folding, by path.
#[derive(Default)]
pub(super) struct Folds {
    tracks: RefCell<HashMap<String, Track>>,
}

impl Folds {
    /// Starts the fold of the branch at `path`, which the user toggled to `open`.
    pub(super) fn start(&self, path: &str, open: bool, cx: &App) {
        let mut tracks = self.tracks.borrow_mut();
        if cx.reduce_motion() || Moving::instant_layout(cx) {
            tracks.remove(path);
            return;
        }
        let now = cx.background_executor().now();
        let from = tracks
            .get(path)
            .map_or(if open { 0. } else { 1. }, |track| track.at(now).0);
        tracks.insert(
            path.to_owned(),
            Track {
                open,
                from,
                start: now,
                seen: true,
            },
        );
    }

    /// Whether the branch at `path` is folding.
    pub(super) fn folding(&self, path: &str) -> bool {
        self.tracks.borrow().contains_key(path)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.tracks.borrow().is_empty()
    }

    /// How the branch at `path` shows its rows this frame: `None` when it is
    /// closed and its fold has ended, so the caller builds no rows for it.
    /// Tests read a running fold's share under `fold:<path>`.
    pub(super) fn branch(&self, path: &str, open: bool, cx: &mut App) -> Option<Fold> {
        let mut tracks = self.tracks.borrow_mut();
        let settled = open.then_some(Fold(None));
        let Some(track) = tracks.get_mut(path) else {
            return settled;
        };
        let (share, running) = track.at(cx.background_executor().now());
        if track.open != open || !running || Moving::finishing(cx) {
            tracks.remove(path);
            return settled;
        }
        track.seen = true;
        Moving::record(&ElementId::Name(format!("fold:{path}").into()), share, cx);
        Some(Fold(Some(share)))
    }

    /// Forgets folds whose branch did not render, and reports whether a fold
    /// needs another frame.
    pub(super) fn end_frame(&self) -> bool {
        let mut tracks = self.tracks.borrow_mut();
        tracks.retain(|_, track| std::mem::take(&mut track.seen));
        !tracks.is_empty()
    }
}

/// A branch that shows rows this frame, and the share of them its fold has
/// reached if it is folding.
pub(super) struct Fold(Option<f32>);

impl Fold {
    /// The branch's `rows` cut to the height the fold has reached. `shown` is
    /// the height a row shows, which an inner fold may already have cut.
    pub(super) fn clip<T>(self, mut rows: Vec<T>, shown: impl Fn(&mut T) -> &mut f32) -> Vec<T> {
        let Some(share) = self.0 else {
            return rows;
        };
        let total: f32 = rows.iter_mut().map(|row| *shown(row)).sum();
        // Whole pixels, so the rows below the branch stay on the pixel grid.
        let mut left = (total * share).round();
        rows.retain_mut(|row| {
            let height = shown(row);
            *height = height.min(left);
            left -= *height;
            *height > 0.
        });
        rows
    }
}

/// A sidebar row and the height it shows.
pub(super) struct Row {
    element: AnyElement,
    height: f32,
    pub(super) shown: f32,
}

impl Row {
    /// `element`, which lays out `height` high.
    pub(super) fn new(element: AnyElement, height: f32) -> Self {
        Self {
            element,
            height,
            shown: height,
        }
    }

    pub(super) fn into_element(self) -> AnyElement {
        cut(self.element, self.height, self.shown)
    }
}

/// `row`, which lays out `height` high, showing its top `shown` pixels.
pub(super) fn cut(row: AnyElement, height: f32, shown: f32) -> AnyElement {
    if shown >= height {
        return row;
    }
    div()
        .flex_none()
        .h(px(shown))
        .overflow_hidden()
        .child(row)
        .into_any_element()
}
