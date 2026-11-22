//! Long row lists inside an ancestor scroller, such as the page or the detail
//! panel body, which `list` cannot own because it needs its own fixed height.
//! A row renders when it intersects the scroller's clip, grown by the log
//! list's overdraw, or when its height is unknown; the rows between rendered
//! ones collapse into spacers of their measured height, so layout matches
//! rendering every row. Heights are measured at prepaint and cached by a row
//! id that the caller derives from everything that sets the row's height.

use super::{clipped::Clipped, timeline::RAIL_EXTENSION};
use gpui_kit::{
    AnyElement, App, AvailableSpace, Edges, EntityId, Window, canvas, div, prelude::*, px,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

/// The overdraw `logs_state.rs` gives the log list.
const OVERDRAW: f32 = 400.;

/// The id of a row whose height follows from `parts`.
pub(super) fn row_id(parts: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    parts.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Default)]
pub(super) struct Windowed {
    slots: Rc<RefCell<Vec<Slot>>>,
    unwrapped: bool,
}

#[derive(Default)]
struct Slot {
    /// The width the heights were measured at.
    width: f32,
    heights: HashMap<u64, f32>,
    /// Window-space list top and clip at the last prepaint.
    band: Option<(f32, f32, f32)>,
    scope: Option<u64>,
    scroll: f32,
}

/// Preserve the scroll extent with spacers between rendered rows.
fn row_children(
    tops: &[f32],
    shown: impl Fn(usize) -> bool,
    mut render: impl FnMut(usize) -> AnyElement,
) -> (Vec<Option<usize>>, Vec<AnyElement>) {
    let mut plan: Vec<Option<usize>> = Vec::new();
    let mut children: Vec<AnyElement> = Vec::new();
    let mut skipped: Option<usize> = None;
    for index in 0..tops.len() - 1 {
        if shown(index) {
            if let Some(start) = skipped.take() {
                plan.push(None);
                children.push(
                    div()
                        .flex_none()
                        .h(px(tops[index] - tops[start]))
                        .into_any_element(),
                );
            }
            plan.push(Some(index));
            children.push(render(index));
        } else if skipped.is_none() {
            skipped = Some(index);
        }
    }
    if let Some(start) = skipped {
        plan.push(None);
        children.push(
            div()
                .flex_none()
                .h(px(tops[tops.len() - 1] - tops[start]))
                .into_any_element(),
        );
    }
    (plan, children)
}

/// Row tops from the cached heights; unknown heights count as zero.
fn row_tops(heights: &HashMap<u64, f32>, ids: &[u64]) -> Vec<f32> {
    let mut tops = Vec::with_capacity(ids.len() + 1);
    let mut top = 0.;
    tops.push(top);
    for id in ids {
        top += heights.get(id).copied().unwrap_or(0.);
        tops.push(top);
    }
    tops
}

impl Windowed {
    /// Fixed table geometry is independent of width. Use it to lay out the
    /// complete scroll extent before choosing rows from this frame's clip.
    pub(super) fn fixed_tops(&self, slot: usize, ids: &[u64]) -> Option<Vec<f32>> {
        if !self.unwrapped {
            return None;
        }
        let slots = self.slots.borrow();
        let slot = slots.get(slot)?;
        ids.iter()
            .all(|id| slot.heights.contains_key(id))
            .then(|| row_tops(&slot.heights, ids))
    }

