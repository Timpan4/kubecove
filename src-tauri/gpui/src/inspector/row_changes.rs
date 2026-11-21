//! A one-time tint on resource table rows that a watch notice added or
//! changed. The web table has no such highlight; the user chose it for this
//! client on 2026-11-17, with the timing of the web sidebar's row reveal. The
//! table compares the page it draws with the page it last drew only on the
//! frame after a notice named rows, so an idle table pays nothing.
use super::Inspector;
use super::resource_observation::Patch;
use super::table_columns::{Column, resource_id, resource_id_of};
use crate::model::{Added, RowsId, identity_hash_of};
use crate::motion::{Bezier, Timing};
use gpui_kit::Context;
use std::collections::{HashMap, HashSet};
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
    /// The `identity_hash` of each row a watch notice named since the table
    /// last drew. A reload names none, so the rows it changes get no tint. A
    /// burst of notices costs one hash per name here and allocates no strings.
    noticed: Vec<u64>,
    /// The rows that reads of those names added.
    pub(super) added: Vec<Added>,
    /// The page the table last drew.
    drawn: Option<DrawnPage>,
    /// When each highlighted row, by `resource_id`, changed.
    marks: HashMap<String, Instant>,
    now: Option<Instant>,
    /// Whether this frame drew the table.
    seen: bool,
}

struct DrawnPage {
    revision: (RowsId, u64),
    indices: Vec<usize>,
    rows: Vec<DrawnRow>,
}

struct DrawnRow {
    identity: u64,
    id: String,
    text: [String; WATCHED.len()],
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

    /// Records the rows a watch notice named, which `patch` then read.
    pub(super) fn notice(&mut self, patch: &Patch) {
        let Some(names) = &patch.names else { return };
        let (context, kind) = (&patch.scope.context, &patch.key.resource_kind.kind);
        self.noticed.extend(
            names
                .iter()
                .map(|(namespace, name)| identity_hash_of(context, kind, namespace, name)),
        );
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
    pub(super) fn observe_row_changes(&mut self, page: &[usize], cx: &mut Context<Self>) {
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
        let mut named = std::mem::take(&mut changes.noticed);
        let source = self.visible.source();
        let revision = source.revision();
        let capture = || DrawnPage {
            revision,
            indices: page.to_vec(),
            rows: page
                .iter()
                .map(|&index| {
                    let row = &source[index];
                    DrawnRow {
                        identity: source.row_identities()[index],
                        id: resource_id(row),
                        text: WATCHED.map(|column| self.cell_text(column, row)),
                    }
                })
                .collect(),
        };
        let next = changes
            .drawn
            .as_ref()
            .is_none_or(|drawn| drawn.revision != revision || drawn.indices.as_slice() != page)
            .then(capture);
        if !named.is_empty()
            && !cx.reduce_motion()
            && let (Some(drawn), Some(next)) = (&changes.drawn, &next)
        {
            named.sort_unstable();
            let noticed = |row: &&DrawnRow| named.binary_search(&row.identity).is_ok();
            let added: HashSet<String> = changes.added.iter().map(added_id).collect();
            let before: HashMap<_, _> = drawn
                .rows
                .iter()
                .filter(noticed)
                .map(|row| (&row.id, &row.text))
                .collect();
            for row in next.rows.iter().filter(noticed) {
                let changed = match before.get(&row.id) {
                    Some(old) => *old != &row.text,
                    None => added.contains(&row.id),
                };
                if changed {
                    changes.marks.insert(row.id.clone(), now);
                }
            }
        }
        // Keeps the list's capacity for the next burst.
        named.clear();
        changes.noticed = named;
        changes.added.clear();
        if let Some(next) = next {
            changes.drawn = Some(next);
        }
        self.row_changes = changes;
    }

    /// The rows whose highlight still fades, with the tint opacity each shows,
    /// in ID order.
    #[doc(hidden)]
    pub fn row_highlights(&self) -> Vec<(String, f32)> {
        self.row_changes.tints()
    }
}
