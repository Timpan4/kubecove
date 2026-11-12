//! Discovered kinds retain their API identity even before they contain objects.
use super::{Inspector, Page, sidebar_discovery::ClusterRead};
use crate::backend::ScopeResult;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{AnyElement, Context, SharedString, deferred, div, prelude::*, px};
use kubecove_lib::models::AppError;

/// The custom-resource read that follows the built-in resource read.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum CustomLoad {
    /// No custom read was started for the current scope.
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

impl Inspector {
    /// The resource summary card's detail, as `customResourcesStatus` in
    /// `ResourceBrowserTopBar.svelte`. `None` renders as "available".
    pub(super) fn custom_status(&self) -> Option<&'static str> {
        if !self.persisted.preferences.show_custom_resources {
            return Some("Custom Resources off");
        }
        let custom = self.scope_load.custom();
        let phase = self
            .loading
            .phase("custom-resources-status", custom == CustomLoad::Loading);
        match custom {
            CustomLoad::Failed => Some("Custom Resources unavailable"),
            _ if phase.shown() => Some("Loading Custom Resources"),
            _ if !phase.ready() => None,
            CustomLoad::Loading => None,
            CustomLoad::Ready | CustomLoad::Idle if !self.live.kinds.is_empty() => {
                Some("Custom Resources added")
            }
            CustomLoad::Ready | CustomLoad::Idle => None,
        }
    }

    /// Adds a context's custom rows after its built-in rows have rendered,
    /// keeping the rows in context rank order.
    pub(super) fn apply_custom_resources(
        &mut self,
        context: &str,
        result: Result<ScopeResult, AppError>,
        cx: &mut Context<Self>,
    ) {
        let added = self.scope_load.custom_settled(context, result);
        if !added.is_empty() {
            let contexts = self.scope_load.contexts();
            let mut rows = self.rows.backend_rows();
            rows.extend(added);
            super::scope_load::rank_order(&mut rows, contexts);
            self.rows = std::rc::Rc::new(crate::model::PreparedRows::with_inherited_owners(
                rows, contexts,
            ));
        }
        self.warnings = self.scope_warnings(self.scope_load.warnings());
        self.prepare_rows();
        self.filter(cx);
        cx.notify();
    }

    /// The "All namespaces" Custom Resources row selects the custom kinds present
    /// in the workspace across every namespace, as a category row selects its
    /// kinds, and lists the kinds below it.
    pub(super) fn select_custom_resources(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.live.custom_open = true;
        // Leaving a namespace rediscovers the present kinds; `fill_custom_selection` runs when they land.
        self.select_namespace(None, window, cx);
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.kind_selection.chosen = Some(Default::default());
        self.sidebar_tree.selected = Some(CUSTOM_RESOURCES.to_owned());
        self.fill_custom_selection();
        self.close_detail(cx);
        self.filter(cx);
    }

    /// Checks the present custom kinds while the Custom Resources selection is
    /// still waiting for them. Returns whether it checked any.
    pub(super) fn fill_custom_selection(&mut self) -> bool {
        if self.sidebar_tree.selected.as_deref() == Some(CUSTOM_RESOURCES)
            && let Some(chosen) = &mut self.kind_selection.chosen
            && chosen.is_empty()
        {
            chosen.extend(self.live.kinds.iter().map(super::kind_selection::identity));
            return !chosen.is_empty();
        }
        false
    }
}

const CUSTOM_RESOURCES: &str = "Custom Resources";

/// The disabled loading and error rows in the Custom Resources section, as the web's
/// `SidebarTree` renders them. The web shows the discovery message as the row's tooltip, so
/// the native row keeps it as its accessible description rather than as visible text.
fn discovery_row(title: &'static str, description: Option<&str>) -> AnyElement {
    div()
        .pl(px(28.))
        .py(px(4.))
        .id(title)
        .test_support()
        .aria_label(title)
        .when_some(description, |row, message| {
            row.aria_description(message.to_owned())
        })
        .child(ui::xs(title))
        .into_any_element()
}

