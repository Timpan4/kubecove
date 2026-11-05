use super::{Inspector, Page};
use crate::{theme::Tokens, ui, workspaces::shortcuts::Target};
use gpui_kit::{Context, SharedString, TestSupportExt, div, prelude::*, px};

impl Inspector {
    pub(super) fn saved_shortcuts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shortcuts = &self.saved_overview.shortcuts;
        div()
            .id("overview-shortcuts")
            .test_support()
            .aria_description(shortcuts.len().to_string())
            .flex()
            .flex_wrap()
            .items_start()
            .min_h(px(64.))
            .p(px(12.))
            .gap(px(8.))
            .when(shortcuts.is_empty(), |view| {
                view.child(ui::sm("No shortcuts"))
            })
            .children(shortcuts.iter().map(|shortcut| {
                let target = shortcut.target.clone();
                let icon = match &target {
                    Target::Argo { .. } => "git-branch",
                    Target::Namespace { .. } => "layers",
                    Target::Compare { .. } => "git-compare-arrows",
                    Target::Resources { .. } => "boxes",
                };
                ui::outline_button()
                    .id(SharedString::from(format!("shortcut:{}", shortcut.id)))
                    .test_support()
                    .aria_label(shortcut.label.clone())
                    .h(px(24.))
                    .pl(px(6.))
                    .pr(px(8.))
                    .child(ui::icon(icon, 12., Tokens::foreground()))
                    .child(ui::medium(ui::text(shortcut.label.clone(), 12., 19.5)))
                    .on_click(cx.listener(move |this, _, window, cx| match &target {
                        Target::Argo { argo_app } => {
                            if let Some(name) = argo_app {
                                this.visit_scope(
                                    crate::workspaces::entries::EntryKind::App,
                                    this.scope.context.clone(),
                                    None,
                                    name.clone(),
                                    cx,
                                );
                            }
                            this.open_page(Page::GitOps, cx);
                        }
                        Target::Resources { namespace }
                        | Target::Namespace { namespace }
                        | Target::Compare { namespace } => {
                            this.select_namespace(namespace.clone(), window, cx);
                            this.shortcut_owner = this.launcher.active.and_then(|id| {
                                let scope =
                                    &this.launcher.transfer.documents.get(&id)?["spec"]["scope"];
                                let filter = scope["gitOpsFilter"]
                                    .as_str()
                                    .or_else(|| scope["argoAppFilter"].as_str())?;
                                (!filter.is_empty())
                                    .then(|| (id, this.scope.context.clone(), filter.to_owned()))
                            });
                            this.filter(cx);
                        }
                    }))
            }))
    }
    pub(super) fn shortcut_owner_matches(
        &self,
        row: &kubecove_lib::models::ResourceSummary,
    ) -> bool {
        let Some((workspace, context, filter)) = &self.shortcut_owner else {
            return true;
        };
        if self.launcher.active != Some(*workspace) || self.scope.context != *context {
            return true;
        }
        if let Some(owner) = &row.git_ops_owner {
            if owner.provider == "argo" && owner.kind == "Application" && owner.name == *filter {
                return true;
            }
            if !matches!(
                (owner.provider.as_str(), owner.kind.as_str()),
                ("argo", "Application") | ("flux", "Kustomization" | "HelmRelease")
            ) {
                return false;
            }
            return filter.split(':').eq([
                owner.provider.as_str(),
                owner.kind.as_str(),
                owner.namespace.as_deref().unwrap_or_default(),
                owner.name.as_str(),
            ]);
        }
        row.argo_app.as_deref().is_some_and(|app| {
            app == filter || filter.split(':').eq(["argo", "Application", "", app])
        })
    }
}
