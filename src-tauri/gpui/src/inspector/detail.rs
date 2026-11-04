//! Resource detail panel, mirroring `ResourceDetailPanel.svelte` and
//! `YamlTab.svelte`. Drafts are local only. The Details tab is in `details_tab`.

use super::Inspector;
use super::details_tab::details_tab;
use super::events::EventsState;
use super::events_tab::events_tab;
use super::yaml_marks::YamlMarks;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::{
    diff::{Diff, DiffFile, DiffState},
    input::{Editor, EditorState},
};
use gpui_kit::{AnyElement, App, AppContext, Context, Div, Entity, Window, div, prelude::*, px};
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
    Diff,
}

const TABS: &[(DetailTab, &str, &str)] = &[
    (DetailTab::Details, "show-details", "Details"),
    (DetailTab::Events, "show-events", "Events"),
    (DetailTab::Logs, "show-logs", "Logs"),
    (DetailTab::Revisions, "show-revisions", "Revisions"),
    (DetailTab::Actions, "show-actions", "Actions"),
    (DetailTab::Yaml, "show-yaml", "YAML"),
];

pub(super) struct ResourceDetail {
    pub data: ResourceDetailsFull,
    pub editor: Entity<EditorState>,
    marks: YamlMarks,
    pub tab: DetailTab,
    pub editing: bool,
    pub diff: Option<Entity<DiffState>>,
    pub events: EventsState,
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
        Self {
            data,
            editor,
            marks,
            tab: DetailTab::Details,
            editing: false,
            diff: None,
            events: EventsState::new(window, cx),
        }
    }

    pub fn dirty(&self, cx: &App) -> bool {
        self.editing && self.editor.read(cx).value().as_ref() != self.data.yaml
    }
}

impl Inspector {
    pub(super) fn show_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let before = detail.data.yaml.clone();
        let after = detail.editor.read(cx).value().to_string();
        self.diff_task.take();
        let request = self.detail_request.clone();
        let task = cx.background_executor().spawn(async move {
            let patch = similar::TextDiff::from_lines(&before, &after)
                .unified_diff()
                .header("resource.yaml", "resource.yaml")
                .to_string();
            DiffFile::parse(&patch).map_err(|error| error.to_string())
        });
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if !std::rc::Rc::ptr_eq(&this.detail_request, &request) {
                    return;
                }
                match result {
                    Ok(files) => {
                        if let Detail::Ready(detail) = &mut this.detail {
                            detail.diff = Some(cx.new(|cx| DiffState::new(files, cx)));
                            detail.tab = DetailTab::Diff;
                        }
                    }
                    Err(error) => {
                        this.notice = Some(format!("Could not prepare local diff: {error}"))
                    }
                }
                cx.notify();
            });
        }));
    }

    fn set_tab(&mut self, tab: DetailTab, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            detail.tab = tab;
        }
        cx.notify();
    }

    fn yaml_tab(&self, detail: &ResourceDetail, cx: &mut Context<Self>) -> Div {
        let summary = &detail.data.summary;
        let path = [
            Some(summary.cluster.as_str()),
            summary.namespace.as_deref(),
            Some(summary.kind.as_str()),
            Some(summary.name.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ");
        let picker = |label: &str, value: &str| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::muted(ui::xs(label.to_owned())))
                .child(
                    ui::outline_button()
                        .h(px(28.))
                        .w(px(140.))
                        .px(px(8.))
                        .justify_between()
                        .child(ui::xs(value.to_owned()))
                        .child(ui::icon("chevron-down", 14., Tokens::muted_foreground())),
                )
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(10.))
            .child(div().flex().child(ui::outline_badge(path)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(picker("YAML shape", "Kubectl view"))
                    .child(picker("Encoding", "YAML")),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .when(!detail.editing, |row| {
                        row.child(
                            ui::secondary_button()
                                .id("edit-draft")
                                .test_support()
                                .cursor_pointer()
                                .h(px(28.))
                                .px(px(8.))
                                .child(ui::icon("pencil", 14., Tokens::foreground()))
                                .child(ui::medium(ui::xs("Edit YAML")))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Detail::Ready(detail) = &mut this.detail {
                                        detail.editing = true;
                                        detail.tab = DetailTab::Yaml;
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .when(detail.editing, |row| {
                        row.child(
                            ui::outline_button()
                                .id("show-diff")
                                .test_support()
                                .cursor_pointer()
                                .h(px(28.))
                                .px(px(8.))
                                .child(ui::medium(ui::xs("Compare local draft")))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.show_diff(window, cx)),
                                ),
                        )
                        .child(
                            ui::outline_button()
                                .id("discard-draft")
                                .test_support()
                                .cursor_pointer()
                                .h(px(28.))
                                .px(px(8.))
                                .child(ui::medium(ui::xs("Discard local draft")))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.diff_task.take();
                                    if let Detail::Ready(detail) = &mut this.detail {
                                        detail.editor.update(cx, |editor, cx| {
                                            editor.set_value(detail.data.yaml.clone(), window, cx)
                                        });
                                        detail.marks.refresh(&detail.editor, cx);
                                        detail.editing = false;
                                        detail.diff = None;
                                        detail.tab = DetailTab::Yaml;
                                    }
                                    this.notice = None;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(detail.editing, |pane| {
                let notice = "This draft is local. Changes cannot be applied to the cluster.";
                pane.child(
                    div()
                        .id("draft-notice")
                        .test_support()
                        .aria_label(notice)
                        .child(ui::muted(ui::xs(notice))),
                )
            })
            .child(match (&detail.tab, &detail.diff) {
                // The diff replaces the editor; draft controls and notice stay visible.
                (DetailTab::Diff, Some(diff)) => div()
                    .id("draft-diff")
                    .test_support()
                    .flex_1()
                    .min_h(px(320.))
                    .child(Diff::new(diff).size_full())
                    .into_any_element(),
                // `YamlCodeEditor minHeight`: 520px read-only, 420px while editing. GPUI's
                // editor needs a definite height, so longer YAML scrolls inside it.
                _ => div()
                    .h(px(if detail.editing { 420. } else { 520. }))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .overflow_hidden()
                    .child(
                        Editor::new(&detail.editor)
                            .aria_label("Resource YAML")
                            .readonly(!detail.editing)
                            .size_full(),
                    )
                    .into_any_element(),
            })
    }

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
        let tabs = div()
            .flex()
            .flex_wrap()
            .items_center()
            .min_h(px(32.))
            .p(px(3.))
            .gap(px(4.))
            .children(TABS.iter().map(|(tab, id, label)| {
                let selected = active == Some(*tab)
                    || (active == Some(DetailTab::Diff) && *tab == DetailTab::Yaml);
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
                    .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)))
            }));
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
                    DetailTab::Yaml | DetailTab::Diff => {
                        self.yaml_tab(detail, cx).into_any_element()
                    }
                    _ => ui::muted(ui::xs("Not available in the native client yet."))
                        .into_any_element(),
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
                    .p(px(16.))
                    .gap(px(12.))
                    .child(tabs)
                    .child(body),
            )
            .into_any_element()
    }
}
