//! Resource detail panel, mirroring `ResourceDetailPanel.svelte`. Each tab
//! lives in its own module; tabs load their data the first time they open.

use super::Inspector;
use super::actions_state::ActionsState;
use super::actions_tab::actions_tab;
use super::details_tab::details_tab;
use super::events::EventsState;
use super::events_tab::events_tab;
use super::logs_state::LogsState;
use super::logs_tab::logs_tab;
use super::revisions_tab::{RevisionsState, revisions_tab};
use super::yaml_apply::YamlApply;
use super::yaml_marks::YamlMarks;
use super::yaml_tab::yaml_tab;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::{EditorState, InputEvent};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, Subscription, Window, div, prelude::*, px,
};
use kubecove_lib::models::{ResourceDetailsFull, ResourceSummary};

pub(super) enum Detail {
    Closed,
    Loading(Box<ResourceSummary>),
    Failed(String),
    Ready(Box<ResourceDetail>),
}

impl Detail {
    /// The resource the panel is open for, which the ownership map selects.
    pub(super) fn resource(&self) -> Option<&ResourceSummary> {
        match self {
            Detail::Loading(resource) => Some(resource),
            Detail::Ready(detail) => Some(&detail.data.summary),
            Detail::Closed | Detail::Failed(_) => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DetailTab {
    Details,
    Events,
    Logs,
    Revisions,
    Actions,
    Yaml,
}

const TABS: &[(DetailTab, &str, &str)] = &[
    (DetailTab::Details, "show-details", "Details"),
    (DetailTab::Events, "show-events", "Events"),
    (DetailTab::Logs, "show-logs", "Logs"),
    (DetailTab::Revisions, "show-revisions", "Revisions"),
    (DetailTab::Actions, "show-actions", "Actions"),
    (DetailTab::Yaml, "show-yaml", "YAML"),
];

/// `canShowLogs` and `canShowRevisions`; the other ported tabs always show.
/// Exec and Forward are not ported yet.
fn available(tab: DetailTab, resource: &ResourceSummary) -> bool {
    let namespaced = resource.namespace.is_some();
    match tab {
        DetailTab::Logs => {
            namespaced && matches!(resource.kind.as_str(), "Pod" | "Deployment" | "Service")
        }
        DetailTab::Revisions => namespaced && resource.kind == "Deployment",
        _ => true,
    }
}

pub(super) struct ResourceDetail {
    pub data: ResourceDetailsFull,
    pub editor: Entity<EditorState>,
    pub marks: YamlMarks,
    pub tab: DetailTab,
    pub yaml: YamlApply,
    pub events: EventsState,
    pub logs: Option<LogsState>,
    pub revisions: Option<RevisionsState>,
    pub actions: Option<ActionsState>,
    _draft_changed: Subscription,
}

impl ResourceDetail {
    pub fn new(
        data: ResourceDetailsFull,
        window: &mut Window,
        cx: &mut Context<Inspector>,
    ) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .line_number(true)
                // The web marks indentation with tints instead of guide lines.
                .indent_guides(false)
                .default_value(data.yaml.clone())
        });
        let marks = YamlMarks::new(&editor, cx);
        let _draft_changed = cx.subscribe_in(&editor, window, |this, _, event, _, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            if let Detail::Ready(detail) = &mut this.detail
                && detail.yaml.draft_changed()
            {
                // A pending dry run reviewed the old text, so its result is dropped.
                this.diff_task.take();
            }
            cx.notify();
        });
        Self {
            data,
            editor,
            marks,
            tab: DetailTab::Details,
            yaml: YamlApply::default(),
            events: EventsState::new(window, cx),
            logs: None,
            revisions: None,
            actions: None,
            _draft_changed,
        }
    }

    pub fn dirty(&self, cx: &App) -> bool {
        self.yaml.editing() && self.editor.read(cx).value().as_ref() != self.yaml.base
    }
}

impl Inspector {
    fn set_tab(&mut self, tab: DetailTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            detail.tab = tab;
        }
        match tab {
            DetailTab::Logs => self.open_logs(window, cx),
            DetailTab::Revisions => self.open_revisions(window, cx),
            DetailTab::Actions => self.open_actions(window, cx),
            _ => {}
        }
        cx.notify();
    }
}

