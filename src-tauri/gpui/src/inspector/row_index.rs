//! Resource data is projected once per read. Queries reuse flat index buffers.

use super::Inspector;
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
        let data = Rc::new(PreparedRows::new(rows));
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
        let source = self.row_index.source(
            &self.rows,
            self.cluster_prepared(),
            &self.request,
            &self.cluster.request,
        );
        let mut visible = std::mem::take(&mut self.visible);
        let summary_profile = crate::frame_profile::scope("rows", "summary");
        let mut owners = std::collections::BTreeMap::new();
        let mut summary = crate::model::HealthSummary::default();
        let mut allowed = Vec::new();
        source.fill_kind_mask(
            &KindFilter {
                kinds: &kinds,
                selected: |kind: &str, api: Option<&str>| self.selected_kind(kind, api),
            },
            &mut allowed,
        );
        for (row, prepared) in source.keyed_rows() {
            if !(allowed[prepared.variant]
                && self.row_scope_visible(row)
                && self.filters.in_namespace(row))
            {
                continue;
            }
            let owner = &prepared.owner;
            if let Some((key, label)) = &owner {
                owners.entry(key.as_str()).or_insert(label.as_str());
            }
            if self
                .filters
                .in_owner(owner.as_ref().map(|(key, _)| key.as_str()))
                && (query.is_empty() || prepared.search.contains(&query))
            {
                summary.add(row);
            }
        }
        let mut owners: Vec<_> = owners
            .into_iter()
            .map(|(key, label)| (key.to_owned(), label.to_owned()))
            .collect();
        owners.sort_by(|a, b| crate::collate::collate(&a.1, &b.1, false));
        self.filters.owners = owners;
        self.filters.summary = summary;
        drop(summary_profile);
        visible.refilter_with(
            source,
            KindFilter {
                kinds: &kinds,
                selected: |kind: &str, api: Option<&str>| self.selected_kind(kind, api),
            },
            &query,
            |row, owner| {
                self.row_scope_visible(row)
                    && self.filters.in_namespace(row)
                    && self.filters.in_owner(owner)
                    && self.filters.health.matches(row)
            },
            |row| self.application_manages(row),
            &self.persisted.preferences.resource_kind_lanes,
        );
        self.visible = visible;
        cx.notify();
    }
}
