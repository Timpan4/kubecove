//! Built-in sidebar branches. Descendants are constructed only for open parents.
use super::{Inspector, Page};
use crate::{model, rbac::RbacView, theme::Tokens, ui};
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, FocusHandle, KeyDownEvent, ScrollHandle, SharedString,
    TestSupportExt, Window, div, prelude::*, px,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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
    /// Shared by the desktop sidebar and the drawer, which never render together,
    /// so the tree keeps its scroll position when the drawer closes.
    pub scroll: ScrollHandle,
    /// One focus handle per row path, so keyboard focus survives re-renders.
    focus: RefCell<HashMap<String, FocusHandle>>,
}

impl TreeState {
    pub(super) fn focus_handle(&self, path: &str, cx: &mut App) -> FocusHandle {
        self.focus
            .borrow_mut()
            .entry(path.to_owned())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }
}

/// What a click or Enter or Space press on a built-in row activates.
#[derive(Clone)]
struct RowTarget {
    path: String,
    label: SharedString,
    namespace: Option<String>,
    is_namespace: bool,
    expandable: bool,
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

    /// Activates a row the way a click does. Enter and Space select without
    /// toggling, as the web's `selectNode` does.
    fn activate_row(&mut self, row: RowTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.can_leave(cx) {
            self.navigation_open = false;
        }
        let RowTarget {
            path: target,
            label,
            namespace,
            is_namespace,
            expandable,
        } = row;
        if is_namespace {
            self.select_namespace(namespace, window, cx);
            return;
        }
        if let Some(ns) = namespace.as_ref()
            && target.starts_with(&format!("{ns}/Custom Resources"))
        {
            if let Some(kind) = self.namespace_custom_target(ns, &target) {
                self.select_namespace(Some(ns.clone()), window, cx);
                self.live.selected = Some(kind);
                self.live.kind_locked = true;
                self.sidebar_tree.selected = Some(target.clone());
                self.filter(cx);
            }
            return;
        }
        if target.starts_with("RBAC/") {
            let view = RbacView::ALL
                .into_iter()
                .find(|view| view.label() == label.as_ref());
            self.open_rbac(view, window, cx);
            self.sidebar_tree.selected = Some(target.clone());
        } else if target == "RBAC" {
            self.open_rbac(None, window, cx);
            self.sidebar_tree.selected = Some(target.clone());
        } else if target == "Cluster Overview" {
            self.select_namespace(None, window, cx);
            self.open_cluster_overview(window, cx);
            self.sidebar_tree.selected = Some(target.clone());
        } else if expandable && namespace.is_none() {
            self.select_namespace(None, window, cx);
            self.sidebar_tree.selected = Some(target.clone());
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
            self.tree_select(target.clone(), namespace.clone(), kinds, window, cx);
        }
    }

    fn toggle_tree_row(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.sidebar_tree.open.remove(path) {
            self.sidebar_tree.open.insert(path.to_owned());
        }
        cx.notify();
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
        // A collapsed branch that holds the selection shows it in its label.
        let hides_selection = !expanded
            && self
                .sidebar_tree
                .selected
                .as_deref()
                .is_some_and(|selected| selected.starts_with(&format!("{path}/")));
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
        let focus = self.sidebar_tree.focus_handle(&path, cx);
        let chevron = path.clone();
        let key_toggle = path.clone();
        let target = RowTarget {
            path,
            label: label.clone(),
            namespace,
            is_namespace,
            expandable,
        };
        let click_row = target;
        div()
            .id(SharedString::from(id))
            .test_support()
            .track_focus(&focus)
            .tab_index(0)
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
                    .id(SharedString::from(format!("tree-toggle:{chevron}")))
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
                                this.toggle_tree_row(&chevron, cx);
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
                    })
                    .when(hides_selection, |text| text.text_color(Tokens::primary())),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.activate_row(click_row.clone(), window, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "right" if expandable && !expanded => {
                        this.toggle_tree_row(&key_toggle, cx);
                    }
                    "left" if expandable && expanded => {
                        this.toggle_tree_row(&key_toggle, cx);
                    }
                    _ => {}
                }
            }))
            .into_any_element()
    }
}
