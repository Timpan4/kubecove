//! Revisions tab of a Deployment's detail panel, mirroring
//! `DeploymentRevisionsTab.svelte` and `deployment-revisions-model.ts` at the
//! `lg:` two-column layout.

use super::Inspector;
use super::controls::diff_pre;
use super::detail::{Detail, ResourceDetail};
use crate::targets::ObservedElement;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use crate::unified_diff::{self, DiffLine};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::{
    AnyElement, AppContext, Context, Div, Entity, SharedString, Stateful, Subscription, Task,
    Window, div, prelude::*, px,
};
use kubecove_lib::models::DeploymentRevision;

type Choices = SelectState<SearchableVec<RevisionChoice>>;

/// A comparison option keyed by ReplicaSet name, as the web `SelectItem` value is:
/// two ReplicaSets can carry the same revision number and so the same label.
#[derive(Clone)]
struct RevisionChoice {
    name: SharedString,
    title: SharedString,
}
impl gpui_kit::component::searchable_list::SearchableListItem for RevisionChoice {
    type Value = SharedString;
    fn value(&self) -> &SharedString {
        &self.name
    }
    fn title(&self) -> SharedString {
        self.title.clone()
    }
}

pub(super) enum RevisionsLoad {
    Pending,
    Failed,
    Ready(Vec<DeploymentRevision>),
}

pub(super) struct RevisionsState {
    load: RevisionsLoad,
    selected: Option<String>,
    comparison: Option<String>,
    picker: Option<Entity<Choices>>,
    diff: Vec<DiffLine>,
    /// The diff text, which tests read as the pane's label.
    diff_text: SharedString,
    _task: Option<Task<()>>,
    _subscription: Option<Subscription>,
}

/// `revisionLabel`.
fn label(revision: &DeploymentRevision) -> String {
    revision
        .revision
        .map_or(revision.name.clone(), |number| format!("Revision {number}"))
}

impl RevisionsState {
    fn revisions(&self) -> &[DeploymentRevision] {
        match &self.load {
            RevisionsLoad::Ready(revisions) => revisions,
            _ => &[],
        }
    }

    fn find(&self, name: Option<&str>) -> Option<&DeploymentRevision> {
        self.revisions()
            .iter()
            .find(|revision| Some(revision.name.as_str()) == name)
    }

    /// `reconcileRevisionSelection`, then the comparison options and diff.
    fn reconcile(&mut self, window: &mut Window, cx: &mut Context<Inspector>) {
        let revisions = self.revisions();
        let selected = if self.find(self.selected.as_deref()).is_some() {
            self.selected.clone()
        } else {
            revisions.first().map(|revision| revision.name.clone())
        };
        let comparison = if self
            .find(self.comparison.as_deref())
            .is_some_and(|revision| Some(&revision.name) != selected.as_ref())
        {
            self.comparison.clone()
        } else {
            revisions
                .iter()
                .find(|revision| Some(&revision.name) != selected.as_ref())
                .map(|revision| revision.name.clone())
        };
        let options: Vec<RevisionChoice> = revisions
            .iter()
            .filter(|revision| Some(&revision.name) != selected.as_ref())
            .map(|revision| RevisionChoice {
                name: revision.name.clone().into(),
                title: label(revision).into(),
            })
            .collect();
        let chosen = comparison.clone().map(SharedString::from);
        self.diff = match (
            self.find(selected.as_deref()),
            self.find(comparison.as_deref()),
        ) {
            (Some(selected), Some(comparison)) => {
                unified_diff::compact(&comparison.pod_template_yaml, &selected.pod_template_yaml)
            }
            _ => Vec::new(),
        };
        self.diff_text = self
            .diff
            .iter()
            .map(|line| line.text.as_ref())
            .collect::<Vec<_>>()
            .join("\n")
            .into();
        self.selected = selected;
        self.comparison = comparison;
        if let Some(picker) = &self.picker {
            picker.update(cx, |picker, cx| {
                picker.set_items(SearchableVec::new(options), window, cx);
                if let Some(chosen) = &chosen {
                    picker.set_selected_value(chosen, window, cx);
                }
            });
        }
    }
}

impl Inspector {
    fn revisions_state(&mut self) -> Option<&mut RevisionsState> {
        match &mut self.detail {
            Detail::Ready(detail) => detail.revisions.as_mut(),
            _ => None,
        }
    }

