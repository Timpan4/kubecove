//! Native workspace dialogs, preview and explicit import decisions.
use super::{
    Inspector,
    controls::{Variant, button},
    request::InFlight,
};
use crate::targets::TestSupportExt;
use crate::{
    theme::Tokens,
    ui,
    workspaces::{
        SavedWorkspace,
        transfer::{self, ImportedWorkspace},
    },
};
use gpui_kit::{Context, Div, SharedString, Window, div, prelude::*, px};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct TransferState {
    /// The native file dialog in flight. Import and Export wait for it.
    dialog: Option<InFlight>,
    pub(super) documents: HashMap<u64, serde_json::Value>,
    /// The parsed import file, with the decision for each workspace.
    preview: Vec<(ImportedWorkspace, ImportAction)>,
    /// The last import or export's outcome.
    notice: Option<Notice>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportAction {
    Add,
    Replace,
    Skip,
}
enum Notice {
    Info(String),
    Failed(String),
}
impl TransferState {
    pub(super) fn busy(&self) -> bool {
        self.dialog.is_some()
    }
    /// Starts a file dialog and clears the last outcome.
    fn start(&mut self, dialog: gpui_kit::Task<()>) {
        self.notice = None;
        self.dialog = Some(dialog.into());
    }
    fn nothing_to_import(&self) -> bool {
        self.preview
            .iter()
            .all(|(_, action)| *action == ImportAction::Skip)
    }
    fn matches(&self, saved: &SavedWorkspace, item: &ImportedWorkspace) -> bool {
        saved.name == item.name
            || self.documents.get(&saved.id).is_some_and(|document| {
                document["metadata"]["name"] == item.document["metadata"]["name"]
            })
    }
}

impl Inspector {
    pub(super) fn import_workspace_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.launcher.transfer.busy() {
            return;
        }
        let read = self.backend.import_workspace();
        let dialog = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let launcher = &mut this.launcher;
                launcher.transfer.dialog = None;
                match result {
                    Ok(None) => {
                        launcher.transfer.notice = Some(Notice::Info("Import canceled.".into()))
                    }
                    Ok(Some(raw)) => match transfer::parse(&raw) {
                        Ok(preview) => {
                            launcher.transfer.preview = preview
                                .into_iter()
                                .map(|item| {
                                    let known = launcher
                                        .saved
                                        .iter()
                                        .any(|saved| launcher.transfer.matches(saved, &item));
                                    let action = if known {
                                        ImportAction::Skip
                                    } else {
                                        ImportAction::Add
                                    };
                                    (item, action)
                                })
                                .collect();
                        }
                        Err(error) => launcher.transfer.notice = Some(Notice::Failed(error)),
                    },
                    Err(error) => launcher.transfer.notice = Some(Notice::Failed(error.message)),
                }
                cx.notify();
            });
        });
        self.launcher.transfer.start(dialog);
        cx.notify();
    }
    pub(super) fn export_workspace_file(
        &mut self,
        id: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.launcher.transfer.busy() {
            return;
        }
        let saved: Vec<_> = self
            .launcher
            .saved
            .iter()
            .filter(|workspace| id.is_none_or(|id| id == workspace.id))
            .map(|workspace| {
                (
                    workspace,
                    self.launcher.transfer.documents.get(&workspace.id),
                )
            })
            .collect();
        let content = transfer::export(&saved);
        let message = format!(
            "Exported {}.",
            if saved.len() == 1 {
                saved[0].0.name.clone()
            } else {
                format!("{} workspaces", saved.len())
            }
        );
        let read = self
            .backend
            .export_workspace("kubecove-workspaces.json".into(), content);
        let dialog = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let transfer = &mut this.launcher.transfer;
                transfer.dialog = None;
                transfer.notice = Some(match result {
                    Ok(true) => Notice::Info(message),
                    Ok(false) => Notice::Info("Export canceled.".into()),
                    Err(error) => Notice::Failed(error.message),
                });
                cx.notify();
            });
        });
        self.launcher.transfer.start(dialog);
        cx.notify();
    }
    fn apply_workspace_import(&mut self, cx: &mut Context<Self>) {
        if self.launcher.transfer.nothing_to_import() {
            return;
        }
        let preview = std::mem::take(&mut self.launcher.transfer.preview);
        for (item, decision) in preview {
            if decision == ImportAction::Skip {
                continue;
            }
            let existing = (decision == ImportAction::Replace)
                .then(|| {
                    self.launcher
                        .saved
                        .iter()
                        .position(|saved| self.launcher.transfer.matches(saved, &item))
                })
                .flatten();
            let next = self.launcher.next;
            self.launcher.next = self.launcher.next.saturating_add(1);
            let id = existing.map_or(next, |index| self.launcher.saved[index].id);
            let mut name = item.name;
            if existing.is_none() {
                let base = name.clone();
                let mut suffix = 2;
                while self.launcher.saved.iter().any(|saved| saved.name == name) {
                    name = format!("{base} ({suffix})");
                    suffix += 1;
                }
            }
            let mut workspace = SavedWorkspace {
                id,
                name,
                context: item.context,
                group: item.group,
                group_name: item.group_name,
                namespaces: item.namespaces,
                kinds: item.kinds,
                updated: next,
                reviews: vec![],
            };
            // `workspace-sharing.ts`: Replace keeps the reviews whose context
            // is still in the workspace's scope.
            if let Some(index) = existing {
                let contexts = workspace.contexts();
                let kept = self.launcher.saved[index]
                    .reviews
                    .iter()
                    .filter(|review| contexts.contains(&review.cluster_context.as_str()))
                    .cloned()
                    .collect();
                workspace.reviews = kept;
            }
            self.launcher.transfer.documents.insert(id, item.document);
            if let Some(index) = existing {
                self.launcher.saved[index] = workspace;
            } else {
                self.launcher.saved.push(workspace);
            }
        }
        self.save_preferences(cx);
        cx.notify();
    }
    pub(super) fn workspace_transfer_panel(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.launcher.transfer;
        let mut panel = div().flex().flex_col().gap(px(8.));
        match &state.notice {
            Some(Notice::Failed(error)) => {
                panel = panel.child(
                    ui::xs(error.clone())
                        .id("workspace-transfer-error")
                        .test_support()
                        .aria_label(error.clone())
                        .text_color(Tokens::destructive()),
                );
            }
            Some(Notice::Info(message)) => {
                panel = panel.child(
                    ui::muted(ui::xs(message.clone()))
                        .id("workspace-transfer-message")
                        .test_support()
                        .aria_label(message.clone()),
                );
            }
            None => {}
        }
        if !state.preview.is_empty() {
            panel = panel.child(
                ui::medium(ui::sm("Import preview"))
                    .id("workspace-import-heading")
                    .test_support()
                    .aria_label("Import preview"),
            );
            for (index, (item, decision)) in state.preview.iter().enumerate() {
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::sm(item.name.clone()))
                        .children(
                            [
                                (ImportAction::Add, "Add as new"),
                                (ImportAction::Replace, "Replace"),
                                (ImportAction::Skip, "Skip"),
                            ]
                            .into_iter()
                            .map(|(action, label)| {
                                button(
                                    if *decision == action {
                                        Variant::Secondary
                                    } else {
                                        Variant::Outline
                                    },
                                    24.,
                                    false,
                                )
                                .id(SharedString::from(format!(
                                    "workspace-import:{index}:{label}"
                                )))
                                .test_support()
                                .aria_label(label)
                                .child(label)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some((_, decision)) =
                                            this.launcher.transfer.preview.get_mut(index)
                                        {
                                            *decision = action;
                                        }
                                        cx.notify();
                                    },
                                ))
                            }),
                        ),
                );
            }
            panel = panel.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(Variant::Default, 28., state.nothing_to_import())
                            .id("settings-button:Apply import")
                            .test_support()
                            .aria_label("Apply import")
                            .child("Apply import")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.apply_workspace_import(cx)),
                            ),
                    )
                    .child(
                        button(Variant::Outline, 28., false)
                            .id("workspace-import-cancel")
                            .test_support()
                            .aria_label("Cancel import")
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launcher.transfer.preview.clear();
                                cx.notify();
                            })),
                    ),
            );
        }
        panel
    }
}
