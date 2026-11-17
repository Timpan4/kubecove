//! A one-time tint on resource table rows that a watch notice added or
//! changed. The web table has no such highlight; the user chose it for this
//! client on 2026-11-17, with the timing of the web sidebar's row reveal. The
//! table compares the page it draws with the page it last drew only on the
//! frame after a notice named rows, so an idle table pays nothing.
use super::Inspector;
use super::resource_observation::Patch;
use super::table_columns::{Column, resource_id, resource_id_of};
use crate::model::{Added, TablePage};
use crate::motion::{Bezier, Timing};
use gpui_kit::Context;
use kubecove_lib::models::ResourceSummary;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// `animate-[sidebar-row-reveal_1s_ease-out]` in `SidebarTreeNode.svelte`.
const HIGHLIGHT: Timing = Timing {
    duration: Duration::from_secs(1),
    easing: Bezier::EASE_OUT,
};
/// The `from` colour of `sidebar-row-reveal` in `App.css`: primary at 30 %.
const TINT: f32 = 0.3;
/// The columns whose text a watch read changes. Age and usage change without one.
const WATCHED: [Column; 4] = [
    Column::Status,
    Column::Ready,
    Column::Restarts,
    Column::Owner,
];

#[derive(Default)]
pub(super) struct RowChanges {
    /// The kinds, as (context, kind), whose rows watch notices named since the
    /// table last drew. A reload names none, so the rows it changes get no
    /// tint. A read of names patches only those rows, so the kind is enough
    /// unless a reload of the same kind lands before the same frame, and a
    /// burst of notices costs nothing per name here.
    noticed: Vec<(String, String)>,
    /// The rows that reads of those names added.
    pub(super) added: Vec<Added>,
    /// The page the table last drew.
    drawn: Option<Rc<TablePage>>,
    /// When each highlighted row, by `resource_id`, changed.
    marks: HashMap<String, Instant>,
    now: Option<Instant>,
    /// Whether this frame drew the table.
    seen: bool,
}

fn noticed(kinds: &[(String, String)], row: &ResourceSummary) -> bool {
    kinds
        .iter()
        .any(|(context, kind)| *context == row.cluster && *kind == row.kind)
}

fn added_id(added: &Added) -> String {
    resource_id_of(
        &added.kind,
        added.namespace.as_deref(),
        &added.name,
        &added.cluster,
    )
}

impl RowChanges {
    pub(super) fn begin_frame(&mut self) {
        self.seen = false;
    }

    /// Records the kind of a `patch` that read the rows a watch notice named.
    pub(super) fn notice(&mut self, patch: &Patch) {
        if patch.names.is_none() {
            return;
        }
        let (context, kind) = (&patch.scope.context, &patch.key.resource_kind.kind);
        if !self.noticed.iter().any(|(c, k)| c == context && k == kind) {
            self.noticed.push((context.clone(), kind.clone()));
        }
    }

    /// Forgets the page of a table this frame did not draw. Returns whether a
    /// highlight still fades, so the view asks for another frame.
    pub(super) fn end_frame(&mut self) -> bool {
        if !self.seen {
            *self = Self::default();
        }
        !self.marks.is_empty()
    }

    /// The opacity of the primary tint behind the row `id`, while it fades.
    pub(super) fn tint(&self, id: &str) -> Option<f32> {
        let (since, now) = (*self.marks.get(id)?, self.now?);
        let faded = HIGHLIGHT.progress(now.saturating_duration_since(since));
        (faded < 1.).then_some(TINT * (1. - faded))
    }

    /// Every row that still fades, with its tint opacity, in ID order.
    pub(super) fn tints(&self) -> Vec<(String, f32)> {
        let mut tints: Vec<_> = self
            .marks
            .keys()
            .filter_map(|id| Some((id.clone(), self.tint(id)?)))
            .collect();
        tints.sort_by(|a, b| a.0.cmp(&b.0));
        tints
    }
}

impl Inspector {
    /// Marks the rows of `page` that a watch notice added or changed since the
    /// table last drew. Reduced motion marks none and ends the highlights
    /// that had started.
    pub(super) fn observe_row_changes(&mut self, page: &Rc<TablePage>, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let mut changes = std::mem::take(&mut self.row_changes);
        changes.seen = true;
        changes.now = Some(now);
        if cx.reduce_motion() {
            changes.marks.clear();
        } else {
            changes
                .marks
                .retain(|_, since| now < *since + HIGHLIGHT.duration);
        }
        let kinds = std::mem::take(&mut changes.noticed);
        if !kinds.is_empty()
            && !cx.reduce_motion()
            && let Some(drawn) = &changes.drawn
        {
            let added: HashSet<String> = changes.added.iter().map(added_id).collect();
            let before: HashMap<String, &ResourceSummary> = drawn
                .rows
                .iter()
                .filter(|row| noticed(&kinds, row))
                .map(|row| (resource_id(row), row))
                .collect();
            for row in page.rows.iter().filter(|row| noticed(&kinds, row)) {
                let id = resource_id(row);
                let changed = match before.get(&id) {
                    Some(old) => WATCHED
                        .iter()
                        .any(|column| self.cell_text(*column, old) != self.cell_text(*column, row)),
                    None => added.contains(&id),
                };
                if changed {
                    changes.marks.insert(id, now);
                }
            }
        }
        changes.added.clear();
        changes.drawn = Some(page.clone());
        self.row_changes = changes;
    }

    /// The rows whose highlight still fades, with the tint opacity each shows,
    /// in ID order.
    #[doc(hidden)]
    pub fn row_highlights(&self) -> Vec<(String, f32)> {
        self.row_changes.tints()
    }
}
