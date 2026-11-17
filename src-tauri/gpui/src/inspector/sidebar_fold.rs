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
    /// A closing run's height in pixels, measured on its first frame. A child
    /// that opens inside the branch then cannot grow it while it closes. An
    /// opening run follows its rows' height instead, which only grows it, so
    /// it ends at the height its rows reach.
    height: Option<f32>,
    /// The pixels the branch showed on the last frame.
    shown: Option<f32>,
    /// The pixels the branch showed when the user turned it back. The run's
    /// first frame moves `from` to them, so the branch turns from where it is.
    turned: Option<f32>,
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
        let (from, turned) = tracks
            .get(path)
            .map_or((if open { 0. } else { 1. }, None), |track| {
                (track.at(now).0, track.shown)
            });
        tracks.insert(
            path.to_owned(),
            Track {
                open,
                from,
                start: now,
                height: None,
                shown: None,
                turned,
                seen: true,
            },
        );
    }

    /// The pixels the folding branch at `path` shows at `now`, whose rows lay
    /// out `measured` high this frame.
    fn reached(&self, path: &str, measured: f32, now: Instant) -> f32 {
        let mut tracks = self.tracks.borrow_mut();
        let Some(track) = tracks.get_mut(path) else {
            return measured;
        };
        if let Some(turned) = track.turned.take()
            && measured > 0.
        {
            track.from = (turned / measured).clamp(0., 1.);
        }
        let height = if track.open {
            measured
        } else {
            *track.height.get_or_insert(measured)
        };
        // Whole pixels, so the rows below the branch stay on the pixel grid.
        let shown = (height * track.at(now).0).round();
        track.shown = Some(shown);
        shown
    }

    /// Whether the branch at `path` is folding.
    pub(super) fn folding(&self, path: &str) -> bool {
        self.tracks.borrow().contains_key(path)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.tracks.borrow().is_empty()
    }

    /// Whether a branch of the namespace tree is folding, which needs every
    /// namespace row built. The other branches key their folds with a `<kind>:`
    /// prefix, such as `gitops:`, and no namespace or tree path holds a colon.
    pub(super) fn tree_folding(&self) -> bool {
        self.tracks.borrow().keys().any(|path| !path.contains(':'))
    }

    /// How the branch at `path` shows its rows this frame: `None` when it is
    /// closed and its fold has ended, so the caller builds no rows for it.
    /// Tests read a running fold's share under `fold:<path>`.
    pub(super) fn branch(&self, path: &str, open: bool, cx: &mut App) -> Option<Fold<'_>> {
        let mut tracks = self.tracks.borrow_mut();
        let settled = open.then_some(Fold(None));
        let Some(track) = tracks.get_mut(path) else {
            return settled;
        };
        let now = cx.background_executor().now();
        let (share, running) = track.at(now);
        // Reduced motion turned on during a fold ends it, as it does any transition.
        if track.open != open || !running || Moving::finishing(cx) || cx.reduce_motion() {
            tracks.remove(path);
            return settled;
        }
        track.seen = true;
        Moving::record(&ElementId::Name(format!("fold:{path}").into()), share, cx);
        Some(Fold(Some(Running {
            now,
            path: path.to_owned(),
            folds: self,
        })))
    }

    /// Forgets folds whose branch did not render, and reports whether a fold
    /// needs another frame.
    pub(super) fn end_frame(&self) -> bool {
        let mut tracks = self.tracks.borrow_mut();
        tracks.retain(|_, track| std::mem::take(&mut track.seen));
        !tracks.is_empty()
    }
}

/// A branch that shows rows this frame, and its run if it is folding.
pub(super) struct Fold<'a>(Option<Running<'a>>);

struct Running<'a> {
    /// The frame's time.
    now: Instant,
    path: String,
    folds: &'a Folds,
}

