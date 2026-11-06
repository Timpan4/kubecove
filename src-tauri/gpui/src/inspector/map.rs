//! Ownership map panel, mirroring `OwnershipMap.svelte`,
//! `OwnershipResourceNode.svelte` and `StandaloneKindGroupNode.svelte` as
//! xyflow draws them. GPUI has no element transforms, so every flow length
//! is multiplied by the zoom when the node is built.

mod cards;
mod paint;
mod viewport;
use paint::EdgePaint;

use super::Inspector;
use super::resources::Panel;
use crate::theme::Tokens;
use crate::topology::{self, Viewport};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, Div, FocusHandle, Pixels, Point, SharedString, Window, div, prelude::*, px,
};
use kubecove_lib::models::{AppError, AppErrorKind, ResourceTopology};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(super) enum TopologyLoad {
    Loading,
    Ready(ResourceTopology),
    Failed(AppError),
}

type CachedEdges = (Rc<topology::View>, Rc<Vec<EdgePaint>>);

pub(super) struct MapState {
    pub topology: TopologyLoad,
    pub(super) expanded: HashSet<String>,
    pub network: bool,
    pub cache: Option<Rc<topology::View>>,
    edge_cache: RefCell<Option<CachedEdges>>,
    /// The graph viewport's size, measured during prepaint.
    size: Rc<Cell<(f32, f32)>>,
    /// A pan or zoom, kept while the fitted viewport it started from is current.
    adjusted: Option<(Viewport, Viewport)>,
    drag: Option<(Point<Pixels>, Viewport)>,
    /// Focus follows node identity when selection reorders the graph.
    pub node_focus: HashMap<SharedString, FocusHandle>,
    /// The node that held focus at the last render. Focus moving onto a
    /// different node pans that node into view.
    focused_node: Option<SharedString>,
    /// The zoom in, zoom out and fit controls, in Tab order.
    zoom_focus: Option<[FocusHandle; 3]>,
    /// Where "skip graph" moves focus: the element right after the graph.
    pub skip_target: Option<FocusHandle>,
}

impl MapState {
    pub fn new() -> Self {
        Self {
            topology: TopologyLoad::Loading,
            expanded: HashSet::new(),
            network: false,
            cache: None,
            edge_cache: RefCell::new(None),
            size: Rc::default(),
            adjusted: None,
            drag: None,
            node_focus: HashMap::new(),
            focused_node: None,
            zoom_focus: None,
            skip_target: None,
        }
    }
}

const ZOOM_STEP: f32 = 1.2;
/// The handle's 6px circle is centered on the card edge; edges meet its outer side.
const HANDLE_HALF: f32 = 3.;

impl Inspector {
    pub(super) fn rebuild_map(&mut self) {
        let view = match &self.map.topology {
            TopologyLoad::Ready(topology) => Some(Rc::new(topology::view(
                topology,
                &self.visible,
                if self.persisted.preferences.show_full_topology_on_selection {
                    None
                } else {
                    self.detail.resource()
                },
                &self.map.expanded,
                self.map.network,
            ))),
            _ => None,
        };
        self.map.cache = view;
    }

