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

    fn tree_row(
        &self,
        id: impl Into<ElementId>,
        label: SharedString,
        (icon, tone): (&'static str, Hsla),
        expandable: bool,
        selected: bool,
        medium: bool,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        div()
            .id(id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(26.))
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
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .when(expandable, |chevron| {
                        chevron.child(ui::icon("chevron-right", 12., Tokens::muted_foreground()))
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
                // Resources pages select their namespace or kind rows instead.
                let selected = node.target == self.page && node.target != Page::Resources;
                let target = node.target;
                let custom = node.label == "Custom Resources";
                vec![
                    self.tree_row(
                        SharedString::from(format!("nav:{}", node.label)),
                        node.label.into(),
                        (node.icon, (node.tone)()),
                        node.expandable,
                        selected,
                        true,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| match target {
                        Page::Resources if custom => {
                            this.live.custom_open = !this.live.custom_open;
                            cx.notify();
                        }
                        Page::Resources => this.select_namespace(None, window, cx),
                        Page::ClusterOverview => this.open_cluster_overview(window, cx),
                        Page::Rbac => this.open_rbac(None, window, cx),
                        page => this.open_page(page, cx),
                    }))
                    .test_support()
                    .into_any_element(),
                ]
            })
            .collect()
    }

    /// The compact-width drawer, `WorkspaceSidebar`'s `SheetContent`. Any click in
    /// the panel that reaches it closes the drawer, after the clicked row acts.
    pub(super) fn navigation_drawer(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("workspace-navigation")
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
                    .test_support()
                    .relative()
                    .h_full()
                    .bg(Tokens::background())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigation_open = false;
                        cx.notify();
                    }))
                    .child(self.sidebar(cx, DRAWER_WIDTH)),
            )
            .into_any_element()
    }

    pub(super) fn sidebar(&self, cx: &mut Context<Self>, width: f32) -> impl IntoElement {
        let namespaces = self.namespace_rows(cx);
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
                    .children(namespaces),
            )
    }
}
