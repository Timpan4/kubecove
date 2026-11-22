//! Resource data is projected once per read. Queries reuse flat index buffers.

use super::resource_filters::HealthFilter;
use super::{Inspector, Page};
use crate::model::{HealthSummary, KindFilter, PreparedRows, RowsChange, RowsId};
use gpui_kit::Context;
use std::rc::Rc;

type Revision = (RowsId, u64);

struct Snapshot {
    scope: Revision,
    cluster: Revision,
    contexts: Vec<String>,
    data: Rc<PreparedRows>,
}

#[derive(Clone, Copy)]
pub(super) enum PatchSource {
    Scope,
    Cluster,
}

#[derive(Default)]
pub(super) struct RowIndex {
    combined: Option<Snapshot>,
    eligibility: Option<Eligibility>,
}

impl RowIndex {
    pub(super) fn invalidate(&mut self) {
        *self = Self::default();
    }

    pub(super) fn invalidate_filters(&mut self) {
        self.eligibility = None;
    }

    fn source(
        &mut self,
        scope: &Rc<PreparedRows>,
        cluster: Option<&PreparedRows>,
        contexts: &[String],
    ) -> Rc<PreparedRows> {
        let _profile = crate::frame_profile::scope("rows", "source");
        let Some(cluster) = cluster.filter(|rows| !rows.is_empty()) else {
            self.combined = None;
            return scope.clone();
        };
        let versions = (scope.revision(), cluster.revision());
        if let Some(snapshot) = &self.combined
            && (snapshot.scope, snapshot.cluster) == versions
            && snapshot.contexts == contexts
        {
            return snapshot.data.clone();
        }
        // Both sources already have their inherited owners. Their indices in
        // the combined view are unchanged until either source rebuilds.
        let mut rows = Vec::with_capacity(scope.len() + cluster.len());
        rows.extend_from_slice(scope);
        rows.extend_from_slice(cluster);
        let data = Rc::new(PreparedRows::new(rows, contexts));
        self.combined = Some(Snapshot {
            scope: versions.0,
            cluster: versions.1,
            contexts: contexts.to_vec(),
            data: data.clone(),
        });
        data
    }

    pub(super) fn apply_patch(
        &mut self,
        target: PatchSource,
        change: &RowsChange,
        scope: &PreparedRows,
        cluster: &PreparedRows,
    ) {
        let RowsChange::Updated(indices) = change else {
            self.combined = None;
            return;
        };
        if indices.is_empty() {
            return;
        }
        let Some(snapshot) = &mut self.combined else {
            return;
        };
        let (source, offset, cached, other_matches) = match target {
            PatchSource::Scope => (
                scope,
                0,
                &mut snapshot.scope,
                snapshot.cluster == cluster.revision(),
            ),
            PatchSource::Cluster => (
                cluster,
                scope.len(),
                &mut snapshot.cluster,
                snapshot.scope == scope.revision(),
            ),
        };
        let next = source.revision();
        if !other_matches || cached.0 != next.0 || cached.1.checked_add(1) != Some(next.1) {
            return;
        }
        Rc::get_mut(&mut snapshot.data)
            .expect("visible source released before patching")
            .update_from(source, offset, indices);
        *cached = next;
    }
}

struct Eligibility {
    source: RowsId,
    page: Page,
    palette_kind: Option<&'static str>,
    application_scope: bool,
    /// Scope and owner filtering are independent of the search text and health.
    included: Vec<bool>,
    preferred: Vec<bool>,
}

impl Eligibility {
    fn matches(&self, inspector: &Inspector, source: &PreparedRows) -> bool {
        self.source == source.identity()
            && self.page == inspector.page
            && self.palette_kind == inspector.palette.kind
            && self.application_scope == inspector.application_workspace_name().is_some()
    }

