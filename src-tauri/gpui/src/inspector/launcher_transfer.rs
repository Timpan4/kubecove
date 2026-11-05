//! Native workspace dialogs, preview and explicit import decisions.
use super::{
    Inspector,
    controls::{Variant, button},
};
use crate::{
    theme::Tokens,
    ui,
    workspaces::{
        SavedWorkspace,
        transfer::{self, ImportedWorkspace},
    },
};
use gpui_kit::{Context, Div, SharedString, Task, TestSupportExt, Window, div, prelude::*, px};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct TransferState {
    pub(super) busy: bool,
    task: Option<Task<()>>,
    pub(super) documents: HashMap<u64, serde_json::Value>,
    preview: Vec<ImportedWorkspace>,
    decisions: Vec<ImportAction>,
    error: Option<String>,
    message: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportAction {
    Add,
    Replace,
    Skip,
}
impl TransferState {
    fn matches(&self, saved: &SavedWorkspace, item: &ImportedWorkspace) -> bool {
        saved.name == item.name
            || self.documents.get(&saved.id).is_some_and(|document| {
                document["metadata"]["name"] == item.document["metadata"]["name"]
            })
    }
}

impl Inspector {
    pub(super) fn import_workspace_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.launcher.transfer.busy {
            return;
        }
        let read = self.backend.import_workspace();
        self.launcher.transfer.busy = true;
        self.launcher.transfer.error = None;
        self.launcher.transfer.message = None;
        self.launcher.transfer.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                this.launcher.transfer.busy = false;
                match result {
                    Ok(None) => this.launcher.transfer.message = Some("Import canceled.".into()),
                    Ok(Some(raw)) => match transfer::parse(&raw) {
                        Ok(preview) => {
                            this.launcher.transfer.decisions =
                                preview
                                    .iter()
                                    .map(|item| {
                                        if this.launcher.saved.iter().any(|saved| {
                                            this.launcher.transfer.matches(saved, item)
                                        }) {
                                            ImportAction::Skip
                                        } else {
                                            ImportAction::Add
                                        }
                                    })
                                    .collect();
                            this.launcher.transfer.preview = preview;
                        }
                        Err(error) => this.launcher.transfer.error = Some(error),
                    },
                    Err(error) => this.launcher.transfer.error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn export_workspace_file(
        &mut self,
        id: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.launcher.transfer.busy {
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
        self.launcher.transfer.busy = true;
        self.launcher.transfer.error = None;
        self.launcher.transfer.message = None;
        self.launcher.transfer.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                this.launcher.transfer.busy = false;
                match result {
                    Ok(true) => this.launcher.transfer.message = Some(message),
                    Ok(false) => this.launcher.transfer.message = Some("Export canceled.".into()),
                    Err(error) => this.launcher.transfer.error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn apply_workspace_import(&mut self, cx: &mut Context<Self>) {
        let preview = std::mem::take(&mut self.launcher.transfer.preview);
        let decisions = std::mem::take(&mut self.launcher.transfer.decisions);
        for (item, decision) in preview.into_iter().zip(decisions) {
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
            let workspace = SavedWorkspace {
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
        if let Some(error) = &state.error {
            panel = panel.child(
                ui::xs(error.clone())
                    .id("workspace-transfer-error")
                    .test_support()
                    .aria_label(error.clone())
                    .text_color(Tokens::destructive()),
            );
        }
        if let Some(message) = &state.message {
            panel = panel.child(
                ui::muted(ui::xs(message.clone()))
                    .id("workspace-transfer-message")
                    .test_support()
                    .aria_label(message.clone()),
            );
        }
        if !state.preview.is_empty() {
            panel = panel.child(
                ui::medium(ui::sm("Import preview"))
                    .id("workspace-import-heading")
                    .test_support()
                    .aria_label("Import preview"),
            );
            for (index, item) in state.preview.iter().enumerate() {
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
                                    if state.decisions[index] == action {
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
                                        this.launcher.transfer.decisions[index] = action;
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
                        button(
                            Variant::Default,
                            28.,
                            state
                                .decisions
                                .iter()
                                .all(|action| *action == ImportAction::Skip),
                        )
                        .id("settings-button:Apply import")
                        .test_support()
                        .aria_label("Apply import")
                        .child("Apply import")
                        .on_click(cx.listener(|this, _, _, cx| this.apply_workspace_import(cx))),
                    )
                    .child(
                        button(Variant::Outline, 28., false)
                            .id("workspace-import-cancel")
                            .test_support()
                            .aria_label("Cancel import")
                            .child("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launcher.transfer.preview.clear();
                                this.launcher.transfer.decisions.clear();
                                cx.notify();
                            })),
                    ),
            );
        }
        panel
    }
}
