//! Entry-point interface shared by table, detail and workspace surfaces.
use super::{Inspector, Page};
use crate::{
    ui,
    workspaces::entries::{Entries, Entry, EntryKind},
};
use gpui_kit::{AnyElement, Context, SharedString, TestSupportExt, Window, div, prelude::*, px};
use kubecove_lib::models::ResourceSummary;

impl Inspector {
    pub(super) fn visit_scope(
        &mut self,
        kind: EntryKind,
        context: String,
        namespace: Option<String>,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.launcher.active {
            let app = kind == EntryKind::App;
            self.persisted
                .workspace_entry_points
                .entry(id)
                .or_default()
                .visit(Entry {
                    kind,
                    cluster_context: context,
                    namespace,
                    name,
                    resource_kind: app.then(|| "Application".into()),
                    api_version: app.then(|| "argoproj.io/v1alpha1".into()),
                    last_visited_at: chrono::Utc::now().to_rfc3339(),
                });
            self.save_preferences(cx);
        }
    }
    pub(super) fn workspace_entries(&self) -> Option<&Entries> {
        self.launcher
            .active
            .and_then(|id| self.persisted.workspace_entry_points.get(&id))
    }
    pub(super) fn resource_pinned(&self, resource: &ResourceSummary) -> bool {
        self.workspace_entries().is_some_and(|entries| {
            entries.pinned.iter().any(|entry| {
                entry.kind == EntryKind::Resource
                    && entry.cluster_context == resource.cluster
                    && entry.namespace == resource.namespace
                    && entry.name == resource.name
                    && entry.resource_kind.as_deref() == Some(resource.kind.as_str())
                    && entry.api_version == resource.api_version
            })
        })
    }
    pub(super) fn toggle_resource_pin(
        &mut self,
        resource: &ResourceSummary,
        cx: &mut Context<Self>,
    ) {
        self.ensure_active_workspace();
        if let Some(id) = self.launcher.active {
            self.persisted
                .workspace_entry_points
                .entry(id)
                .or_default()
                .toggle_pin(Entry::resource(resource));
            self.save_preferences(cx);
        }
    }
    pub(super) fn visit_resource(&mut self, resource: &ResourceSummary, cx: &mut Context<Self>) {
        if let Some(id) = self.launcher.active {
            self.persisted
                .workspace_entry_points
                .entry(id)
                .or_default()
                .visit(Entry::resource(resource));
            self.save_preferences(cx);
        }
    }
    pub(super) fn entry_list(&self, pinned: bool, cx: &mut Context<Self>) -> AnyElement {
        let panel = if pinned { "Pinned" } else { "Recent" };
        let entries = self.workspace_entries().map(|entries| {
            if pinned {
                &entries.pinned
            } else {
                &entries.recent
            }
        });
        let entries = entries.map_or(&[][..], Vec::as_slice);
        div()
            .id(SharedString::from(format!("entries:{panel}")))
            .test_support()
            .aria_description(entries.len().to_string())
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .min_h(px(80.))
            .p(px(12.))
            .when(entries.is_empty(), |list| {
                list.child(ui::muted(ui::sm(if pinned {
                    "Pin resources from the table or detail panel."
                } else {
                    "Recent visits will appear here."
                })))
            })
            .children(entries.iter().enumerate().map(|(index, entry)| {
                let label = format!(
                    "{} / {}",
                    entry.resource_kind.as_deref().unwrap_or("Namespace"),
                    entry.name
                );
                let entry = entry.clone();
                ui::outline_button()
                    .id(SharedString::from(format!("entry:{panel}:{index}")))
                    .test_support()
                    .aria_label(label.clone())
                    .h(px(24.))
                    .child(ui::xs(label))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_workspace_entry(&entry, window, cx)
                    }))
            }))
            .into_any_element()
    }
    fn open_workspace_entry(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_leave(cx) {
            return;
        }
        if entry.kind == EntryKind::Namespace && entry.cluster_context == self.scope.context {
            self.select_namespace(Some(entry.name.clone()), window, cx);
            return;
        }
        if entry.kind == EntryKind::App {
            self.visit_scope(
                EntryKind::App,
                entry.cluster_context.clone(),
                entry.namespace.clone(),
                entry.name.clone(),
                cx,
            );
            self.open_page(super::Page::GitOps, cx);
            return;
        }
        // A saved identity is resolved against current rows, never a cached object.
        let resource = self
            .workspace_rows()
            .find(|row| {
                row.cluster == entry.cluster_context
                    && row.namespace == entry.namespace
                    && row.name == entry.name
                    && Some(&row.kind) == entry.resource_kind.as_ref()
                    && row.api_version == entry.api_version
            })
            .cloned();
        if let Some(resource) = resource {
            self.open_page(Page::Resources, cx);
            self.open_resource(resource, window, cx);
        } else {
            self.notice = Some("This Kubernetes object is no longer available. Refresh the view and select the current object if it still exists.".into());
            cx.notify();
        }
    }
}