    pub(super) fn fixed_rows(
        tops: Vec<f32>,
        mut render: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> gpui_kit::Div {
        let height = *tops.last().expect("row tops include zero");
        let rows = canvas(
            move |bounds, window, cx| {
                let clip = window.content_mask().bounds;
                let visible = clip.size.width > px(0.) && clip.size.height > px(0.);
                let start = f32::from(clip.top() - bounds.top());
                let end = f32::from(clip.bottom() - bounds.top());
                let (_, children) = row_children(
                    &tops,
                    |i| visible && tops[i + 1] >= start && tops[i] <= end,
                    |i| render(i, window, cx),
                );
                let mut child = div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .children(children)
                    .into_any_element();
                child.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
                child.prepaint_at(bounds.origin, window, cx);
                child
            },
            |_, mut child, window, cx| child.paint(window, cx),
        );
        div()
            .flex()
            .flex_col()
            .flex_none()
            .h(px(height))
            .child(rows.size_full())
    }

    /// Retain geometry only for the current timeline's day groups.
    pub(super) fn truncate(&self, count: usize) {
        self.slots.borrow_mut().truncate(count);
    }

    /// Reopening the same content can reuse its geometry. Translate its last
    /// window position by the ancestor's scroll change before choosing rows.
    pub(super) fn scrolled_rows(
        &self,
        slot: usize,
        ids: Vec<u64>,
        scope: u64,
        scroll: f32,
        entity: EntityId,
        render: impl FnMut(usize) -> AnyElement,
    ) -> gpui_kit::Div {
        let mut slots = self.slots.borrow_mut();
        if slots.len() <= slot {
            slots.resize_with(slot + 1, Slot::default);
        }
        let state = &mut slots[slot];
        if state.scope != Some(scope) {
            *state = Slot {
                scope: Some(scope),
                ..Default::default()
            };
        }
        // The immutable scope includes every row and both expansion states.
        // Retain their geometry until the scope or measured width changes.
        if let Some((top, _, _)) = &mut state.band {
            *top += scroll - state.scroll;
        }
        state.scroll = scroll;
        drop(slots);
        self.rows(slot, ids, entity, render)
    }

    /// Table rows never wrap. Their measured heights also hold after a width change.
    pub(super) fn unwrapped() -> Self {
        Self {
            slots: Rc::default(),
            unwrapped: true,
        }
    }

    /// The rows of list `slot` (one per independent list sharing this state),
    /// stacked with no gap. `render(index)` builds row `index`.
    pub(super) fn rows(
        &self,
        slot: usize,
        ids: Vec<u64>,
        entity: EntityId,
        mut render: impl FnMut(usize) -> AnyElement,
    ) -> gpui_kit::Div {
        let mut slots = self.slots.borrow_mut();
        if slots.len() <= slot {
            slots.resize_with(slot + 1, Slot::default);
        }
        let state = &slots[slot];
        let tops = row_tops(&state.heights, &ids);
        let band = state.band.map(|(top, clip_top, clip_bottom)| {
            (clip_top - top - OVERDRAW, clip_bottom - top + OVERDRAW)
        });
        let shown = |index: usize| {
            !state.heights.contains_key(&ids[index])
                || band.is_some_and(|(start, end)| tops[index + 1] >= start && tops[index] <= end)
        };
        let (plan, children) = row_children(&tops, shown, |index| {
            Clipped::new(
                render(index),
                Edges {
                    bottom: px(RAIL_EXTENSION),
                    ..Default::default()
                },
            )
            .into_any_element()
        });
        drop(slots);
        let state = self.slots.clone();
        let unwrapped = self.unwrapped;
        div()
            .flex()
            .flex_col()
            .children(children)
            .on_children_prepainted(move |bounds, window: &mut Window, _| {
                let Some(first) = bounds.first() else {
                    return;
                };
                let mut slots = state.borrow_mut();
                let state = &mut slots[slot];
                let width = f32::from(first.size.width);
                let resized = !unwrapped && state.width != width;
                if resized {
                    state.width = width;
                    state.heights.clear();
                }
                let heights = &mut state.heights;
                for (child, bounds) in plan.iter().zip(&bounds) {
                    if let Some(index) = child {
                        heights.insert(ids[*index], f32::from(bounds.size.height));
                    }
                }
                let top = f32::from(first.origin.y);
                let clip = window.content_mask().bounds;
                let (clip_top, clip_bottom) = (f32::from(clip.top()), f32::from(clip.bottom()));
                state.band = Some((top, clip_top, clip_bottom));
                // Redraw when a row in view did not render this frame.
                let tops = row_tops(heights, &ids);
                let rendered: Vec<bool> = {
                    let mut rendered = vec![false; ids.len()];
                    for index in plan.iter().flatten() {
                        rendered[*index] = true;
                    }
                    rendered
                };
                let missing = resized
                    || (0..ids.len()).any(|index| {
                        !rendered[index]
                            && top + tops[index + 1] >= clip_top
                            && top + tops[index] <= clip_bottom
                    });
                if missing {
                    window.on_next_frame(move |_, cx| cx.notify(entity));
                }
            })
    }
}
