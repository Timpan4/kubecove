//! Sidebar discovery search and resource-kind checkbox scope.
use super::{Inspector, live_data};
use crate::backend::Scope;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, SharedString, Subscription, Window, div, prelude::*,
    px,
};
use kubecove_lib::models::{DiscoveredResourceKind, WatchResourceKey};
use std::collections::HashSet;

pub(super) struct KindSelection {
    pub sidebar: Entity<InputState>,
    pub search: Entity<InputState>,
    pub chosen: Option<HashSet<String>>,
    pub reads: std::collections::HashMap<String, KindRead>,
    _subscriptions: Vec<Subscription>,
}

/// A newly checked kind's list read in every watched context. Its watches wait
/// until the rows land, so they start at their version.
pub(super) struct KindRead {
    /// The `watch_id` of the kind in each context read.
    ids: Vec<String>,
    loading: bool,
    _task: gpui_kit::Task<()>,
}

impl KindSelection {
    /// Whether a checked kind's rows for the watch `id` are still loading.
    pub(super) fn loading(&self, id: &str) -> bool {
        self.reads
            .values()
            .any(|read| read.loading && read.ids.iter().any(|other| other == id))
    }
}

pub(super) fn identity(kind: &DiscoveredResourceKind) -> String {
    format!("{}/{}/{}", kind.api_version, kind.plural, kind.kind)
}

impl KindSelection {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let sidebar =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search custom resources"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search kinds..."));
        let subscriptions = [&sidebar, &search].map(|input| {
            cx.subscribe(input, |_, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
        });
        Self {
            sidebar,
            search,
            chosen: None,
            reads: Default::default(),
            _subscriptions: subscriptions.into(),
        }
    }
}

impl Inspector {
    pub(super) fn custom_matches(&self, kind: &DiscoveredResourceKind, cx: &Context<Self>) -> bool {
        let query = self
            .kind_selection
            .sidebar
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        query.is_empty()
            || [&kind.kind, &kind.plural, &kind.group, &kind.api_version]
                .into_iter()
                .chain(kind.short_names.iter())
                .any(|value| value.to_lowercase().contains(&query))
    }

    pub(super) fn kind_selected(&self, key: &str) -> bool {
        let _span = crate::frame_profile::scope("kinds", "selected");
        self.kind_selection.chosen.as_ref().map_or_else(
            || {
                self.base_kinds().contains(&key)
                    || self.live.kinds.iter().any(|kind| {
                        identity(kind) == key
                            && self.observation.rows().iter().any(|row| {
                                row.kind == kind.kind
                                    && row.api_version.as_ref() == Some(&kind.api_version)
                            })
                    })
            },
            |chosen| chosen.contains(key),
        )
    }

    /// A row of a custom kind (a CRD in the catalog, or present in the scope)
    /// matches only that kind's API identity, so a built-in kind of the same
    /// name does not select it. Explicit selections resolve against the whole
    /// catalog, so a kind that had no objects at discovery shows its new rows.
    pub(super) fn selected_kind(&self, row_kind: &str, api_version: Option<&str>) -> bool {
        self.kind_selection.chosen.as_ref().is_none_or(|chosen| {
            let custom = api_version.and_then(|api| {
                self.live
                    .catalog
                    .iter()
                    .chain(&self.live.kinds)
                    .find(|kind| kind.kind == row_kind && kind.api_version == api)
            });
            match custom {
                Some(kind) => chosen.contains(&identity(kind)),
                None => chosen.contains(row_kind),
            }
        })
    }

    pub(super) fn available_kind_keys(&self) -> Vec<(String, String, WatchResourceKey)> {
        let mut keys: Vec<_> = self
            .picker_builtin_kinds()
            .iter()
            .map(|kind| {
                (
                    (*kind).to_owned(),
                    (*kind).to_owned(),
                    live_data::key(kind, self.scope.namespace.clone()),
                )
            })
            .collect();
        if self.persisted.preferences.show_custom_resources {
            keys.extend(self.live.catalog.iter().map(|kind| {
                (
                    identity(kind),
                    kind.kind.clone(),
                    crate::backend::discovered_key(kind, self.scope.namespace.clone()),
                )
            }));
        }
        keys
    }