impl Inspector {
    pub(super) fn custom_kind_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if !self.persisted.preferences.show_custom_resources {
            return Vec::new();
        }
        let phase = self.loading.phase(
            "sidebar-custom-kinds",
            matches!(self.cluster_read(), ClusterRead::Loading),
        );
        match self.cluster_read() {
            _ if phase.shown() => {
                return vec![discovery_row("Loading custom resources...", None)];
            }
            _ if !phase.ready() => return Vec::new(),
            ClusterRead::Ready(_) => {}
            ClusterRead::Loading => return Vec::new(),
            ClusterRead::Failed(message) => {
                return vec![discovery_row(
                    "Custom resource discovery unavailable",
                    Some(message),
                )];
            }
        }
        let mut groups = self
            .cluster_kinds()
            .iter()
            .filter(|kind| self.custom_matches(kind, cx))
            .map(|kind| kind.group.clone())
            .collect::<Vec<_>>();
        groups.sort();
        groups.dedup();
        let mut rows = Vec::new();
        if groups.is_empty() {
            rows.push(
                div()
                    .pl(px(28.))
                    .py(px(4.))
                    .child(ui::xs(
                        if self
                            .kind_selection
                            .sidebar
                            .read(cx)
                            .value()
                            .trim()
                            .is_empty()
                        {
                            "No custom resources"
                        } else {
                            "No custom resources match search"
                        },
                    ))
                    .into_any_element(),
            );
        }
        for group in groups {
            let expanded = self.live.groups.contains(&group);
            let selected = group.clone();
            rows.push(
                div()
                    .id(SharedString::from(format!("custom-group:{group}")))
                    .test_support()
                    .aria_label(group.clone())
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .pl(px(28.))
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(ui::icon(
                        if expanded {
                            "chevron-down"
                        } else {
                            "chevron-right"
                        },
                        12.,
                        Tokens::muted_foreground(),
                    ))
                    .child(ui::xs(group.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.live.groups.remove(&selected) {
                            this.live.groups.insert(selected.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            if expanded {
                rows.extend(self.kind_rows(&group, cx));
            }
        }
        rows
    }

    fn kind_rows(&self, group: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.cluster_kinds()
            .iter()
            .filter(|kind| kind.group == group)
            .filter(|kind| self.custom_matches(kind, cx))
            .map(|kind| {
                let kind = kind.clone();
                let label = kind.kind.clone();
                div()
                    .id(SharedString::from(format!(
                        "custom-kind:{}:{}",
                        kind.api_version, kind.kind
                    )))
                    .test_support()
                    .aria_label(label.clone())
                    .aria_description(format!("{} / {}", kind.api_version, kind.plural))
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .pl(px(36.))
                    .pr(px(8.))
                    .cursor_pointer()
                    .hover(|row| row.bg(Tokens::muted()))
                    .child(ui::xs(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !this.can_leave(cx) {
                            return;
                        }
                        this.close_detail(cx);
                        if this.scope.namespace.is_some() {
                            this.select_namespace(None, window, cx);
                        }
                        this.set_page(Page::Resources);
                        this.palette.kind = None;
                        this.live.selected = Some(kind.clone());
                        this.kind_selection.chosen = None;
                        this.live.kind_locked = true;
                        this.live.picker_open = false;
                        this.filter(cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn kinds_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(kind) = self
            .sidebar_tree
            .selected
            .as_ref()
            .and_then(|path| path.rsplit('/').next())
            && self
                .kind_selection
                .chosen
                .as_ref()
                .is_some_and(|chosen| chosen.len() == 1 && chosen.contains(kind))
        {
            let label = SharedString::from(format!("Kind {kind}"));
            return ui::outline_badge(label.clone())
                .id("fixed-kind-scope")
                .test_support()
                .aria_label(label)
                .into_any_element();
        }
        if self.live.kind_locked
            && let Some(kind) = &self.live.selected
        {
            return ui::outline_badge(format!("Kind {}", kind.kind)).into_any_element();
        }
        div()
            .relative()
            .child(
                ui::outline_button()
                    .id("kinds-picker")
                    .test_support()
                    .aria_label("Kinds")
                    .h(px(32.))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(ui::muted(ui::xs("Kinds")))
                    .child(ui::xs(self.live.selected.as_ref().map_or_else(
                        || {
                            crate::model::kinds_label(
                                &self
                                    .available_kind_keys()
                                    .into_iter()
                                    .filter(|(key, _, _)| self.kind_selected(key))
                                    .map(|(_, label, _)| label)
                                    .collect::<Vec<_>>(),
                            )
                        },
                        |kind| kind.kind.clone(),
                    )))
                    .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.live.picker_open = !this.live.picker_open;
                        cx.notify();
                    })),
            )
            .when(self.live.picker_open, |picker| {
                picker.child(
                    deferred(
                        div()
                            .id("kind-popup")
                            .occlude()
                            .absolute()
                            .top(px(34.))
                            .left_0()
                            .min_w(px(240.))
                            .p(px(4.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::surface2())
                            .shadow_lg()
                            .child(self.kind_picker_options(cx)),
                    )
                    .with_priority(1),
                )
            })
            .into_any_element()
    }
}
