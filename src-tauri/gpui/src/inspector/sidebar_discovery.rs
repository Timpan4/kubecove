//! Custom-kind discovery for the sidebar: every discovered kind for the Custom Resources
//! section (the web's `list_resource_kinds` query), and namespace-owned kinds for namespace
//! branches (the web's expanded namespace query).
use super::{Inspector, request::Fetch};
use gpui_kit::{AnyElement, Context, div, prelude::*, px};
use kubecove_lib::models::DiscoveredResourceKind;
use std::collections::HashMap;

/// Each read owns its task, so removing a namespace's entry or changing the
/// context cancels the read, and a result only settles into its own entry.
#[derive(Default)]
pub(super) struct Discovery {
    context: String,
    /// Namespace-owned kinds for each open namespace branch.
    reads: HashMap<String, Fetch<Vec<DiscoveredResourceKind>>>,
    /// Cluster-wide discovered kinds, which list kinds even without objects.
    /// `Idle` until the first sync with an open workspace.
    cluster: Fetch<Vec<DiscoveredResourceKind>>,
}

fn settled(
    result: Result<Vec<DiscoveredResourceKind>, kubecove_lib::models::AppError>,
) -> Fetch<Vec<DiscoveredResourceKind>> {
    match result {
        Ok(kinds) => Fetch::Ready(kinds),
        Err(error) => Fetch::Failed(error.message),
    }
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
            state.cluster = Fetch::Idle;
            state.context = self.scope.context.clone();
        }
        state.reads.retain(|ns, _| {
            self.persisted.preferences.show_custom_resources && self.sidebar_tree.open.contains(ns)
        });
        if !self.persisted.preferences.show_custom_resources || !self.workspace_open() {
            return;
        }
        if matches!(self.sidebar_tree.discovery.cluster, Fetch::Idle) {
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
                .await;
                let _ = this.update(cx, |this, cx| {
                    this.sidebar_tree.discovery.cluster = settled(result);
                    cx.notify();
                });
            });
            self.sidebar_tree.discovery.cluster = Fetch::start(task);
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
            let task = cx.spawn(async move |this, cx| {
                let result = read.await;
                let _ = this.update(cx, |this, cx| {
                    let state = &mut this.sidebar_tree.discovery;
                    if state.context != context {
                        return;
                    }
                    if let Some(read) = state.reads.get_mut(&namespace) {
                        *read = settled(result);
                        cx.notify();
                    }
                });
            });
            state.reads.insert(ns.name.clone(), Fetch::start(task));
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
        match &self.sidebar_tree.discovery.cluster {
            Fetch::Idle | Fetch::Loading { .. } => ClusterRead::Loading,
            Fetch::Failed(message) => ClusterRead::Failed(message),
            Fetch::Ready(kinds) => ClusterRead::Ready(kinds),
        }
    }

    fn namespace_kinds(&self, namespace: &str) -> &[DiscoveredResourceKind] {
        self.sidebar_tree
            .discovery
            .reads
            .get(namespace)
            .and_then(Fetch::ready)
            .map_or(&[], Vec::as_slice)
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
        let phase = self.loading.phase(
            &format!("sidebar-namespace-kinds:{namespace}"),
            read.loading(),
        );
        let message = match read {
            _ if phase.shown() => Some("Loading custom resources..."),
            _ if !phase.ready() => return vec![],
            Fetch::Idle | Fetch::Loading { .. } => return vec![],
            Fetch::Failed(_) => Some("Custom resources failed to load"),
            Fetch::Ready(_) => None,
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