impl Fold<'_> {
    /// The branch's `rows` cut to the height the fold has reached. `shown` is
    /// the height a row shows, which an inner fold may already have cut.
    pub(super) fn clip<T>(self, mut rows: Vec<T>, shown: impl Fn(&mut T) -> &mut f32) -> Vec<T> {
        let Some(run) = self.0 else {
            return rows;
        };
        let measured: f32 = rows.iter_mut().map(|row| *shown(row)).sum();
        let mut left = run.folds.reached(&run.path, measured, run.now);
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

#[cfg(test)]
mod tests {
    //! Failure modes of a fold with a child folding inside it:
    //! - a closing branch sized from its rows on every frame grows while a
    //!   child opens, so the rows below it move down before they move up;
    //! - a branch turned back mid-fold jumps at the turn, or reopens to a
    //!   height measured before its child opened and then jumps at the end.
    use super::*;
    use gpui_kit::TestAppContext;

    /// `sidebar_tree::ROW_HEIGHT`.
    const ROW: f32 = 26.;
    /// Ten-millisecond frames, so the first frames of a run are seen.
    const FRAMES: u32 = 15;

    /// The height a namespace branch, toggled to `open`, shows this frame:
    /// four group rows, the first of which, Workloads, holds seven kind rows.
    fn namespace(folds: &Folds, open: bool, cx: &mut TestAppContext) -> f32 {
        cx.update(|cx| {
            let kinds = folds
                .branch("ns/Workloads", true, cx)
                .map_or(Vec::new(), |fold| fold.clip(vec![ROW; 7], |row| row));
            let mut rows = vec![ROW];
            rows.extend(kinds);
            rows.extend([ROW; 3]);
            let shown = folds
                .branch("ns", open, cx)
                .map_or(0., |fold| fold.clip(rows, |row| row).iter().sum());
            folds.end_frame();
            shown
        })
    }

    /// Opens the namespace's Workloads and closes the namespace in one frame.
    fn close_while_a_child_opens(folds: &Folds, cx: &mut TestAppContext) {
        cx.update(|cx| {
            folds.start("ns/Workloads", true, cx);
            folds.start("ns", false, cx);
        });
    }

    fn frame(cx: &mut TestAppContext) {
        cx.background_executor
            .advance_clock(control_motion::DURATION / FRAMES);
    }

    #[gpui_kit::test]
    fn a_closing_branch_does_not_grow_while_a_child_opens(cx: &mut TestAppContext) {
        let folds = Folds::default();
        close_while_a_child_opens(&folds, cx);
        let mut last = namespace(&folds, false, cx);
        assert_eq!(last, 4. * ROW);
        for _ in 0..FRAMES {
            frame(cx);
            let shown = namespace(&folds, false, cx);
            assert!(
                shown <= last,
                "the closing branch grew from {last} to {shown}"
            );
            last = shown;
        }
        assert_eq!(last, 0.);
    }

    #[gpui_kit::test]
    fn a_branch_turned_back_while_a_child_opens_moves_without_a_jump(cx: &mut TestAppContext) {
        let folds = Folds::default();
        close_while_a_child_opens(&folds, cx);
        let mut last = namespace(&folds, false, cx);
        for _ in 0..FRAMES / 2 {
            frame(cx);
            last = namespace(&folds, false, cx);
        }
        cx.update(|cx| folds.start("ns", true, cx));
        assert_eq!(
            namespace(&folds, true, cx),
            last,
            "the branch jumped at the turn"
        );
        let mut steps = Vec::new();
        while folds.folding("ns") {
            frame(cx);
            let shown = namespace(&folds, true, cx);
            assert!(
                shown >= last,
                "the opening branch shrank from {last} to {shown}"
            );
            steps.push(shown - last);
            last = shown;
        }
        assert_eq!(last, 11. * ROW);
        // The frame the fold ends on moves the rows below no further than a
        // frame of the run did.
        let (end, run) = steps.split_last().unwrap();
        let largest = run.iter().copied().fold(0., f32::max);
        assert!(
            *end <= largest,
            "the branch jumped {end} px as its fold ended"
        );
    }
}