    pub(super) fn map_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let topology = match &self.map.topology {
            TopologyLoad::Ready(topology) => Some(topology),
            _ => None,
        };
        let view = self.map.cache.clone();
        let edges = view.as_ref().map_or(0, |view| view.edges.len());
        let subtitle = match &view {
            Some(view) if view.focused => format!(
                "{} focused / {} nodes / {edges} edges",
                view.nodes.len(),
                view.topology_nodes
            ),
            _ => format!(
                "{} nodes / {edges} edges",
                view.as_ref().map_or(0, |view| view.topology_nodes)
            ),
        };
        let warnings = topology.map_or(&[][..], |topology| &topology.warnings[..]);
        let segment = |icon: &str, label: &'static str, active: bool| {
            div()
                .id(SharedString::from(format!("map-mode:{label}")))
                .test_support()
                .aria_label(label)
                .aria_selected(active)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_map_mode(label == "Network Flow", window, cx)
                }))
                .flex()
                .items_center()
                .gap(px(4.))
                .h(px(28.))
                .px(px(8.))
                .rounded(px(6.))
                .when(active, |segment| segment.bg(Tokens::secondary()))
                .child(ui::icon(icon, 14., Tokens::foreground()))
                .child(ui::medium(ui::xs(label.to_owned())))
        };
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        ui::ghost_icon("panel-left-close", 28., 16.)
                            .id("panel-control:Collapse ownership map")
                            .test_support()
                            .aria_label("Collapse ownership map")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_panel_open(Panel::Map, false);
                                cx.notify();
                            })),
                    )
                    .child(ui::icon(
                        if self.map.network {
                            "network"
                        } else {
                            "git-branch"
                        },
                        24.,
                        Tokens::foreground(),
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                ui::semibold(ui::xs(if self.map.network {
                                    "Network Flow"
                                } else {
                                    "Ownership Map"
                                }))
                                .truncate(),
                            )
                            .child(
                                ui::muted(ui::text(subtitle.clone(), 11., 16.))
                                    .id("map-counts")
                                    .test_support()
                                    .aria_label(subtitle),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .h(px(32.))
                            .p(px(2.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::background())
                            .child(segment("git-branch", "Ownership", !self.map.network))
                            .child(segment("network", "Network Flow", self.map.network)),
                    )
                    .when(!warnings.is_empty(), |row| {
                        row.child(ui::outline_badge(warnings.len().to_string()))
                    }),
            );
        let centered = |content: Div| {
            div()
                .flex()
                .flex_1()
                .min_h(px(320.))
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(content)
        };
        let body: Vec<AnyElement> = match (&self.map.topology, &view) {
            (TopologyLoad::Loading, _) => {
                vec![centered(ui::muted(ui::xs("Loading topology"))).id("map-loading").test_support().aria_label("Loading topology").into_any_element()]
            }
            (TopologyLoad::Failed(error), _) => vec![
                div()
                    .p(px(12.))
                    .child(
                        div()
                            .id("map-error")
                            .test_support()
                            .aria_description(error.message.clone())
                            .role(gpui_kit::Role::Alert)
                            .aria_label(if error.kind == AppErrorKind::Forbidden {
                                "KubeCove does not have permission for this"
                            } else if self.map.network {
                                "Failed to load network flow"
                            } else {
                                "Failed to load ownership map"
                            })
                            .p(px(12.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::destructive().opacity(0.4))
                            .child(ui::semibold(ui::xs(if error.kind == AppErrorKind::Forbidden { "KubeCove does not have permission for this" } else if self.map.network { "Failed to load network flow" } else { "Failed to load ownership map" })))
                            .child(ui::muted(ui::xs(error.message.clone()))),
                    )
                    .into_any_element(),
            ],
            (TopologyLoad::Ready(_), Some(view)) if !view.nodes.is_empty() => {
                let mut body = vec![
                    div()
                        .flex()
                        .flex_wrap()
                        .px(px(12.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(Tokens::border())
                        .child(ui::muted(ui::text(
                            "Drag to pan. Use controls to zoom. Press Tab to reach graph nodes, or\u{a0}",
                            11.,
                            16.,
                        )))
                        // Button `variant="link"`: `text-primary`.
                        .child(
                            ui::text("skip graph", 11., 16.)
                                .id("map-skip-graph")
                                .test_support()
                                .aria_label("skip graph")
                                .text_color(Tokens::primary())
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(target) = &this.map.skip_target {
                                        target.focus(window, cx);
                                    }
                                })),
                        )
                        .child(ui::muted(ui::text(".", 11., 16.)))
                        .into_any_element(),
                    self.map_viewport(view, cx),
                    // Focus target after the graph. Tab skips it, so only "skip graph" lands here.
                    match &self.map.skip_target {
                        Some(target) => div()
                            .id("map-graph-end")
                            .test_support()
                            .track_focus(target)
                            .into_any_element(),
                        None => div().into_any_element(),
                    },
                ];
                if !warnings.is_empty() {
                    body.push(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .px(px(12.))
                            .py(px(8.))
                            .border_t_1()
                            .border_color(Tokens::border())
                            .child(ui::semibold(ui::text("Topology warnings", 11., 16.)))
                            .children(warnings.iter().take(3).map(|warning| {
                                ui::muted(ui::text(warning.clone(), 11., 16.)).truncate().id(SharedString::from(format!("map-warning:{warning}"))).test_support().aria_label(warning.clone())
                            }))
                            .when(warnings.len() > 3, |list| {
                                list.child(ui::muted(ui::text(
                                    format!("{} more warnings", warnings.len() - 3),
                                    11.,
                                    16.,
                                )))
                            })
                            .into_any_element(),
                    );
                }
                body
            }
            _ => vec![
                centered(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(4.))
                        .child(ui::medium(ui::sm(if self.map.network { "No network flow" } else { "No ownership graph" })).id("map-empty").test_support().aria_label(if self.map.network { "No network flow" } else { "No ownership graph" }))
                        .child(ui::muted(ui::xs(
                            if self.map.network { "No ingress, service, or pod traffic relationships were found in this scope." } else { "No workload ownership relationships were found in this scope." },
                        ))),
                )
                .into_any_element(),
            ],
        };
        ui::card()
            .size_full()
            .min_h(px(400.))
            .shadow_sm()
            .child(header)
            .children(body)
    }

    /// Keeps one focus handle per graph node and one for "skip graph". Called before
    /// the page renders, so handles follow the cached view without rebuilding it.
    ///
    /// Node and zoom handles are tab stops on the handle itself: an element's
    /// `tab_stop(true)` applies only to a handle the element creates, not to one it tracks.
    ///
    /// Focus moving onto a node pans it into view, as xyflow's `autoPanOnNodeFocus`
    /// does. This reads focus here rather than through `on_focus_in`, because GPUI
    /// sends focus events only while the window is active, and focus changes
    /// re-render the window, so this runs in the frame that shows the new focus.
    pub(super) fn sync_map_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut keys = HashSet::new();
        if let Some(view) = &self.map.cache {
            for node in &view.nodes {
                let key = viewport::node_id(&node.item);
                keys.insert(key.clone());
                if !self.map.node_focus.contains_key(&key) {
                    self.map
                        .node_focus
                        .insert(key, cx.focus_handle().tab_stop(true));
                }
            }
        }
        self.map.node_focus.retain(|key, _| keys.contains(key));
        let focused = self
            .map
            .node_focus
            .iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(key, _)| key.clone());
        if let Some(key) = &focused
            && self.map.focused_node.as_ref() != Some(key)
        {
            self.pan_map_node_into_view(key);
        }
        self.map.focused_node = focused;
        if self.map.zoom_focus.is_none() {
            self.map.zoom_focus = Some(std::array::from_fn(|_| cx.focus_handle().tab_stop(true)));
        }
        if self.map.skip_target.is_none() {
            self.map.skip_target = Some(cx.focus_handle());
        }
    }
}
