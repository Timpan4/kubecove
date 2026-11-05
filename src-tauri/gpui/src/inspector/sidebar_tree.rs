//! Built-in sidebar branches. Descendants are constructed only for open parents.
use super::{Inspector, Page};
use crate::{model, rbac::RbacView, theme::Tokens, ui};
use gpui_kit::{
    AnyElement, ClickEvent, Context, SharedString, TestSupportExt, Window, div, prelude::*, px,
};
use std::collections::HashSet;

pub(super) const GROUPS: &[(&str, &[&str])] = &[
    (
        "Workloads",
        &[
            "Pod",
            "Deployment",
            "ReplicaSet",
            "StatefulSet",
            "DaemonSet",
            "Job",
            "CronJob",
        ],
    ),
    ("Network", &["Service", "Ingress"]),
    ("Config", &["ConfigMap", "Secret"]),
    (
        "Storage",
        &["PersistentVolumeClaim", "PersistentVolume", "StorageClass"],
    ),
    (
        "Cluster Overview",
        &["Node", "StorageClass", "PersistentVolume"],
    ),
    (
        "RBAC",
        &[
            "Namespace Access",
            "Roles",
            "Cluster Roles",
            "Bindings",
            "Service Accounts",
        ],
    ),
];

#[derive(Default)]
pub(super) struct TreeState {
    pub discovery: super::sidebar_discovery::Discovery,
    pub open: HashSet<String>,
    pub selected: Option<String>,
}

