//! The sidebar's GitOps and Helm sections: `buildGitOpsNode` and the Helm
//! node in `workspaceShellModel.ts`, drawn like `SidebarTreeNode.svelte` with
//! the visuals of `resourceVisuals.ts`. Only the chevron expands a node, and
//! providers show once detected, as with `showUnavailableGitOpsProviders` off.

use super::{Inspector, Page};
use crate::gitops_surface::{ARGO_KINDS, FLUX_FAMILIES, Focus};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, SharedString, div, prelude::*, px,
};

/// What clicking a row opens.
#[derive(Clone, Copy)]
enum Open {
    GitOps(Focus),
    Helm,
    Nothing,
}

/// One tree row: depth, ids, label, visual, expansion and target.
struct Row {
    depth: usize,
    id: String,
    /// The node a chevron toggles, when it has children.
    toggle: Option<&'static str>,
    label: &'static str,
    icon: &'static str,
    tone: fn() -> Hsla,
    open: Open,
    disabled: bool,
}

/// `getResourceKindVisual` for the GitOps kinds.
fn kind_visual(label: &str) -> (&'static str, fn() -> Hsla) {
    match label {
        "Argo CD Applications" => ("git-branch", Tokens::primary),
        "Argo CD ApplicationSets" => ("workflow", Tokens::primary),
        "Argo CD AppProjects" => ("shield", Tokens::primary),
        "Flux Git Repositories" => ("git-branch", Tokens::muted_foreground),
        "Flux Buckets" => ("database", Tokens::storage),
        "Flux Kustomizations" => ("layers", Tokens::workload),
        "Flux Providers" => ("network", Tokens::network),
        "Flux Alerts" => ("circle-alert", Tokens::cluster),
        "Flux Receivers" => ("cable", Tokens::network),
        "Flux Image Policies" | "Flux Image Update Automations" => {
            ("workflow", Tokens::muted_foreground)
        }
        _ => ("package", Tokens::muted_foreground),
    }
}

/// `getResourceGroupVisual` for the Flux families.
fn family_visual(label: &str) -> (&'static str, fn() -> Hsla) {
    match label {
        "Workloads" => ("boxes", Tokens::workload),
        "Notifications" => ("circle-alert", Tokens::cluster),
        "Image Automation" => ("workflow", Tokens::muted_foreground),
        _ => ("package", Tokens::muted_foreground),
    }
}

