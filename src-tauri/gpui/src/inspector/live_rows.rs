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
    pub(super) fn patch_kind(
        &mut self,
        scope: &Scope,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        resource_version: Option<String>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        let _span = crate::frame_profile::scope("watch", "patch");
        if self.scope != *scope {
            return rows;
        }
        let cluster_rows = self.page == super::Page::ClusterOverview
            && crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        // The version moves with the rows it describes, so a watch never starts past them.
        let versions = if cluster_rows {
            &mut self.cluster.versions
        } else {
            &mut self.live.versions
        };
        let signature = crate::backend::watch_signature(key);
        match resource_version {
            Some(version) => versions.insert(signature, version),
            None => versions.remove(&signature),
        };
        // A patch can change sorted values in place, which the visible
        // generation does not show.
        self.table_order.invalidate();
        let retired = if cluster_rows {
            crate::model::reconcile(&mut self.cluster.rows, key, rows, names);
            Vec::new()
        } else {
            Rc::get_mut(&mut self.rows)
                .expect("visible source released before patching")
                .patch(key, rows, names)
        };
        if !matches!(self.load, Load::Ready) && !self.rows.is_empty() {
            self.load = Load::Ready;
        }
        retired
    }

    /// The list version of the rows the view shows for `key`, where `patch_kind`
    /// records it. A cluster kind outside the Cluster Overview keeps the version
    /// of its cluster read until a patch replaces those rows.
    pub(super) fn shown_version(&self, key: &WatchResourceKey) -> Option<String> {
        let signature = crate::backend::watch_signature(key);
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        let cluster = || self.cluster.versions.get(&signature);
        let version = if cluster_kind && self.page == super::Page::ClusterOverview {
            cluster()
        } else if cluster_kind {
            self.live.versions.get(&signature).or_else(cluster)
        } else {
            self.live.versions.get(&signature)
        };
        version.cloned()
    }

    /// Whether the read that shows rows for `key` has settled, so its watch can
    /// start at their version.
    pub(super) fn rows_settled(&self, key: &WatchResourceKey) -> bool {
        if self
            .kind_selection
            .loading(&crate::backend::watch_signature(key))
        {
            return false;
        }
        let custom = key.resource_kind.plural.is_some();
        let cluster_kind = crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str());
        !self.live.scope_loading
            && !(custom
                && matches!(
                    self.custom_load,
                    super::custom_resources::CustomLoad::Loading
                ))
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