    fn prepare(inspector: &Inspector, source: &PreparedRows) -> (Self, Vec<(String, String)>) {
        let kinds = inspector.scope_kinds();
        let contexts = inspector.context_selection();
        let mut allowed = Vec::new();
        source.fill_kind_mask(
            &KindFilter {
                kinds: &kinds,
                selected: |kind: &str, api: Option<&str>| inspector.selected_kind(kind, api),
            },
            &mut allowed,
        );
        // Keep the choices from all rows in scope, before search, owner and health filters.
        let mut first_row = vec![usize::MAX; source.owner_count()];
        let mut included = Vec::with_capacity(source.len());
        let mut preferred = Vec::with_capacity(source.len());
        for (index, (row, key)) in source.keyed_rows().enumerate() {
            let in_scope = allowed[key.variant]
                && inspector.row_scope_visible(row)
                && contexts.as_ref().is_none_or(|c| c.contains(&row.cluster))
                && inspector.filters.in_namespace(row);
            let owner = key.owner_index();
            if let Some(owner) = owner.filter(|_| in_scope) {
                first_row[owner] = first_row[owner].min(index);
            }
            let included_row = in_scope
                && inspector
                    .filters
                    .in_owner(owner.map(|owner| source.owner_at(owner).0));
            included.push(included_row);
            preferred.push(included_row && inspector.application_manages(row));
        }
        // Owners sharing a key take the label of their first row in scope.
        let mut by_key: Vec<Option<(usize, usize)>> = vec![None; source.owner_key_count()];
        for (owner, row) in first_row.into_iter().enumerate() {
            let slot = &mut by_key[source.owner_key_id(owner)];
            if row != usize::MAX && slot.is_none_or(|(first, _)| row < first) {
                *slot = Some((row, owner));
            }
        }
        let mut listed: Vec<_> = by_key
            .into_iter()
            .flatten()
            .map(|(_, owner)| owner)
            .collect();
        listed.sort_unstable_by_key(|&owner| source.owner_rank(owner));
        let owners = listed
            .into_iter()
            .map(|owner| {
                let (key, label) = source.owner_at(owner);
                (key.to_owned(), label.to_owned())
            })
            .collect();
        (
            Self {
                source: source.identity(),
                page: inspector.page,
                palette_kind: inspector.palette.kind,
                application_scope: inspector.application_workspace_name().is_some(),
                included,
                preferred,
            },
            owners,
        )
    }
}

impl Inspector {
    pub(super) fn filter_rows(&mut self, cx: &mut Context<Self>) {
        let _profile = crate::frame_profile::scope("rows", "filter");
        let query = self.search.read(cx).value().trim().to_lowercase();
        let cluster = self.wants_cluster_rows().then_some(&self.cluster.rows);
        let source = self.row_index.source(
            self.observation.rows(),
            cluster,
            self.observation.contexts(),
        );
        if self
            .row_index
            .eligibility
            .as_ref()
            .is_none_or(|cached| !cached.matches(self, &source))
        {
            let (eligibility, owners) = Eligibility::prepare(self, &source);
            self.row_index.eligibility = Some(eligibility);
            self.filters.owners = owners;
        }
        let eligibility = self
            .row_index
            .eligibility
            .as_ref()
            .expect("scope eligibility prepared");
        let health = self.filters.health;
        self.visible.refilter_prepared(
            source.clone(),
            &query,
            |index, row| eligibility.included[index] && health.matches(row),
            |index, _| eligibility.preferred[index],
            &self.persisted.preferences.resource_kind_lanes,
        );
        let _profile = crate::frame_profile::scope("rows", "summary");
        // Without a health filter the refilter has already summarized these rows.
        self.filters.summary = if health == HealthFilter::All {
            self.visible.health()
        } else {
            let matches = self.visible.search_matches();
            HealthSummary::of(source.iter().enumerate().filter_map(|(index, row)| {
                (eligibility.included[index] && matches.contains(index)).then_some(row)
            }))
        };
        cx.notify();
    }
}
