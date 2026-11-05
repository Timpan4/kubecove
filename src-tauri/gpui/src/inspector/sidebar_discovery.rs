//! Namespace-owned custom-kind reads, matching the web's expanded namespace query.
use super::Inspector;
use gpui_kit::{AnyElement, Context, Task, div, prelude::*, px};
use kubecove_lib::models::DiscoveredResourceKind;
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct Discovery {
    context: String,
    reads: HashMap<String, Read>,
}

struct Read {
    request: Rc<()>,
    task: Option<Task<()>>,
    result: Option<Result<Vec<DiscoveredResourceKind>, String>>,
}

impl Inspector {
    pub(super) fn sync_sidebar_discovery(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.sidebar_tree.discovery;
        if state.context != self.scope.context {
            state.reads.clear();
            state.context = self.scope.context.clone();
        }
        state.reads.retain(|ns, _| {
            self.persisted.preferences.show_custom_resources && self.sidebar_tree.open.contains(ns)
        });
        if !self.persisted.preferences.show_custom_resources {
            return;
        }
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