impl Inspector {
    pub(super) fn detail_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let (name, subtitle) = match &self.detail {
            Detail::Closed => return div().into_any_element(),
            Detail::Loading(summary) => (summary.name.clone(), summary.kind.clone()),
            Detail::Ready(detail) => {
                let summary = &detail.data.summary;
                (
                    summary.name.clone(),
                    format!(
                        "{} / {}",
                        summary.kind,
                        summary.namespace.as_deref().unwrap_or("Cluster")
                    ),
                )
            }
            Detail::Failed(_) => ("Resource".into(), String::new()),
        };
        let active = match &self.detail {
            Detail::Ready(detail) => Some(detail.tab),
            _ => None,
        };
        let resource = self.detail.resource().cloned();
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(8.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .min_w_0()
                    .child(ui::semibold(ui::sm(name)).truncate())
                    .child(ui::muted(ui::xs(subtitle)).truncate()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(ui::ghost_icon("pin", 24., 14.))
                    .child(
                        ui::ghost_icon("x", 24., 14.)
                            .id("close-detail")
                            .test_support()
                            .aria_label("Close resource details")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_detail(cx))),
                    ),
            );
        // `TabsList variant="line" class="flex-wrap"`: an `h-8 p-[3px] gap-1` row of
        // 25px `px-1.5 text-xs font-medium` triggers; narrow panes wrap tabs.
        let tabs =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .min_h(px(32.))
                .p(px(3.))
                .gap(px(4.))
                .children(
                    TABS.iter()
                        .filter(|(tab, ..)| {
                            resource
                                .as_ref()
                                .is_none_or(|resource| available(*tab, resource))
                        })
                        .map(|(tab, id, label)| {
                            let selected = active == Some(*tab);
                            let tab = *tab;
                            div()
                                .id(*id)
                                .test_support()
                                .relative()
                                .flex()
                                .items_center()
                                .h(px(25.))
                                .px(px(6.))
                                .cursor_pointer()
                                .child(ui::medium(ui::xs(*label)).text_color(if selected {
                                    Tokens::foreground()
                                } else {
                                    Tokens::foreground().opacity(0.6)
                                }))
                                .when(selected, |trigger| {
                                    trigger.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .right_0()
                                            .bottom(px(-4.))
                                            .h(px(2.))
                                            .bg(Tokens::foreground()),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_tab(tab, window, cx)
                                }))
                        }),
                );
        let body: AnyElement = match &self.detail {
            Detail::Closed => unreachable!("closed detail returned early"),
            Detail::Loading(_) => {
                ui::muted(ui::xs("Loading resource details...")).into_any_element()
            }
            Detail::Failed(error) => div()
                .id("detail-error")
                .role(gpui_kit::Role::Alert)
                .text_color(Tokens::destructive())
                .child(ui::xs(error.clone()))
                .into_any_element(),
            Detail::Ready(detail) => {
                let summary = &detail.data.summary;
                let identity = format!(
                    "{} {} {}",
                    summary.kind,
                    summary.namespace.as_deref().unwrap_or("Cluster"),
                    summary.name
                );
                let content: AnyElement = match detail.tab {
                    DetailTab::Details => details_tab(detail, cx).into_any_element(),
                    DetailTab::Events => events_tab(detail, cx),
                    DetailTab::Logs => logs_tab(detail, cx),
                    DetailTab::Revisions => revisions_tab(detail, cx),
                    DetailTab::Actions => actions_tab(detail, cx),
                    DetailTab::Yaml => yaml_tab(detail, cx).into_any_element(),
                };
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("detail-identity")
                            .test_support()
                            .aria_label(identity)
                            .absolute()
                            .size_0(),
                    )
                    .child(
                        div()
                            .id("detail-context")
                            .test_support()
                            .aria_label(summary.cluster.clone())
                            .absolute()
                            .size_0(),
                    )
                    .child(content)
                    .into_any_element()
            }
        };
        div()
            .id("detail")
            .flex()
            .flex_col()
            .size_full()
            // `ResourceInspector.svelte`: `section bg-surface-1`.
            .bg(Tokens::card())
            .child(header)
            // Tabs scroll with the body, as inside the web's `p-4` scroller (`Tabs gap-3`).
            .child(
                div()
                    .id("detail-body")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    // The web capture shows content 8px from the panel's right
                    // edge despite `p-4`; the rest of the inset is the panel frame.
                    .pt(px(16.))
                    .pb(px(16.))
                    .pl(px(16.))
                    .pr(px(8.))
                    .gap(px(12.))
                    .child(tabs)
                    .child(body),
            )
            .into_any_element()
    }
}