    pub(super) fn picker_builtin_kinds(&self) -> &'static [&'static str] {
        // SUPPORTED_KINDS and CLUSTER_SCOPED_KINDS in src/lib/types.ts.
        &[
            "Pod",
            "Deployment",
            "ReplicaSet",
            "StatefulSet",
            "DaemonSet",
            "Service",
            "Ingress",
            "ConfigMap",
            "Secret",
            "PersistentVolumeClaim",
            "Job",
            "CronJob",
            "Node",
            "StorageClass",
            "PersistentVolume",
            "CustomResourceDefinition",
        ]
    }

    pub(super) fn toggle_kind(&mut self, key: String, cx: &mut Context<Self>) {
        let _span = crate::frame_profile::scope("kinds", "toggle");
        if !self.can_leave(cx) {
            return;
        }
        let all = self
            .available_kind_keys()
            .into_iter()
            .filter(|(key, _, _)| self.kind_selected(key))
            .map(|(key, _, _)| key)
            .collect();
        let chosen = self.kind_selection.chosen.get_or_insert(all);
        if !chosen.remove(&key) {
            chosen.insert(key.clone());
            self.read_selected_kind(key.clone(), cx);
        } else {
            self.kind_selection.reads.remove(&key);
        }
        self.close_detail(cx);
        self.table_page = 0;
        self.filter(cx);
    }

    fn read_selected_kind(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((_, _, key)) = self
            .available_kind_keys()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == id)
        else {
            return;
        };
        let scope = self.scope.clone();
        let contexts = self.watch_contexts();
        let ids = contexts
            .iter()
            .map(|context| crate::backend::watch_id(context, &key))
            .collect();
        let backend = self.backend.clone();
        let read_key = key.clone();
        let namespace = scope.namespace.clone();
        let executor = cx.background_executor().clone();
        // A backend can prepare a snapshot before returning its read future.
        let read = cx.background_executor().spawn(async move {
            let reads = contexts
                .into_iter()
                .map(|context| {
                    let backend = backend.clone();
                    let read_scope = Scope {
                        context: context.clone(),
                        namespace: namespace.clone(),
                    };
                    let read_key = read_key.clone();
                    let read = crate::read_retry::read(executor.clone(), move || {
                        backend.kind_resources(read_scope.clone(), read_key.clone(), Vec::new())
                    });
                    (context, read)
                })
                .collect();
            crate::workspaces::reads::settle(reads).await
        });
        let selected = id.clone();
        let task = cx.spawn(async move |this, cx| {
            let results = read.await;
            let _ = this.update(cx, |this, cx| {
                // The task is still in the map: removing an entry drops its task.
                if let Some(read) = this.kind_selection.reads.get_mut(&selected) {
                    read.loading = false;
                }
                if this.scope != scope || !this.kind_selected(&selected) {
                    return;
                }
                let several = results.len() > 1;
                for (context, result) in results {
                    match result {
                        Ok(crate::backend::KindRows {
                            rows,
                            resource_version,
                        }) => this.queue_live_patch(
                            super::resource_observation::Patch {
                                scope: Scope {
                                    context,
                                    namespace: scope.namespace.clone(),
                                },
                                key: key.clone(),
                                rows,
                                resource_version,
                                names: None,
                                watch: None,
                            },
                            cx,
                        ),
                        // A context whose rows don't show has nothing to call stale;
                        // its own read failure is already its warning.
                        Err(error) if this.observation.shows(&context) => {
                            this.live.refresh_error =
                                Some(super::resource_observation::context_message(
                                    several,
                                    &context,
                                    &error.message,
                                ))
                        }
                        Err(_) => {}
                    }
                }
                cx.notify();
            });
        });
        self.kind_selection.reads.insert(
            id,
            KindRead {
                ids,
                loading: true,
                _task: task,
            },
        );
    }

    fn select_all_kinds(&mut self, cx: &mut Context<Self>) {
        if !self.can_leave(cx) {
            return;
        }
        let options = self.available_kind_keys();
        let added: Vec<_> = options
            .iter()
            .filter(|(key, _, _)| !self.kind_selected(key))
            .map(|(key, _, _)| key.clone())
            .collect();
        self.kind_selection.chosen = Some(options.into_iter().map(|(key, _, _)| key).collect());
        for key in added {
            self.read_selected_kind(key, cx);
        }
        self.close_detail(cx);
        self.table_page = 0;
        self.filter(cx);
    }

    pub(super) fn custom_search_input(&self) -> AnyElement {
        div()
            .pl(px(24.))
            .pr(px(8.))
            .py(px(4.))
            .child(
                Input::new(&self.kind_selection.sidebar)
                    .id("custom-resource-search")
                    .aria_label("Search custom resources")
                    .h(px(28.))
                    .appearance(false)
                    .bg(Tokens::background())
                    .border_1()
                    .border_color(Tokens::border())
                    .rounded(px(2.))
                    .text_xs(),
            )
            .into_any_element()
    }

    pub(super) fn kind_picker_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self
            .kind_selection
            .search
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let options = self.available_kind_keys();
        let empty = !options
            .iter()
            .any(|(_, label, _)| label.to_lowercase().contains(&query));
        div()
            .w(px(344.))
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(ui::caps("Kinds", 10.))
            .child(
                ui::outline_button()
                    .id("select-all-kinds")
                    .test_support()
                    .aria_label("Select all kinds")
                    .h(px(32.))
                    .child(ui::xs("Select all kinds"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_all_kinds(cx);
                    })),
            )
            .child(
                Input::new(&self.kind_selection.search)
                    .id("kind-search")
                    .aria_label("Search resource kinds")
                    .h(px(32.))
                    .prefix(ui::icon("search", 16., Tokens::muted_foreground()))
                    .text_xs(),
            )
            .child(
                div()
                    .id("kind-options")
                    .test_support()
                    .h(px(224.))
                    .mt(px(4.))
                    .p(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::background().opacity(0.3))
                    .overflow_y_scroll()
                    .when(empty, |list| {
                        list.child(ui::muted(ui::xs("No matching kinds")).p(px(8.)))
                    })
                    .children(
                        options
                            .into_iter()
                            .filter(|(_, label, _)| label.to_lowercase().contains(&query))
                            .map(|(key, label, _)| {
                                let selected = self.kind_selected(&key);
                                div()
                                    .id(SharedString::from(format!("pick-kind:{key}")))
                                    .test_support()
                                    .role(gpui_kit::Role::CheckBox)
                                    .aria_label(label.clone())
                                    .aria_description(if selected {
                                        "checked"
                                    } else {
                                        "unchecked"
                                    })
                                    .h(px(32.))
                                    .px(px(8.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .cursor_pointer()
                                    .hover(|row| row.bg(Tokens::muted()))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_none()
                                            .items_center()
                                            .justify_center()
                                            .size(px(16.))
                                            .rounded(px(4.))
                                            .border_1()
                                            .border_color(Tokens::input())
                                            .bg(Tokens::input().opacity(0.3))
                                            .when(selected, |box_| {
                                                box_.child(ui::icon(
                                                    "check",
                                                    14.,
                                                    Tokens::foreground(),
                                                ))
                                            }),
                                    )
                                    .child(ui::xs(label))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_kind(key.clone(), cx)
                                    }))
                            }),
                    ),
            )
            .into_any_element()
    }
}
