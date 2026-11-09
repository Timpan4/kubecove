//! Saved scope and comparison projections are rebuilt on reads, never per frame.
use super::{Inspector, overview::titled_card};
use crate::targets::TestSupportExt;
use crate::{
    model::{HealthSummary, WORKSPACE_KINDS},
    ui,
    workspaces::shortcuts::{CompareKind, compare_entries},
};
use gpui_kit::{AnyElement, SharedString, div, prelude::*, px};

#[derive(Default)]
pub(super) struct SavedOverview {
    pub(super) shortcuts: Vec<crate::workspaces::shortcuts::Shortcut>,
    scope: [String; 3],
    warnings: Vec<String>,
    comparisons: Vec<Comparison>,
}
struct Comparison {
    title: String,
    sides: [(String, HealthSummary); 2],
}

impl Inspector {
    pub(super) fn update_saved_overview(&mut self) {
        let saved = self
            .launcher
            .active
            .and_then(|id| self.launcher.saved.iter().find(|saved| saved.id == id));
        let scope = if let Some(saved) = saved {
            let contexts = saved.contexts();
            [
                if contexts.len() > 1 {
                    format!(
                        "{} ({})",
                        saved.group_name.as_deref().unwrap_or("Cluster group"),
                        contexts.len()
                    )
                } else {
                    saved.context.clone()
                },
                if saved.namespaces.is_empty() {
                    "All namespaces".into()
                } else {
                    format!("{} namespaces", saved.namespaces.len())
                },
                saved.kinds.join(", "),
            ]
        } else {
            [
                self.scope.context.clone(),
                "All namespaces".into(),
                WORKSPACE_KINDS.join(", "),
            ]
        };
        let mut projection = SavedOverview {
            scope,
            ..Default::default()
        };
        if saved.is_none() {
            projection
                .shortcuts
                .push(crate::workspaces::shortcuts::Shortcut {
                    id: "resources:all".into(),
                    label: "Resources".into(),
                    target: crate::workspaces::shortcuts::Target::Resources { namespace: None },
                });
        }
        if let Some(saved) = saved {
            match crate::workspaces::shortcuts::for_workspace(
                saved,
                self.launcher.transfer.documents.get(&saved.id),
            ) {
                Ok(shortcuts) => projection.shortcuts = shortcuts,
                Err(error) => projection.warnings.push(error),
            }
            // Restore status comes from reads that do not wait for the resource
            // load, so a failed resource read still shows unavailable scope.
            if !self.launcher.context_choices.is_empty() {
                if !self
                    .launcher
                    .context_choices
                    .iter()
                    .any(|(name, _)| name == &saved.context)
                {
                    projection
                        .warnings
                        .push(format!("Context missing: {}", saved.context));
                }
                let missing: Vec<_> = saved
                    .contexts()
                    .into_iter()
                    .filter(|context| {
                        !self
                            .launcher
                            .context_choices
                            .iter()
                            .any(|(name, _)| name == context)
                    })
                    .collect();
                if !missing.is_empty() {
                    projection
                        .warnings
                        .push(format!("Group members: {}", missing.join(", ")));
                }
                // Namespaces and kinds come from the saved primary context only,
                // as the web's restore status does.
                let inventory = self
                    .restore_inventory
                    .as_ref()
                    .filter(|inventory| inventory.context == saved.context);
                let missing: Vec<&str> =
                    match inventory.and_then(|inventory| inventory.namespaces.as_deref()) {
                        Some(available) => saved
                            .namespaces
                            .iter()
                            .filter(|namespace| !available.contains(*namespace))
                            .map(String::as_str)
                            .collect(),
                        None => Vec::new(),
                    };
                if !missing.is_empty() {
                    projection
                        .warnings
                        .push(format!("Namespaces: {}", missing.join(", ")));
                }
                let missing: Vec<&str> =
                    match inventory.and_then(|inventory| inventory.kinds.as_deref()) {
                        Some(available) => saved
                            .kinds
                            .iter()
                            .filter(|kind| {
                                !self.picker_builtin_kinds().contains(&kind.as_str())
                                    && !available.contains(*kind)
                            })
                            .map(String::as_str)
                            .collect(),
                        None => Vec::new(),
                    };
                if !missing.is_empty() {
                    projection
                        .warnings
                        .push(format!("Kinds: {}", missing.join(", ")));
                }
            }
            // The web derives these cards from the saved scope, not from the
            // Compare shortcut's own metadata, so an imported shortcut that
            // differs from its scope does not change them.
            // One pass over the workspace rows fills every side.
            let entries = compare_entries(saved);
            let mut health = vec![[HealthSummary::default(); 2]; entries.len()];
            if !entries.is_empty() {
                for row in self.workspace_rows() {
                    for (entry, sides) in entries.iter().zip(&mut health) {
                        let labels = [&entry.left_label, &entry.right_label];
                        for (label, side) in labels.into_iter().zip(sides) {
                            let matches = match entry.kind {
                                CompareKind::Contexts => row.cluster == *label,
                                CompareKind::Namespaces => row.namespace.as_ref() == Some(label),
                            };
                            if matches {
                                side.add(row);
                            }
                        }
                    }
                }
            }
            for (entry, [left, right]) in entries.into_iter().zip(health) {
                projection.comparisons.push(Comparison {
                    title: entry.label,
                    sides: [(entry.left_label, left), (entry.right_label, right)],
                });
            }
        }
        self.saved_overview = projection;
    }
    pub(super) fn saved_scope_view(&self) -> impl IntoElement {
        div()
            .mt(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div().flex().flex_wrap().gap(px(8.)).children(
                    self.saved_overview
                        .scope
                        .iter()
                        .enumerate()
                        .map(|(index, label)| {
                            ui::outline_badge(label.clone())
                                .id(SharedString::from(format!("overview-scope:{index}")))
                                .test_support()
                                .aria_label(label.clone())
                                .rounded(px(6.))
                        }),
                ),
            )
            .when(!self.saved_overview.warnings.is_empty(), |view| {
                view.child(
                    div()
                        .id("overview-restore-warning")
                        .test_support()
                        .aria_label("Unavailable saved scope")
                        .child(ui::sm("Unavailable saved scope"))
                        .children(self.saved_overview.warnings.iter().enumerate().map(
                            |(index, warning)| {
                                ui::xs(warning.clone())
                                    .id(("overview-missing", index))
                                    .test_support()
                                    .aria_label(warning.clone())
                            },
                        )),
                )
            })
    }
    pub(super) fn compare_cards(&self) -> Option<AnyElement> {
        (!self.saved_overview.comparisons.is_empty()).then(|| {
            titled_card(
                "Compare",
                "Health split for saved workspace compare shortcuts.",
                div()
                    .flex()
                    .gap(px(12.))
                    .children(self.saved_overview.comparisons.iter().map(|comparison| {
                        ui::panel()
                            .flex_1()
                            .p(px(12.))
                            .child(ui::sm(comparison.title.clone()))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .children(comparison.sides.iter().map(|(label, health)| {
                                    let summary = format!(
                                        "{} total / {} healthy / {} degraded / {} need attention",
                                        health.total,
                                        health.healthy,
                                        health.degraded,
                                        health.attention
                                    );
                                    ui::panel()
                                        .id(SharedString::from(format!(
                                            "compare:{}:{label}",
                                            comparison.title
                                        )))
                                        .test_support()
                                        .aria_label(label.clone())
                                        .aria_description(summary.clone())
                                        .flex_1()
                                        .p(px(8.))
                                        .child(ui::sm(label.clone()))
                                        .child(ui::muted(ui::xs(summary)))
                                })),
                            )
                    })),
            )
            .into_any_element()
        })
    }
}
