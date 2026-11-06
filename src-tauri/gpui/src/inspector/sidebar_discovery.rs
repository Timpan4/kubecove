//! Custom-kind discovery for the sidebar: every discovered kind for the Custom Resources
//! section (the web's `list_resource_kinds` query), and namespace-owned kinds for namespace
//! branches (the web's expanded namespace query).
use super::Inspector;
use gpui_kit::{AnyElement, Context, Task, div, prelude::*, px};
use kubecove_lib::models::DiscoveredResourceKind;
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct Discovery {
    context: String,
    reads: HashMap<String, Read>,
    /// Cluster-wide discovered kinds, which list kinds even without objects.
    cluster: Option<ClusterKinds>,
}

struct Read {
    request: Rc<()>,
    task: Option<Task<()>>,
    result: Option<Result<Vec<DiscoveredResourceKind>, String>>,
}

struct ClusterKinds {
    task: Option<Task<()>>,
    result: Option<Result<Vec<DiscoveredResourceKind>, String>>,
}

/// The web's `resourceKindsPending`, `resourceKindsError` and kind list for the Custom Resources section.
pub(super) enum ClusterRead<'a> {
    Loading,
    Failed(&'a str),
    Ready(&'a [DiscoveredResourceKind]),
}

impl Inspector {
    pub(super) fn sync_sidebar_discovery(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.sidebar_tree.discovery;
        if state.context != self.scope.context {
            state.reads.clear();
            state.cluster = None;
            state.context = self.scope.context.clone();
        }
        state.reads.retain(|ns, _| {
            self.persisted.preferences.show_custom_resources && self.sidebar_tree.open.contains(ns)
        });
        if !self.persisted.preferences.show_custom_resources {
            return;
        }
        if self.sidebar_tree.discovery.cluster.is_none() {
            // The Svelte sidebar opts this query into `cancelledQueryRetry`: only a superseded
            // (cancelled) read retries, so a transport failure shows at once.
            let backend = self.backend.clone();
            let context = self.scope.context.clone();
            let executor = cx.background_executor().clone();
            let task = cx.spawn(async move |this, cx| {
                let result = crate::read_retry::cancelled_read(
                    move || backend.resource_kinds(context.clone()),
                    move |delay| executor.timer(delay),
                )
                .await
                .map_err(|error| error.message);
                let _ = this.update(cx, |this, cx| {
                    if let Some(cluster) = this.sidebar_tree.discovery.cluster.as_mut() {
                        cluster.result = Some(result);
                        cluster.task.take();
                        cx.notify();
                    }
                });
            });
            self.sidebar_tree.discovery.cluster = Some(ClusterKinds {
                task: Some(task),
                result: None,
            });
        }
        let state = &mut self.sidebar_tree.discovery;
        for ns in &self.namespaces {
            if !self.sidebar_tree.open.contains(&ns.name)
                || state.reads.contains_key(&ns.name)
                || !self.launcher.shows_namespace(&ns.name)
            {
                continue;
            }
            let namespace = ns.name.clone();
            let context = self.scope.context.clone();
            let scope = crate::backend::Scope {
                context: context.clone(),
                namespace: Some(namespace.clone()),
            };
            let read = self.backend.namespace_custom_kinds(scope);
            let request = Rc::new(());
            let owner = request.clone();
            let task = cx.spawn(async move |this, cx| {
                let result = read.await.map_err(|error| error.message);
                let _ = this.update(cx, |this, cx| {
                    let state = &mut this.sidebar_tree.discovery;
                    if state.context != context {
                        return;
                    }
                    if let Some(read) = state.reads.get_mut(&namespace) {
                        if !Rc::ptr_eq(&read.request, &owner) {
                            return;
                        }
                        read.result = Some(result);
                        read.task.take();
                        cx.notify();
                    }
                });
            });
            state.reads.insert(
                ns.name.clone(),
                Read {
                    request,
                    task: Some(task),
                    result: None,
                },
            );
        }
    }

    /// Cluster-wide kinds for the Custom Resources section; empty until discovery succeeds.
    pub(super) fn cluster_kinds(&self) -> &[DiscoveredResourceKind] {
        match self.cluster_read() {
            ClusterRead::Ready(kinds) => kinds,
            ClusterRead::Loading | ClusterRead::Failed(_) => &[],
        }
    }

    /// The cluster-wide kind read the Custom Resources section shows as loading, failed or ready.
    pub(super) fn cluster_read(&self) -> ClusterRead<'_> {
        match self
            .sidebar_tree
            .discovery
            .cluster
            .as_ref()
            .map(|cluster| cluster.result.as_ref())
        {
            None | Some(None) => ClusterRead::Loading,
            Some(Some(Err(message))) => ClusterRead::Failed(message),
            Some(Some(Ok(kinds))) => ClusterRead::Ready(kinds),
        }
    }

    fn namespace_kinds(&self, namespace: &str) -> &[DiscoveredResourceKind] {
        match self
            .sidebar_tree
            .discovery
            .reads
            .get(namespace)
            .and_then(|read| read.result.as_ref())
        {
            Some(Ok(kinds)) => kinds,
            _ => &[],
        }
    }

    pub(super) fn namespace_custom_target(
        &self,
        namespace: &str,
        path: &str,
    ) -> Option<DiscoveredResourceKind> {
        self.namespace_kinds(namespace)
            .iter()
            .find(|kind| {
                format!("{namespace}/Custom Resources/{}/{}", kind.group, kind.kind) == path
            })
            .cloned()
    }

    pub(super) fn namespace_custom_rows(
        &self,
        namespace: &str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if !self.persisted.preferences.show_custom_resources {
            return vec![];
        }
        let Some(read) = self.sidebar_tree.discovery.reads.get(namespace) else {
            return vec![];
        };
        let message = match &read.result {
            None => Some("Loading custom resources..."),
            Some(Err(_)) => Some("Custom resources failed to load"),
            Some(Ok(_)) => None,
        };
        if let Some(message) = message {
            return vec![
                div()
                    .pl(px(24.))
                    .h(px(26.))
                    .child(crate::ui::xs(message))
                    .into_any_element(),
            ];
        }
        let kinds = self.namespace_kinds(namespace);
        if kinds.is_empty() {
            return vec![];
        }
        let path = format!("{namespace}/Custom Resources");
        let mut rows = vec![self.builtin_row(
            path.clone(),
            "Custom Resources".into(),
            Some(namespace.into()),
            1,
            true,
            cx,
        )];
        if !self.sidebar_tree.open.contains(&path) {
            return rows;
        }
        let mut groups: Vec<_> = kinds
            .iter()
            .filter(|kind| kind.namespaced)
            .map(|kind| kind.group.as_str())
            .collect();
        groups.sort();
        groups.dedup();
        for group in groups {
            let group_path = format!("{path}/{group}");
            rows.push(self.builtin_row(
                group_path.clone(),
                group.to_owned().into(),
                Some(namespace.into()),
                2,
                true,
                cx,
            ));
            if !self.sidebar_tree.open.contains(&group_path) {
                continue;
            }
            let mut children: Vec<_> = kinds
                .iter()
                .filter(|kind| kind.namespaced && kind.group == group)
                .collect();
            children.sort_by(|a, b| a.kind.cmp(&b.kind));
            for kind in children {
                rows.push(self.builtin_row(
                    format!("{group_path}/{}", kind.kind),
                    kind.kind.clone().into(),
                    Some(namespace.into()),
                    3,
                    false,
                    cx,
                ));
            }
        }
        rows
    }
}
