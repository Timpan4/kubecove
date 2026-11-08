//! Workspace sidebar, mirroring `WorkspaceSidebar.svelte`, `SidebarTree.svelte`
//! and `SidebarTreeNode.svelte`.

use super::{Inspector, Page};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, ElementId, FontWeight, Hsla, SharedString, div, prelude::*, px,
};

/// The web shell's `xl` breakpoint. The sidebar is in flow at this width or wider.
pub(super) const COMPACT_WIDTH: f32 = 1280.;
/// The sidebar's width in the wide layout.
pub(super) const WIDE_WIDTH: f32 = 260.;
/// The width of the compact navigation drawer, `SheetContent`'s `w-[280px]`.
const DRAWER_WIDTH: f32 = 280.;

struct Node {
    label: &'static str,
    icon: &'static str,
    tone: fn() -> Hsla,
    expandable: bool,
    target: Page,
}

const fn node(
    label: &'static str,
    icon: &'static str,
    tone: fn() -> Hsla,
    expandable: bool,
    target: Page,
) -> Node {
    Node {
        label,
        icon,
        tone,
        expandable,
        target,
    }
}

const WORKSPACE: &[Node] = &[
    node(
        "Workspace Overview",
        "package",
        Tokens::muted_foreground,
        false,
        Page::Overview,
    ),
    node(
        "Incidents",
        "circle-alert",
        Tokens::cluster,
        false,
        Page::Incidents,
    ),
    node(
        "Port Forwards",
        "cable",
        Tokens::network,
        false,
        Page::PortForwards,
    ),
];
/// The Cluster section after its GitOps and Helm nodes, which `gitops_nav` draws.
const CLUSTER: &[Node] = &[
    node("RBAC", "shield", Tokens::secret, true, Page::Rbac),
    node(
        "Cluster Overview",
        "server",
        Tokens::cluster,
        true,
        Page::ClusterOverview,
    ),
];
const ALL_NAMESPACES: &[Node] = &[
    node(
        "Workloads",
        "boxes",
        Tokens::workload,
        true,
        Page::Resources,
    ),
    node("Network", "network", Tokens::network, true, Page::Resources),
    node("Config", "file-cog", Tokens::config, true, Page::Resources),
    node(
        "Storage",
        "hard-drive",
        Tokens::storage,
        true,
        Page::Resources,
    ),
    node(
        "Custom Resources",
        "package",
        Tokens::muted_foreground,
        true,
        Page::Resources,
    ),
];

impl Inspector {
    fn section_header(&self, label: &str, trailing: Option<AnyElement>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .pt(px(12.))
            .pb(px(4.))
            .child(ui::caps(label, 10.5).line_height(px(16.)))
            .children(trailing)
    }

