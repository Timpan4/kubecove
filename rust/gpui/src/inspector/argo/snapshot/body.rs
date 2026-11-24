//! An article's body below its header, by `diffView` (the `{#if}` chain of
//! `ArgoApplicationDetails.svelte`), with `ArgoResourceDiff.svelte` for the
//! Changes view. Read-only YAML editors and diff lines both use the native
//! YAML syntax highlighting. `Body::of` decides what shows, for the renderers
//! here and for the editors `ensure_argo_editors` builds.

use super::{DiffView, toggle, update};
use crate::argo_app::{ArgoView, Change, ChangeBadge};
use crate::inspector::Inspector;
use crate::inspector::argo::tab::ArgoTab;
use crate::inspector::controls::{self, Variant};
use crate::inspector::delivery_ui as dui;
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Tokens};
use crate::ui;
use crate::unified_diff::DiffLine;
use gpui_kit::{AnyElement, Context, Div, SharedString, div, prelude::*, px};
use std::sync::Arc;

/// `ArgoResourceDiff` shows this many lines until expanded.
const DIFF_PREVIEW: usize = 24;

/// What an article shows below its header.
pub(in crate::inspector::argo) enum Body<'a> {
    /// A removal outside the Desired view, until Show diff expands it.
    Collapsed,
    /// The Desired view of a removal.
    NoDesired,
    Diff(&'a Arc<[DiffLine]>),
    /// A read-only YAML document: the live one, or else the desired one.
    Yaml {
        live: bool,
    },
    /// The view has no document for this change.
    Missing,
}

impl<'a> Body<'a> {
    pub(in crate::inspector::argo) fn of(
        view: DiffView,
        change: &'a Change,
        expanded: bool,
    ) -> Self {
        let removal = change.badge == ChangeBadge::Remove;
        if removal && view != DiffView::Target && !expanded {
            return Body::Collapsed;
        }
        match (view, change.diff.as_ref()) {
            (DiffView::Target, _) if removal => Body::NoDesired,
            (DiffView::Changes, Some(lines)) => Body::Diff(lines),
            (DiffView::Changes, None) if removal && !change.live.is_empty() => {
                Body::Yaml { live: true }
            }
            (DiffView::Target, _) if !change.desired.is_empty() => Body::Yaml { live: false },
            (DiffView::Live, _) if !change.live.is_empty() => Body::Yaml { live: true },
            _ => Body::Missing,
        }
    }
}

/// `border-t p-3 text-xs leading-relaxed`, with an optional medium title.
fn note(title: Option<&str>, text: &str) -> impl IntoElement {
    div()
        .id("argo-note")
        .test_support()
        .aria_label(title.unwrap_or(text).to_owned())
        .aria_description(text.to_owned())
        .p(px(12.))
        .border_t_1()
        .border_color(Tokens::border())
        .when_some(title, |note, title| {
            note.child(ui::medium(ui::text(title.to_owned(), 12., 19.5)))
        })
        .child(
            ui::muted(ui::text(text.to_owned(), 12., 19.5))
                .when(title.is_some(), |text| text.mt(px(4.))),
        )
}

/// Read-only YAML with the web editor's 180px minimum viewport.
fn yaml_text(index: usize, text: &SharedString, state: &ArgoTab, live: bool) -> AnyElement {
    div()
        .id(SharedString::from(format!("argo-yaml:{index}")))
        .min_h(px(180.))
        .p(px(12.))
        .border_t_1()
        .border_color(Tokens::border())
        .overflow_x_scroll()
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(18.))
        .whitespace_nowrap()
        .test_support()
        .aria_label(text.clone())
        .children(
            state
                .editors
                .get(index)
                .and_then(|editors| editors.get(live))
                .map(|editor| {
                    gpui_kit::component::input::Editor::new(editor)
                        .readonly(true)
                        .aria_label("Argo CD resource YAML")
                        .w_full()
                        .h(px(180.))
                }),
        )
        .into_any_element()
}

/// `ArgoResourceDiff.svelte` in its `overflow-x-auto border-t font-mono
/// text-[0.6875rem] leading-5` wrapper. A full diff builds only the rows the
/// detail body shows; its label is the whole diff, joined with the change.
fn resource_diff(
    index: usize,
    change: &Change,
    lines: &Arc<[DiffLine]>,
    full: bool,
    state: &ArgoTab,
    cx: &mut Context<Inspector>,
) -> Div {
    let highlights = state.highlights.get(index);
    let body = div()
        .id(SharedString::from(format!("argo-diff:{index}")))
        .test_support()
        .aria_label(change.diff_label.clone())
        .border_t_1()
        .border_color(Tokens::border())
        .font_family(MONO)
        .text_size(px(11.))
        .line_height(px(20.));
    let body = if full {
        body.child(controls::windowed_diff_rows(
            lines.clone(),
            20.,
            highlights.cloned(),
        ))
    } else {
        body.children(
            lines
                .iter()
                .take(DIFF_PREVIEW)
                .enumerate()
                .map(|(i, line)| {
                    controls::diff_row(
                        line,
                        20.,
                        highlights.and_then(|lines| lines.get(i)).map(Vec::as_slice),
                    )
                }),
        )
    };
    if lines.len() <= DIFF_PREVIEW {
        return div().child(body);
    }
    div().child(body).child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .font_family(crate::theme::SANS)
            .when(!full, |row| {
                // The count covers the whole diff, so only the preview is scanned here.
                let shown_changed = lines
                    .iter()
                    .take(DIFF_PREVIEW)
                    .filter(|line| line.kind.changed())
                    .count();
                row.child(
                    ui::muted(ui::xs(format!(
                        "{} more lines ({} changed) hidden.",
                        lines.len() - DIFF_PREVIEW,
                        change.changed - shown_changed
                    )))
                    .px(px(12.)),
                )
            })
            .child(
                dui::sm_button(
                    Variant::Ghost,
                    &if full {
                        "Show fewer lines".to_owned()
                    } else {
                        format!("Show all {} lines", lines.len())
                    },
                )
                .id(SharedString::from(format!("argo-diff-lines:{index}")))
                .test_support()
                .on_click(cx.listener(move |this, _, _, cx| {
                    update(this, cx, |state| toggle(&mut state.full, index))
                })),
            ),
    )
}

/// The article body below its header, or `None` while it is collapsed.
pub(super) fn article_body(
    view: &ArgoView,
    state: &ArgoTab,
    index: usize,
    change: &Change,
    cx: &mut Context<Inspector>,
) -> Option<AnyElement> {
    Some(match Body::of(state.view, change, state.expanded.contains(&index)) {
        Body::Collapsed => return None,
        Body::NoDesired => note(
            Some("No desired manifest"),
            "Selected revision no longer contains this resource.",
        )
        .into_any_element(),
        Body::Diff(lines) => resource_diff(
            index,
            change,
            lines,
            state.full.contains(&index),
            state,
            cx,
        )
        .into_any_element(),
        Body::Yaml { live } => yaml_text(
            index,
            if live { &change.live } else { &change.desired },
            state,
            live,
        ),
        Body::Missing => note(
            None,
            &if change.inexact || view.kubernetes {
                "Exact target and live manifests are unavailable through Kubernetes fallback. Select a connected Argo CD profile for manifest comparison.".to_owned()
            } else {
                format!(
                    "No {} manifest reported.",
                    match state.view {
                        DiffView::Target => "desired",
                        DiffView::Live => "live",
                        DiffView::Changes => "comparison",
                    }
                )
            },
        )
        .into_any_element(),
    })
}
