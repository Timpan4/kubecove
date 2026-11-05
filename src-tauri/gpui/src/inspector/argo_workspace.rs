//! `ArgoApplicationWorkspaceHeader.svelte` and `argo-workspace-model.ts`.
//! A focused Application filters by managed resource identity, never its name.

use super::controls::{self, Variant};
use super::{Inspector, Page};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{Context, Div, SharedString, Task, Window, div, prelude::*, px};
use kubecove_lib::models::{
    ArgoApplicationInspector, ArgoApplicationRef, ArgoApplicationSummary, ResourceSummary,
};
use std::collections::HashMap;
use std::rc::Rc;

#[path = "argo_workspace_header.rs"]
mod header;

const FILTERS: [&str; 6] = [
    "All managed",
    "Needs sync",
    "Healthy",
    "Degraded",
    "Progressing",
    "Prune",
];

enum Snapshot {
    Loading,
    Ready(Box<ArgoApplicationInspector>),
    Failed(String),
}

pub(super) struct ApplicationWorkspace {
    app: ArgoApplicationSummary,
    snapshot: Snapshot,
    namespaces: Vec<String>,
    /// Each identity's filter memberships, rebuilt only after a read completes.
    identities: HashMap<String, [bool; 6]>,
    counts: [usize; 6],
    filter: Option<usize>,
    request: Rc<()>,
    task: Option<Task<()>>,
}

fn key(group: &str, kind: &str, namespace: &str, name: &str) -> String {
    format!(
        "{}:{}:{}:{}",
        group.trim(),
        kind.trim(),
        namespace.trim(),
        name.trim()
    )
    .to_lowercase()
}

impl Inspector {
    pub(super) fn open_application_resources(
        &mut self,
        app: ArgoApplicationSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.close_detail(cx);
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.page = Page::Resources;
        let mut namespaces: Vec<_> = app
            .resource_namespaces
            .iter()
            .map(|ns| ns.trim().to_owned())
            .filter(|ns| !ns.is_empty())
            .collect();
        if namespaces.is_empty()
            && let Some(ns) = app
                .destination_namespace
                .as_deref()
                .map(str::trim)
                .filter(|ns| !ns.is_empty())
        {
            namespaces.push(ns.to_owned());
        }
        namespaces.sort();
        namespaces.dedup();
        self.scope.namespace = if namespaces.len() == 1 {
            namespaces.first().cloned()
        } else {
            None
        };
        self.gitops_page.workspace = Some(ApplicationWorkspace {
            app,
            snapshot: Snapshot::Loading,
            namespaces,
            identities: HashMap::new(),
            counts: [0; 6],
            filter: None,
            request: Rc::new(()),
            task: None,
        });
        self.table_page = 0;
        self.load_scope(false, window, cx);
    }

    pub(super) fn application_workspace_name(&self) -> Option<&str> {
        self.gitops_page
            .workspace
            .as_ref()
            .filter(|_| self.page == Page::Resources)
            .map(|state| state.app.name.as_str())
    }

    pub(super) fn application_resources_read(
        &self,
        refresh: bool,
    ) -> crate::backend::Read<crate::backend::ScopeResult> {
        match self
            .gitops_page
            .workspace
            .as_ref()
            .filter(|state| self.page == Page::Resources && state.app.cluster == self.scope.context)
        {
            Some(state) => self
                .backend
                .application_resources(state.app.clone(), refresh),
            None => self.backend.resources(self.scope.clone(), refresh),
        }
    }

