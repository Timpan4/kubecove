//! Snapshot reconciliation patches listed and visible rows without rebuilding the table.
use super::Inspector;
use crate::backend::Scope;
use crate::model::ObservedNames;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};

impl Inspector {
    /// The contexts whose rows the page's watches patch: every context of the
    /// scope read, or the selected context alone on the Cluster Overview,
    /// whose cluster read covers that one context.
    pub(super) fn watch_contexts(&self) -> Vec<String> {
        if self.page == super::Page::ClusterOverview {
            vec![self.scope.context.clone()]
        } else {
            self.observation.contexts().to_vec()
        }
    }

    pub(super) fn patch_kind(
        &mut self,
        scope: &Scope,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        resource_version: Option<String>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        let _span = crate::frame_profile::scope("watch", "patch");
        if !self.observation.accepts(scope)
            || self.behind_shown(scope, key, resource_version.as_deref())
        {
            return rows;
        }
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        // The Cluster Overview's cluster read covers the selected context only.
        if cluster_kind
            && self.page == super::Page::ClusterOverview
            && scope.context != self.scope.context
        {
            return rows;
        }
        // Where the cluster read's rows show, its kinds patch there, so a
        // selected cluster kind on Resources never shows a second, stale copy.
        let cluster_rows = self.cluster_source(&scope.context, key);
        if cluster_rows {
            let id = crate::backend::watch_id(&scope.context, key);
            match resource_version {
                Some(version) => self.cluster.versions.insert(id, version),
                None => self.cluster.versions.remove(&id),
            };
            crate::model::reconcile(&mut self.cluster.rows, &scope.context, key, rows, names);
            self.observation.rows_patched();
            Vec::new()
        } else {
            self.observation
                .patch(scope, key, rows, resource_version, names)
        }
    }

    /// Whether rows read at `version` are older than the rows the view shows,
    /// as when a watch reloads a kind while the scope list is held and the
    /// list then installs newer rows. Versions compare as numbers, as the live
    /// store's watch patching compares them; others never count as older.
    fn behind_shown(&self, scope: &Scope, key: &WatchResourceKey, version: Option<&str>) -> bool {
        let number = |version: &str| version.parse::<u64>().ok();
        let shown = self.shown_version(&scope.context, key);
        matches!(
            (version.and_then(number), shown.as_deref().and_then(number)),
            (Some(version), Some(shown)) if version < shown
        )
    }

    /// The list version of the rows the view shows for `key` in `context`,
    /// where `patch_kind` records it. A cluster kind outside the Cluster
    /// Overview keeps the version of its cluster read until a patch replaces those rows.
    pub(super) fn shown_version(&self, context: &str, key: &WatchResourceKey) -> Option<String> {
        let id = crate::backend::watch_id(context, key);
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        let cluster = || self.cluster.versions.get(&id);
        let version = if cluster_kind
            && (self.page == super::Page::ClusterOverview || self.cluster_source(context, key))
        {
            cluster()
        } else if cluster_kind {
            self.observation.version(&id).or_else(cluster)
        } else {
            self.observation.version(&id)
        };
        version.cloned()
    }

    /// Whether the read that shows rows for `key` in `context` has settled, so
    /// its watch can start at their version.
    pub(super) fn rows_settled(&self, context: &str, key: &WatchResourceKey) -> bool {
        if self
            .kind_selection
            .loading(&crate::backend::watch_id(context, key))
        {
            return false;
        }
        let custom = key.resource_kind.plural.is_some();
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        // Each context's watches wait only for that context's read.
        !self.observation.context_loading(context)
            && !(custom && self.observation.custom_loading(context))
            && !(cluster_kind && self.cluster_loading())
    }

    pub(super) fn row_scope_visible(&self, row: &ResourceSummary) -> bool {
        self.argo_workspace_matches(row)
            && self.shortcut_owner_matches(row)
            && self.launcher.in_scope(row)
            && self.palette.kind.is_none_or(|kind| row.kind == kind)
            && self.live.selected.as_ref().is_none_or(|kind| {
                row.kind == kind.kind && row.api_version.as_ref() == Some(&kind.api_version)
            })
    }
}
