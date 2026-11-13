//! Resource data is projected once per read. Queries reuse flat index buffers.

use super::Inspector;
use super::resource_filters::HealthFilter;
use crate::model::{KindFilter, PreparedRows};
use gpui_kit::Context;
use kubecove_lib::models::ResourceSummary;
use std::{cell::RefCell, rc::Rc};

struct Snapshot {
    scope: Rc<PreparedRows>,
    request: Rc<()>,
    cluster_request: Rc<()>,
    buffers: [(usize, usize); 2],
    data: Rc<PreparedRows>,
}

#[derive(Default)]
pub(super) struct RowIndex(RefCell<Option<Snapshot>>);

impl RowIndex {
    pub(super) fn invalidate(&self) {
        self.0.take();
    }

    fn source(
        &self,
        scope: &Rc<PreparedRows>,
        cluster: &[ResourceSummary],
        request: &Rc<()>,
        cluster_request: &Rc<()>,
        contexts: &[String],
    ) -> Rc<PreparedRows> {
        let _profile = crate::frame_profile::scope("rows", "source");
        // Scope rows already show inherited owners (`PreparedRows::with_inherited_owners`).
        if cluster.is_empty() {
            self.0.take();
            return scope.clone();
        }
        let sources = [&scope[..], cluster];
        let buffers = sources.map(|rows| (rows.as_ptr() as usize, rows.len()));
        let mut current = self.0.borrow_mut();
        if let Some(snapshot) = &*current
            && Rc::ptr_eq(&snapshot.request, request)
            && Rc::ptr_eq(&snapshot.cluster_request, cluster_request)
            && Rc::ptr_eq(&snapshot.scope, scope)
            && snapshot.buffers == buffers
        {
            return snapshot.data.clone();
        }
        // Completed reads replace immutable buffers before filtering. Cluster
        // loading drops its previous buffer earlier, so its separate read token
        // prevents allocator reuse from preserving a stale projection.
        let mut rows = Vec::with_capacity(scope.len() + cluster.len());
        rows.extend_from_slice(scope);
        rows.extend_from_slice(cluster);
        let data = Rc::new(PreparedRows::new(rows, contexts));
        *current = Some(Snapshot {
            scope: scope.clone(),
            request: request.clone(),
            cluster_request: cluster_request.clone(),
            buffers,
            data: data.clone(),
        });
        data
    }
}

impl Inspector {
    pub(super) fn filter_rows(&mut self, cx: &mut Context<Self>) {
        let _profile = crate::frame_profile::scope("rows", "filter");
        let query = self.search.read(cx).value().trim().to_lowercase();
        let kinds = self.scope_kinds();
        let contexts = self.context_selection();
        let in_context =
            |row: &ResourceSummary| contexts.as_ref().is_none_or(|c| c.contains(&row.cluster));
        let source = self.row_index.source(
            self.observation.rows(),
            self.cluster_prepared(),
            &self.request,
            &self.cluster.request,
            self.observation.contexts(),
        );
        let mut visible = std::mem::take(&mut self.visible);
        let kind_filter = KindFilter {
            kinds: &kinds,
            selected: |kind: &str, api: Option<&str>| self.selected_kind(kind, api),
        };
        let mut allowed = Vec::new();
        source.fill_kind_mask(&kind_filter, &mut allowed);
        // The kind, scope, context and namespace tests run once per row. The
        // owner list, the refilter and the health summary all read the result.
        // Per interned owner, the first row in view that lists it.
        let mut first_row = vec![usize::MAX; source.owner_count()];
        let in_view: Vec<bool> = source
            .keyed_rows()
            .enumerate()
            .map(|(index, (row, prepared))| {
                let keep = allowed[prepared.variant]
                    && self.row_scope_visible(row)
                    && in_context(row)
                    && self.filters.in_namespace(row);
                if let Some(owner) = prepared.owner_index().filter(|_| keep) {
                    first_row[owner] = first_row[owner].min(index);
                }
                keep
            })
            .collect();
        visible.refilter_with(
            source.clone(),
            kind_filter,
            &query,
            |index, row, owner| {
                in_view[index] && self.filters.in_owner(owner) && self.filters.health.matches(row)
            },
            |row| self.application_manages(row),
            &self.persisted.preferences.resource_kind_lanes,
        );
        let summary_profile = crate::frame_profile::scope("rows", "summary");
        // Without a health filter the summary counts exactly the refilter's
        // rows, which it has already summarized.
        let summary = if self.filters.health == HealthFilter::All {
            visible.health()
        } else {
            let matches = visible.search_matches();
            let mut summary = crate::model::HealthSummary::default();
            for (index, (row, prepared)) in source.keyed_rows().enumerate() {
                if in_view[index]
                    && matches.contains(index)
                    && self
                        .filters
                        .in_owner(prepared.owner_index().map(|owner| source.owner_at(owner).0))
                {
                    summary.add(row);
                }
            }
            summary
        };
        // Owners that share a filter key show once, with the label of the
        // first row in view, in the rank `PreparedRows` collated.
        let mut by_key: Vec<Option<(usize, usize)>> = vec![None; source.owner_key_count()];
        for (owner, &row) in first_row.iter().enumerate() {
            let slot = &mut by_key[source.owner_key_id(owner)];
            if row != usize::MAX && slot.is_none_or(|(first, _)| row < first) {
                *slot = Some((row, owner));
            }
        }
        let mut listed: Vec<usize> = by_key
            .into_iter()
            .flatten()
            .map(|(_, owner)| owner)
            .collect();
        listed.sort_unstable_by_key(|&owner| source.owner_rank(owner));
        let owners: Vec<_> = listed
            .into_iter()
            .map(|owner| {
                let (key, label) = source.owner_at(owner);
                (key.to_owned(), label.to_owned())
            })
            .collect();
        drop(summary_profile);
        self.filters.owners = owners;
        self.filters.summary = summary;
        self.visible = visible;
        cx.notify();
    }
}
