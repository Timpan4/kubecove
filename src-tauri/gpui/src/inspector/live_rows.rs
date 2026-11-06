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
    pub names: Option<ObservedNames>,
    pub watch: Option<String>,
}

impl Inspector {
    pub(super) fn patch_kind(
        &mut self,
        scope: &Scope,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        let _span = crate::frame_profile::scope("watch", "patch");
        if self.scope != *scope {
            return rows;
        }
        let retired = if self.page == super::Page::ClusterOverview
            && crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str())
        {
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
