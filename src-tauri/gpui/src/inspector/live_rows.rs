//! Snapshot reconciliation patches listed and visible rows without rebuilding the table.
use super::{Inspector, Load};
use crate::{backend::Scope, model::ownership_group};
use gpui_kit::Context;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};
use std::collections::{HashMap, HashSet};
type Name = (Option<String>, String);

pub(super) struct Patch {
    pub scope: Scope,
    pub key: WatchResourceKey,
    pub rows: Vec<ResourceSummary>,
    pub names: Option<HashSet<Name>>,
}

impl Inspector {
    pub(super) fn patch_kind(
        &mut self,
        scope: &Scope,
        key: &WatchResourceKey,
        rows: Vec<ResourceSummary>,
        names: Option<&HashSet<Name>>,
        cx: &mut Context<Self>,
    ) {
        if self.scope != *scope {
            return;
        }
        let matches = |row: &ResourceSummary| {
            row.kind == key.resource_kind.kind
                && (key.resource_kind.api_version.is_some() || row.dynamic != Some(true))
                && key
                    .resource_kind
                    .api_version
                    .as_ref()
                    .is_none_or(|api| row.api_version.as_ref() == Some(api))
                && key
                    .namespace
                    .as_ref()
                    .is_none_or(|ns| row.namespace.as_ref() == Some(ns))
                && names
                    .is_none_or(|names| names.contains(&(row.namespace.clone(), row.name.clone())))
        };
        let mut next: HashMap<_, _> = rows
            .into_iter()
            .filter(&matches)
            .map(|row| ((row.namespace.clone(), row.name.clone()), row))
            .collect();
        let search = self.search.read(cx).value().trim().to_lowercase();
        let kinds = self.scope_kinds();
        let mut visible = next
            .values()
            .filter(|row| self.row_visible(row, &search, &kinds))
            .map(|row| ((row.namespace.clone(), row.name.clone()), row.clone()))
            .collect::<HashMap<_, _>>();
        let rows = if self.page == super::Page::ClusterOverview
            && crate::backend::CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str())
        {
            &mut self.cluster.rows
        } else {
            &mut self.rows
        };
        rows.retain_mut(|row| {
            if !matches(row) {
                return true;
            }
            if let Some(new) = next.remove(&(row.namespace.clone(), row.name.clone())) {
                *row = new;
                true
            } else {
                false
            }
        });
        rows.extend(next.into_values());
        self.visible.retain_mut(|row| {
            if !matches(row) {
                return true;
            }
            if let Some(new) = visible.remove(&(row.namespace.clone(), row.name.clone())) {
                *row = new;
                true
            } else {
                false
            }
        });
        self.visible.extend(visible.into_values());
        if !matches!(self.load, Load::Ready) && !self.rows.is_empty() {
            self.load = Load::Ready;
        }
        cx.notify();
    }

    pub(super) fn row_visible(
        &self,
        row: &ResourceSummary,
        search: &str,
        kinds: &[String],
    ) -> bool {
        self.argo_workspace_matches(row)
            && kinds.contains(&row.kind)
            && self.launcher.in_scope(row)
            && self.palette.kind.is_none_or(|kind| row.kind == kind)
            && self.live.selected.as_ref().is_none_or(|kind| {
                row.kind == kind.kind && row.api_version.as_ref() == Some(&kind.api_version)
            })
            && (search.is_empty()
                || row.name.to_lowercase().contains(search)
                || row.kind.to_lowercase().contains(search)
                || row
                    .namespace
                    .as_deref()
                    .is_some_and(|ns| ns.to_lowercase().contains(search))
                || ownership_group(row).0.to_lowercase().contains(search)
                || row
                    .helm_release
                    .as_deref()
                    .is_some_and(|release| release.to_lowercase().contains(search)))
    }
}