impl Inspector {
    /// The header location: the selected GitOps node's label.
    pub(super) fn gitops_location(&self) -> &'static str {
        match self.gitops_page.focus {
            Focus::Section => "GitOps",
            Focus::Argo(None) => "Argo CD",
            Focus::Argo(Some(kind)) => ARGO_KINDS
                .iter()
                .find(|(item, _)| *item == kind)
                .map_or("Argo CD", |(_, label)| label),
            Focus::Flux {
                kind: Some(kind), ..
            } => FLUX_FAMILIES
                .iter()
                .flat_map(|(_, kinds)| kinds.iter())
                .find(|(item, _)| *item == kind)
                .map_or("Flux", |(_, label)| label),
            Focus::Flux {
                family: Some(family),
                ..
            } => family,
            Focus::Flux { .. } => "Flux",
        }
    }

    fn nav_open(&self, node: &str) -> bool {
        self.gitops_page.nav_open.contains(&node)
    }

    fn toggle_nav(&mut self, node: &'static str, cx: &mut Context<Self>) {
        let open = &mut self.gitops_page.nav_open;
        match open.iter().position(|item| *item == node) {
            Some(index) => {
                open.remove(index);
            }
            None => open.push(node),
        }
        cx.notify();
    }

    fn delivery_rows(&self) -> Vec<Row> {
        let row = |depth, id: String, toggle, label, (icon, tone), open| Row {
            depth,
            id,
            toggle,
            label,
            icon,
            tone,
            open,
            disabled: false,
        };
        let mut rows = vec![row(
            0,
            "nav:GitOps".into(),
            Some("GitOps"),
            "GitOps",
            ("package", Tokens::muted_foreground as fn() -> Hsla),
            Open::GitOps(Focus::Section),
        )];
        if self.nav_open("GitOps") {
            let detecting = self.loading.phase(
                "sidebar-gitops",
                self.gitops.inventory().is_none() && self.gitops.error().is_none(),
            );
            match self.gitops.inventory() {
                Some(data) if detecting.ready() => {
                    if data.argo.is_some()
                        || self
                            .persisted
                            .preferences
                            .show_unavailable_git_ops_providers
                    {
                        rows.push(Row {
                            disabled: data.argo.is_none(),
                            ..row(
                                1,
                                "nav:Argo CD".into(),
                                Some("Argo CD"),
                                "Argo CD",
                                ("git-branch", Tokens::primary),
                                Open::GitOps(Focus::Argo(None)),
                            )
                        });
                        if data.argo.is_some() && self.nav_open("Argo CD") {
                            rows.extend(ARGO_KINDS.iter().map(|(kind, label)| {
                                row(
                                    2,
                                    format!("nav:{label}"),
                                    None,
                                    *label,
                                    kind_visual(label),
                                    Open::GitOps(Focus::Argo(Some(*kind))),
                                )
                            }));
                        }
                    }
                    if data.flux.is_some()
                        || self
                            .persisted
                            .preferences
                            .show_unavailable_git_ops_providers
                    {
                        rows.push(Row {
                            disabled: data.flux.is_none(),
                            ..row(
                                1,
                                "nav:Flux".into(),
                                Some("Flux"),
                                "Flux",
                                ("package", Tokens::muted_foreground),
                                Open::GitOps(Focus::Flux {
                                    family: None,
                                    kind: None,
                                }),
                            )
                        });
                        if data.flux.is_some() && self.nav_open("Flux") {
                            for (family, kinds) in FLUX_FAMILIES {
                                rows.push(row(
                                    2,
                                    format!("nav:Flux/{family}"),
                                    Some(family),
                                    family,
                                    family_visual(family),
                                    Open::GitOps(Focus::Flux {
                                        family: Some(family),
                                        kind: None,
                                    }),
                                ));
                                if self.nav_open(family) {
                                    rows.extend(kinds.iter().map(|(kind, label)| {
                                        row(
                                            3,
                                            format!("nav:{label}"),
                                            None,
                                            *label,
                                            kind_visual(label),
                                            Open::GitOps(Focus::Flux {
                                                family: None,
                                                kind: Some(*kind),
                                            }),
                                        )
                                    }));
                                }
                            }
                        }
                    }
                }
                _ if detecting.shown() => rows.push(Row {
                    disabled: true,
                    ..row(
                        1,
                        "nav:gitops-detecting".into(),
                        None,
                        "Detecting providers...",
                        ("package", Tokens::muted_foreground),
                        Open::Nothing,
                    )
                }),
                _ => {}
            }
        }
        rows.push(row(
            0,
            "nav:Helm".into(),
            Some("Helm"),
            "Helm",
            ("package", Tokens::muted_foreground),
            Open::Helm,
        ));
        if self.nav_open("Helm") {
            rows.push(row(
                1,
                "nav:Helm/Releases".into(),
                None,
                "Releases",
                ("package", Tokens::muted_foreground),
                Open::Helm,
            ));
        }
        rows
    }

    fn nav_selected(&self, row: &Row) -> bool {
        match row.open {
            Open::GitOps(focus) => self.page == Page::GitOps && self.gitops_page.focus == focus,
            // The section row stands for the page; its one child selects the same view.
            Open::Helm => self.page == Page::Helm && row.depth == 1,
            Open::Nothing => false,
        }
    }

    /// The GitOps and Helm rows that open the Cluster section.
    pub(super) fn delivery_nav(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.delivery_rows()
            .into_iter()
            .map(|row| {
                let selected = self.nav_selected(&row);
                let expanded = row.toggle.is_some_and(|node| self.nav_open(node));
                let open = row.open;
                let toggle = row.toggle;
                div()
                    .id(SharedString::from(row.id))
                    .test_support()
                    .aria_selected(selected)
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .h(px(26.))
                    .pl(px(8. + 16. * row.depth as f32))
                    .pr(px(8.))
                    .text_color(if row.disabled {
                        Tokens::muted_foreground().opacity(0.7)
                    } else if selected {
                        Tokens::foreground()
                    } else {
                        Tokens::foreground().opacity(0.8)
                    })
                    .when(!row.disabled, |row| {
                        row.cursor_pointer().hover(|row| row.bg(Tokens::card()))
                    })
                    .when(selected, |row| {
                        row.bg(Tokens::primary().opacity(0.1)).child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(2.))
                                .rounded_r(px(2.))
                                .bg(Tokens::primary()),
                        )
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("nav-toggle:{}", row.label)))
                            .test_support()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .when_some(toggle, |chevron, node| {
                                chevron
                                    .child(ui::icon(
                                        if expanded {
                                            "chevron-down"
                                        } else {
                                            "chevron-right"
                                        },
                                        12.,
                                        Tokens::muted_foreground(),
                                    ))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.toggle_nav(node, cx);
                                    }))
                            }),
                    )
                    .child(
                        ui::icon(
                            row.icon,
                            if row.depth == 0 { 12. } else { 14. },
                            (row.tone)(),
                        )
                        .when(row.disabled, |icon| icon.opacity(0.7)),
                    )
                    .child(
                        ui::xs(row.label)
                            .line_height(px(12.))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(row.depth == 0, |text| text.font_weight(FontWeight::MEDIUM)),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| match open {
                        Open::GitOps(focus) => this.open_gitops(focus, cx),
                        Open::Helm => this.open_page(Page::Helm, cx),
                        Open::Nothing => {}
                    }))
                    .into_any_element()
            })
            .collect()
    }
}