    /// Loads the revisions the first time the tab opens, and again after a failed
    /// read, as the web query refetches its error when the tab mounts again.
    pub(super) fn open_revisions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail
            .revisions
            .as_ref()
            .is_some_and(|state| !matches!(state.load, RevisionsLoad::Failed))
        {
            return;
        }
        let picker = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<RevisionChoice>::new()),
                None,
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(&picker, window, |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(chosen)) = event else {
                return;
            };
            if let Some(state) = this.revisions_state() {
                let name = state
                    .find(Some(chosen.as_ref()))
                    .map(|revision| revision.name.clone());
                if name.is_some() && name != state.comparison {
                    state.comparison = name;
                    state.reconcile(window, cx);
                    cx.notify();
                }
            }
        });
        let read = backend.deployment_revisions(detail.data.summary.clone());
        let read = self.timings.measure("deployment_revisions", read);
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(state) = this.revisions_state() {
                    state.load = match result {
                        Ok(revisions) => RevisionsLoad::Ready(revisions),
                        Err(_) => RevisionsLoad::Failed,
                    };
                    state.reconcile(window, cx);
                }
                cx.notify();
            });
        });
        detail.revisions = Some(RevisionsState {
            load: RevisionsLoad::Pending,
            selected: None,
            comparison: None,
            picker: Some(picker),
            diff: Vec::new(),
            diff_text: SharedString::default(),
            _task: Some(task),
            _subscription: Some(subscription),
        });
    }
}

fn note(text: &str) -> Div {
    ui::sm(text.to_owned()).p(px(16.))
}

fn revision_button(
    revision: &DeploymentRevision,
    selected: bool,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let text = label(revision);
    let name = revision.name.clone();
    div()
        .id(SharedString::from(format!("revision:{text}")))
        .test_support()
        .aria_label(text.clone())
        .cursor_pointer()
        .w_full()
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(if selected {
            Tokens::primary()
        } else {
            Tokens::border()
        })
        .when(selected, |button| {
            button.bg(Tokens::primary().opacity(0.05))
        })
        .when(!selected, |button| {
            button.hover(|button| button.bg(Tokens::muted().opacity(0.5)))
        })
        .child(ui::medium(ui::sm(text)))
        .child(
            ui::muted(ui::xs(
                revision
                    .change_cause
                    .clone()
                    .unwrap_or_else(|| revision.name.clone()),
            ))
            .mt(px(4.))
            .truncate(),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            if let Some(state) = this.revisions_state()
                && state.selected.as_ref() != Some(&name)
            {
                state.selected = Some(name.clone());
                state.reconcile(window, cx);
                cx.notify();
            }
        }))
}

pub(super) fn revisions_tab(
    detail: &ResourceDetail,
    loading: &super::loading::LoadingDelays,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let Some(state) = detail.revisions.as_ref() else {
        return div().into_any_element();
    };
    let phase = loading.phase("revisions", matches!(state.load, RevisionsLoad::Pending));
    let revisions = match &state.load {
        // DeploymentRevisionsTab.svelte checks the loading state first.
        _ if phase.shown() => {
            return ui::muted(note("Loading Deployment revisions…")).into_any_element();
        }
        _ if !phase.ready() => return div().into_any_element(),
        RevisionsLoad::Pending => return div().into_any_element(),
        RevisionsLoad::Failed => {
            return note("Unable to load Deployment revisions.")
                .id("revisions-error")
                .test_support()
                .text_color(Tokens::destructive())
                .into_any_element();
        }
        RevisionsLoad::Ready(revisions) if revisions.is_empty() => {
            return ui::muted(note("No controller-owned ReplicaSet revisions were found."))
                .id("revisions-empty")
                .test_support()
                .into_any_element();
        }
        RevisionsLoad::Ready(revisions) => revisions,
    };
    let selected = state.find(state.selected.as_deref());
    let comparison = state.find(state.comparison.as_deref());
    let pane = div()
        .flex_1()
        .min_w_0()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border());
    let pane = match (selected, comparison, &state.picker) {
        _ if revisions.len() == 1 => pane.child(ui::muted(note(
            "Only one revision is available; there is no other pod template to compare.",
        ))),
        (Some(selected), Some(comparison), Some(picker)) => pane
            .child(
                div()
                    .id("revision-header")
                    .test_support()
                    .aria_label(format!(
                        "{} compared with {}",
                        label(selected),
                        label(comparison)
                    ))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(ui::medium(ui::sm(label(selected))))
                    .child(ui::muted(ui::sm("compared with")))
                    .child(
                        Select::new(picker)
                            .id("revision-comparison")
                            // `h-8 w-44`, which the web capture renders 28px tall;
                            // the wrapped row shrinks it to the pane.
                            .h(px(28.))
                            .text_xs()
                            .w(px(176.))
                            .max_w_full(),
                    ),
            )
            .child(
                div()
                    .id("revision-diff")
                    .test_support()
                    .aria_label(state.diff_text.clone())
                    .child(diff_pre("revision-diff-lines", &state.diff)),
            ),
        _ => pane,
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .p(px(4.))
        .child(
            div()
                .w(px(240.))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(revisions.iter().map(|revision| {
                    let active = selected.is_some_and(|selected| selected.name == revision.name);
                    revision_button(revision, active, cx)
                })),
        )
        .child(pane)
        .into_any_element()
}
