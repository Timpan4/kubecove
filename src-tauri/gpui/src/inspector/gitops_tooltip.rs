//! Source and revision disclosure from `surfaceTooltips.ts`.

use super::Inspector;
use crate::gitops_surface::Selection;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{ClipboardItem, Context, Div, SharedString, Stateful, prelude::*};

pub(super) fn source(selection: Selection) -> String {
    let Selection::App(app) = selection else {
        return selection.source_line().unwrap_or_default();
    };
    if app.sources.is_empty() {
        return app.source_repo.clone().unwrap_or_default();
    }
    let mut lines = vec![format!("Sources · {} total", app.sources.len())];
    for source in &app.sources {
        let mode = match source.source_mode.as_deref() {
            Some("git") => "Git",
            Some("helm") => "Helm",
            Some("plugin") => "Plugin",
            _ => "Unknown",
        };
        lines.push(mode.to_owned());
        for (label, value) in [
            ("repo", &source.repo_url),
            ("path", &source.path),
            ("chart", &source.chart),
            ("ref", &source.reference),
            ("target", &source.target_revision),
            ("resolved", &source.resolved_revision),
        ] {
            if let Some(value) = value {
                lines.push(format!("{label} {value}"));
            }
        }
    }
    lines.join("\n")
}

pub(super) fn revision(selection: Selection) -> String {
    let Selection::App(app) = selection else {
        return selection.revision_label().unwrap_or_default();
    };
    let mut rows = Vec::new();
    for source in &app.sources {
        let mut fields = Vec::new();
        if let Some(target) = &source.target_revision {
            fields.push(format!("target {target}"));
        }
        if let Some(resolved) = &source.resolved_revision {
            fields.push(format!("resolved {resolved}"));
        }
        let row = fields.join(" · ");
        if !row.is_empty() && !rows.contains(&row) {
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return app.source_revision.clone().unwrap_or_default();
    }
    format!("Revisions · {} unique\n{}", rows.len(), rows.join("\n"))
}

/// The list's Health tooltip: the state, the sources that decided it, then the
/// raw provider statuses.
pub(super) fn health(selection: Selection, label: &str) -> String {
    let mut lines = vec![
        label.to_owned(),
        crate::model::health_source(&selection.health().winning_sources),
    ];
    let statuses: Vec<String> = selection
        .badges()
        .into_iter()
        .map(|(label, value)| format!("{label}: {}", crate::gitops_surface::status_label(value)))
        .collect();
    if !statuses.is_empty() {
        lines.push(statuses.join(" · "));
    }
    lines.join("\n")
}

/// Shows `text` on hover.
pub(super) fn hint(element: Div, id: String, text: String) -> Stateful<Div> {
    element
        .id(SharedString::from(id))
        .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
}

/// Shows `text` on hover and copies it on click. The copy names `label` in the
/// shared copy toast, as `GitOpsView.svelte`'s `copyRevealedValue` does.
pub(super) fn reveal(
    element: Div,
    id: String,
    text: String,
    label: String,
    cx: &mut Context<Inspector>,
) -> Stateful<Div> {
    let copy = text.clone();
    element
        .id(SharedString::from(id))
        .cursor_pointer()
        .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
            this.show_copy_toast(format!("Copied {label}."), cx);
        }))
}