    /// `expandable` rows are Custom Resources, the only branch built here: its
    /// chevron opens and closes the kind list, as the web's "Expand" button does.
    fn tree_row(
        &self,
        id: impl Into<ElementId>,
        label: SharedString,
        (icon, tone): (&'static str, Hsla),
        expandable: bool,
        selected: bool,
        medium: bool,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let toggle = format!("nav-toggle:{label}");
        div()
            .id(id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(super::sidebar_tree::ROW_HEIGHT))
            .pl(px(8.))
            .pr(px(8.))
            .cursor_pointer()
            .text_color(if selected {
                Tokens::foreground()
            } else {
                Tokens::foreground().opacity(0.8)
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
            .hover(|row| row.bg(Tokens::card()))
            .child(
                div()
                    .id(SharedString::from(toggle))
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .when(expandable, |chevron| {
                        chevron
                            .child(ui::icon(
                                if self.live.custom_open {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                },
                                12.,
                                Tokens::muted_foreground(),
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.live.custom_open = !this.live.custom_open;
                                cx.notify();
                            }))
                    }),
            )
            .child(ui::icon(icon, if medium { 12. } else { 14. }, tone))
            .child(
                ui::xs(label)
                    .line_height(px(12.))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(medium, |text| text.font_weight(FontWeight::MEDIUM)),
            )
    }

    fn nodes(&self, nodes: &'static [Node], cx: &mut Context<Self>) -> Vec<AnyElement> {
        nodes
            .iter()
            .flat_map(|node| {
                if node.expandable && node.label != "Custom Resources" {
                    return self.builtin_branch(node.label, None, cx);
                }
                let custom = node.label == "Custom Resources";
                // Resources pages select their namespace or kind rows instead,
                // and Custom Resources selects its kinds.
                let selected = if custom {
                    self.page == Page::Resources
                        && self.sidebar_tree.selected.as_deref() == Some(node.label)
                } else {
                    node.target == self.page && node.target != Page::Resources
                };
                let target = node.target;
                let focus = self.sidebar_tree.focus_handle(node.label, cx);
                vec![
                    self.tree_row(
                        SharedString::from(format!("nav:{}", node.label)),
                        node.label.into(),
                        (node.icon, (node.tone)()),
                        node.expandable,
                        selected,
                        true,
                        cx,
                    )
                    .track_focus(&focus)
                    .tab_index(0)
                    .on_key_down(
                        cx.listener(move |this, event: &gpui_kit::KeyDownEvent, _, cx| {
                            if custom && matches!(event.keystroke.key.as_str(), "left" | "right") {
                                this.live.custom_open = event.keystroke.key == "right";
                                cx.notify();
                            }
                        }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.activate_navigation(target, custom, window, cx);
                    }))
                    .test_support()
                    .aria_description(if selected { "selected" } else { "unselected" })
                    .into_any_element(),
                ]
            })
            .collect()
    }

    fn activate_navigation(
        &mut self,
        target: Page,
        custom: bool,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        if self.can_leave(cx) {
            self.navigation_open = false;
        }
        if custom {
            self.select_custom_resources(window, cx);
            return;
        }
        match target {
            Page::Resources => self.select_namespace(None, window, cx),
            Page::ClusterOverview => self.open_cluster_overview(window, cx),
            Page::Rbac => self.open_rbac(None, window, cx),
            page => self.open_page(page, cx),
        }
    }

    /// The compact-width drawer, `WorkspaceSidebar`'s `SheetContent`.
    pub(super) fn navigation_drawer(
        &self,
        window: &gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("workspace-navigation")
            .occlude()
            .test_support()
            .aria_label("Workspace navigation")
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .right_0()
            .flex()
            .child(
                div()
                    .id("navigation-backdrop")
                    .test_support()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .bg(Tokens::background().opacity(0.6))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigation_open = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("navigation-panel")
                    .occlude()
                    .test_support()
                    .relative()
                    .h_full()
                    .bg(Tokens::background())
                    .child(self.sidebar(window, cx, DRAWER_WIDTH)),
            )
            .into_any_element()
    }

    pub(super) fn sidebar_contents(
        &self,
        window: &gpui_kit::Window,
        cx: &mut Context<Self>,
        width: f32,
    ) -> impl IntoElement {
        let _profile = crate::frame_profile::scope("sidebar", "build");
        let namespaces = if self.sidebar_tree.open.is_empty()
            && self.sidebar_tree.reveal.borrow().is_none()
            && self.sidebar_tree.focused(window, cx).is_none()
        {
            let names: Vec<_> = self
                .namespaces
                .iter()
                .filter(|ns| self.launcher.shows_namespace(&ns.name))
                .map(|ns| ns.name.clone())
                .collect();
            let tops = (0..=names.len())
                .map(|i| i as f32 * super::sidebar_tree::ROW_HEIGHT)
                .collect();
            let owner = cx.entity().downgrade();
            super::windowed::Windowed::fixed_rows(tops, move |index, _, cx| {
                owner
                    .update(cx, |owner, cx| {
                        let name = names[index].clone();
                        owner.builtin_row(
                            name.clone(),
                            name.clone().into(),
                            Some(name),
                            0,
                            true,
                            cx,
                        )
                    })
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(self.namespace_rows(cx))
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .h_full()
            .overflow_hidden()
            .rounded(px(14.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                ui::semibold(ui::sm(self.workspace_name.clone()))
                                    .id("workspace-title")
                                    .test_support()
                                    .aria_label(self.workspace_name.clone())
                                    .truncate(),
                            )
                            .child(
                                ui::muted(ui::xs(self.scope.context.clone()))
                                    .id("scope-context")
                                    .test_support()
                                    .aria_label(self.scope.context.clone())
                                    .truncate(),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(ui::outline_badge("1"))
                            .child(
                                ui::ghost_icon("chevrons-down-up", 24., 12.)
                                    .id("collapse-sidebar")
                                    .test_support()
                                    .aria_label("Collapse all sidebar sections")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        // Clicking this inside the drawer must not close it.
                                        cx.stop_propagation();
                                        this.gitops_page.nav_open.clear();
                                        this.live.custom_open = false;
                                        this.live.groups.clear();
                                        this.sidebar_tree.open.clear();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("sidebar-tree")
                    .test_support()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.sidebar_tree.scroll)
                    .py(px(8.))
                    .child(self.section_header("Workspace", None))
                    .children(self.nodes(WORKSPACE, cx))
                    .child(self.section_header("Cluster", None))
                    .children(self.delivery_nav(cx))
                    .children(self.nodes(CLUSTER, cx))
                    .child(self.section_header("All namespaces", None))
                    .children(self.nodes(ALL_NAMESPACES, cx))
                    .when(self.live.custom_open, |tree| {
                        tree.child(self.custom_search_input())
                            .children(self.custom_kind_rows(cx))
                    })
                    .child(
                        self.section_header(
                            "Namespaces",
                            Some(
                                ui::muted(ui::text("View all", 10.5, 14.))
                                    .id("view-all-namespaces")
                                    .test_support()
                                    .aria_label("View all")
                                    .when(self.page == Page::Namespaces, |link| {
                                        link.text_color(Tokens::primary())
                                    })
                                    .font_weight(FontWeight::MEDIUM)
                                    .px(px(8.))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_namespace_list(cx);
                                    }))
                                    .into_any_element(),
                            ),
                        ),
                    )
                    .child(namespaces),
            )
    }
}
