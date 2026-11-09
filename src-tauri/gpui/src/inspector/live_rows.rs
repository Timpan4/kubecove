//! Snapshot reconciliation patches listed and visible rows without rebuilding the table.
use super::{Inspector, Load};
use crate::backend::Scope;
use crate::model::ObservedNames;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};
use std::rc::Rc;

pub(super) struct Patch {
    pub scope: Scope,
    pub key: WatchResourceKey,
    pub rows: Vec<ResourceSummary>,
    /// The version of the list `rows` came from.
    pub resource_version: Option<String>,
    pub names: Option<ObservedNames>,
    pub watch: Option<String>,
}

impl Inspector {
    /// The contexts whose rows the page's watches patch: every context of the
    /// scope read, or the selected context alone on the Cluster Overview,
    /// whose cluster read covers that one context.
    pub(super) fn watch_contexts(&self) -> Vec<String> {
        if self.page == super::Page::ClusterOverview {
            vec![self.scope.context.clone()]
        } else {
            self.scope_load.contexts().to_vec()
        }
    }

    /// Whether a patch read in `scope` belongs to the rows the page shows: one
    /// of the scope read's contexts, in the selected namespace.
    pub(super) fn patch_in_scope(&self, scope: &Scope) -> bool {
        scope.namespace == self.scope.namespace && self.scope_load.has_context(&scope.context)
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
        if !self.patch_in_scope(scope) {
            return rows;
        }
        let cluster_rows = self.page == super::Page::ClusterOverview
            && crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        // The Cluster Overview's cluster read covers the selected context only.
        if cluster_rows && scope.context != self.scope.context {
            return rows;
        }
        // The version moves with the rows it describes, so a watch never starts past them.
        let versions = if cluster_rows {
            Some(&mut self.cluster.versions)
        } else {
            self.scope_load.versions_mut(&scope.context)
        };
        if let Some(versions) = versions {
            let id = crate::backend::watch_id(&scope.context, key);
            match resource_version {
                Some(version) => versions.insert(id, version),
                None => versions.remove(&id),
            };
        }
        let retired = if cluster_rows {
            crate::model::reconcile(&mut self.cluster.rows, &scope.context, key, rows, names);
            Vec::new()
        } else {
            Rc::get_mut(&mut self.rows)
                .expect("visible source released before patching")
                .patch(&scope.context, key, rows, names)
        };
        if !matches!(self.scope_load.rows, Load::Ready) && !self.rows.is_empty() {
            self.scope_load.rows = Load::Ready;
        }
        retired
    }

    /// The list version of the rows the view shows for `key` in `context`,
    /// where `patch_kind` records it. A cluster kind outside the Cluster
    /// Overview keeps the version of its cluster read until a patch replaces those rows.
    pub(super) fn shown_version(&self, context: &str, key: &WatchResourceKey) -> Option<String> {
        let id = crate::backend::watch_id(context, key);
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        let cluster = || self.cluster.versions.get(&id);
        let version = if cluster_kind && self.page == super::Page::ClusterOverview {
            cluster()
        } else if cluster_kind {
            self.scope_load.version(&id).or_else(cluster)
        } else {
            self.scope_load.version(&id)
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
        !self.scope_load.loading()
            && !(custom
                && self.scope_load.custom() == super::custom_resources::CustomLoad::Loading)
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