    pub(super) fn load_application_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.gitops_page.workspace.as_mut() else {
            return;
        };
        if state.app.cluster != self.scope.context {
            self.gitops_page.workspace = None;
            return;
        }
        let application = ArgoApplicationRef {
            name: state.app.name.clone(),
            namespace: state.app.namespace.clone(),
            context: Some(state.app.cluster.clone()),
            ..Default::default()
        };
        state.request = Rc::new(());
        let request = state.request.clone();
        let read = self.backend.argo_inspector(application);
        state.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(state) = this.gitops_page.workspace.as_mut() else {
                    return;
                };
                if !Rc::ptr_eq(&request, &state.request) {
                    return;
                }
                state.identities.clear();
                state.counts = [0; 6];
                state.snapshot = match result {
                    Ok(snapshot) => {
                        for resource in &snapshot.resources {
                            let (Some(kind), Some(name)) = (&resource.kind, &resource.name) else {
                                continue;
                            };
                            let health = resource
                                .health
                                .as_deref()
                                .unwrap_or_default()
                                .trim()
                                .to_lowercase();
                            let filters = [
                                true,
                                !resource
                                    .status
                                    .as_deref()
                                    .unwrap_or_default()
                                    .trim()
                                    .eq_ignore_ascii_case("synced"),
                                health == "healthy",
                                health == "degraded",
                                health == "progressing",
                                resource.requires_pruning == Some(true),
                            ];
                            for (count, matches) in state.counts.iter_mut().zip(filters) {
                                *count += usize::from(matches);
                            }
                            state.identities.insert(
                                key(
                                    resource.group.as_deref().unwrap_or_default(),
                                    kind,
                                    resource.namespace.as_deref().unwrap_or_default(),
                                    name,
                                ),
                                filters,
                            );
                        }
                        Snapshot::Ready(Box::new(snapshot))
                    }
                    Err(error) => Snapshot::Failed(error.message),
                };
                this.filter(cx);
            });
        }));
        cx.notify();
    }

    pub(super) fn argo_workspace_matches(&self, row: &ResourceSummary) -> bool {
        let Some(state) = self
            .gitops_page
            .workspace
            .as_ref()
            .filter(|_| self.page == Page::Resources)
        else {
            return true;
        };
        if row.cluster != state.app.cluster {
            return false;
        }
        if !state.namespaces.is_empty()
            && row.namespace.is_some()
            && !row
                .namespace
                .as_ref()
                .is_some_and(|ns| state.namespaces.contains(ns))
        {
            return false;
        }
        let group = row.group.as_deref().unwrap_or_else(|| {
            row.api_version
                .as_deref()
                .and_then(|version| version.split_once('/').map(|(group, _)| group))
                .unwrap_or_default()
        });
        let Some(filter) = state.filter else {
            return true;
        };
        state
            .identities
            .get(&key(
                group,
                &row.kind,
                row.namespace.as_deref().unwrap_or_default(),
                &row.name,
            ))
            .is_some_and(|filters| filters[filter])
    }

    pub(super) fn application_manages(&self, row: &ResourceSummary) -> bool {
        let Some(state) = self
            .gitops_page
            .workspace
            .as_ref()
            .filter(|_| self.page == Page::Resources)
        else {
            return false;
        };
        let group = row.group.as_deref().unwrap_or_else(|| {
            row.api_version
                .as_deref()
                .and_then(|version| version.split_once('/').map(|(group, _)| group))
                .unwrap_or_default()
        });
        state.identities.contains_key(&key(
            group,
            &row.kind,
            row.namespace.as_deref().unwrap_or_default(),
            &row.name,
        ))
    }

    pub(super) fn application_resource_filters(&self, cx: &mut Context<Self>) -> Option<Div> {
        let state = self
            .gitops_page
            .workspace
            .as_ref()
            .filter(|_| self.page == Page::Resources)?;
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .mt(px(8.))
                .pt(px(8.))
                .border_t_1()
                .border_color(Tokens::border())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .mr(px(4.))
                        .child(div().size(px(6.)).rounded_full().bg(Palette::sky400()))
                        .child(ui::caps("Argo CD resources", 10.)),
                )
                .children(FILTERS.iter().enumerate().map(|(index, label)| {
                    controls::button(Variant::Outline, 28., false)
                        .id(SharedString::from(format!("argo-filter:{label}")))
                        .test_support()
                        .when(state.filter == Some(index), |button| {
                            button
                                .bg(Tokens::primary().opacity(0.1))
                                .border_color(Tokens::primary().opacity(0.5))
                        })
                        .child(div().size(px(6.)).rounded_full().bg(match index {
                            1 | 4 => Palette::amber400(),
                            2 => Palette::emerald400(),
                            3 | 5 => gpui_kit::rgb(0xf87171).into(),
                            _ => Tokens::muted_foreground(),
                        }))
                        .child(ui::xs(format!("{label} {}", state.counts[index])))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(state) = this.gitops_page.workspace.as_mut() {
                                state.filter = if state.filter == Some(index) {
                                    None
                                } else {
                                    Some(index)
                                };
                            }
                            this.table_page = 0;
                            this.filter(cx);
                        }))
                })),
        )
    }
}