impl Inspector {
    fn tree_select(
        &mut self,
        path: String,
        namespace: Option<String>,
        kinds: &'static [&'static str],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        let namespaced = namespace.is_some();
        self.select_namespace(namespace, window, cx);
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        if !namespaced
            && kinds.len() == 1
            && kinds
                .iter()
                .any(|kind| crate::backend::CLUSTER_KINDS.contains(kind))
        {
            self.open_cluster_overview(window, cx);
        }
        self.kind_selection.chosen = Some(kinds.iter().map(|kind| (*kind).to_owned()).collect());
        if kinds
            .iter()
            .any(|kind| crate::backend::CLUSTER_KINDS.contains(kind))
        {
            self.load_cluster(false, cx);
        }
        self.sidebar_tree.selected = Some(path);
        self.close_detail(cx);
        self.filter(cx);
    }

    pub(super) fn builtin_branch(
        &self,
        label: &'static str,
        namespace: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let depth = usize::from(namespace.is_some());
        let path = namespace.map_or_else(|| label.to_owned(), |ns| format!("{ns}/{label}"));
        let mut rows = vec![self.builtin_row(
            path.clone(),
            label.into(),
            namespace.map(str::to_owned),
            depth,
            true,
            cx,
        )];
        if self.sidebar_tree.open.contains(&path) {
            let kinds = GROUPS
                .iter()
                .find(|(name, _)| *name == label)
                .expect("builtin group")
                .1;
            for kind in kinds {
                rows.push(self.builtin_row(
                    format!("{path}/{kind}"),
                    (*kind).into(),
                    namespace.map(str::to_owned),
                    depth + 1,
                    false,
                    cx,
                ));
            }
        }
        rows
    }

    pub(super) fn namespace_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for ns in self
            .namespaces
            .iter()
            .filter(|ns| self.launcher.shows_namespace(&ns.name))
        {
            rows.push(self.builtin_row(
                ns.name.clone(),
                ns.name.clone().into(),
                Some(ns.name.clone()),
                0,
                true,
                cx,
            ));
            if self.sidebar_tree.open.contains(&ns.name) {
                for (group, _) in &GROUPS[..4] {
                    rows.extend(self.builtin_branch(group, Some(&ns.name), cx));
                }
                rows.extend(self.namespace_custom_rows(&ns.name, cx));
            }
        }
        rows
    }

    pub(super) fn builtin_row(
        &self,
        path: String,
        label: SharedString,
        namespace: Option<String>,
        depth: usize,
        expandable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_namespace = namespace.as_deref() == Some(path.as_str());
        let selected = self.sidebar_tree.selected.as_ref().map_or_else(
            || {
                if is_namespace {
                    self.page == Page::Resources
                        && self.application_workspace_name().is_none()
                        && self.scope.namespace == namespace
                } else {
                    depth == 0
                        && match label.as_ref() {
                            "RBAC" => self.page == Page::Rbac && !self.rbac.kind_location,
                            "Cluster Overview" => self.page == Page::ClusterOverview,
                            _ => false,
                        }
                }
            },
            |id| {
                id == &path
                    && matches!(
                        self.page,
                        Page::Resources | Page::ClusterOverview | Page::Rbac
                    )
            },
        );
        let expanded = self.sidebar_tree.open.contains(&path);
        let visual = if is_namespace {
            ("folder-tree", Tokens::muted_foreground())
        } else if expandable {
            match label.as_ref() {
                "Workloads" => ("boxes", Tokens::workload()),
                "Network" => ("network", Tokens::network()),
                "Config" => ("file-cog", Tokens::config()),
                "Storage" => ("hard-drive", Tokens::storage()),
                "RBAC" => ("shield", Tokens::secret()),
                "Cluster Overview" => ("server", Tokens::cluster()),
                _ => ("package", Tokens::muted_foreground()),
            }
        } else {
            match label.as_ref() {
                "Namespace Access" | "Cluster Roles" => ("shield", Tokens::secret()),
                "Roles" | "Service Accounts" => ("key-round", Tokens::secret()),
                "Bindings" => ("workflow", Tokens::secret()),
                _ => model::kind_visual(&label),
            }
        };
        let id = if is_namespace {
            format!("namespace:{path}")
        } else if depth == 0 {
            format!("nav:{path}")
        } else {
            format!("tree:{path}")
        };
        let toggle = path.clone();
        let target = path.clone();
        div()
            .id(SharedString::from(id))
            .test_support()
            .aria_label(label.clone())
            .aria_description(if selected { "selected" } else { "unselected" })
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(26.))
            .pl(px(8. + 16. * depth as f32))
            .pr(px(8.))
            .cursor_pointer()
            .text_color(Tokens::foreground().opacity(if selected { 1. } else { 0.8 }))
            .when(selected, |row| {
                row.bg(Tokens::primary().opacity(0.1)).child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(2.))
                        .rounded_r(px(2.))
                        .bg(Tokens::primary()),
                )
            })
            .hover(|row| row.bg(Tokens::card()))
            .child(
                div()
                    .id(SharedString::from(format!("tree-toggle:{path}")))
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .when(expandable, |row| {
                        row.child(ui::icon(
                            if expanded {
                                "chevron-down"
                            } else {
                                "chevron-right"
                            },
                            12.,
                            Tokens::muted_foreground(),
                        ))
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if !this.sidebar_tree.open.remove(&toggle) {
                                    this.sidebar_tree.open.insert(toggle.clone());
                                }
                                cx.notify();
                            },
                        ))
                    }),
            )
            .child(ui::icon(
                visual.0,
                if depth == 0 { 12. } else { 14. },
                visual.1,
            ))
            .child(
                ui::xs(label.clone())
                    .line_height(px(12.))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(depth == 0 && !is_namespace, |text| {
                        text.font_weight(gpui_kit::FontWeight::MEDIUM)
                    }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if is_namespace {
                    this.select_namespace(namespace.clone(), window, cx);
                    return;
                }
                if let Some(ns) = namespace.as_ref()
                    && target.starts_with(&format!("{ns}/Custom Resources"))
                {
                    if let Some(kind) = this.namespace_custom_target(ns, &target) {
                        this.select_namespace(Some(ns.clone()), window, cx);
                        this.live.selected = Some(kind);
                        this.live.kind_locked = true;
                        this.sidebar_tree.selected = Some(target.clone());
                        this.filter(cx);
                    }
                    return;
                }
                if target.starts_with("RBAC/") {
                    let view = RbacView::ALL
                        .into_iter()
                        .find(|view| view.label() == label.as_ref());
                    this.open_rbac(view, window, cx);
                    this.sidebar_tree.selected = Some(target.clone());
                } else if target == "RBAC" {
                    this.open_rbac(None, window, cx);
                    this.sidebar_tree.selected = Some(target.clone());
                } else if target == "Cluster Overview" {
                    this.select_namespace(None, window, cx);
                    this.open_cluster_overview(window, cx);
                    this.sidebar_tree.selected = Some(target.clone());
                } else if expandable && namespace.is_none() {
                    this.select_namespace(None, window, cx);
                    this.sidebar_tree.selected = Some(target.clone());
                } else {
                    let kinds = GROUPS
                        .iter()
                        .find(|(name, _)| *name == label.as_ref())
                        .map(|(_, kinds)| *kinds)
                        .unwrap_or_else(|| {
                            GROUPS
                                .iter()
                                .flat_map(|(_, kinds)| kinds.iter())
                                .find(|kind| **kind == label.as_ref())
                                .map(std::slice::from_ref)
                                .expect("builtin kind")
                        });
                    this.tree_select(target.clone(), namespace.clone(), kinds, window, cx);
                }
            }))
            .into_any_element()
    }
}
