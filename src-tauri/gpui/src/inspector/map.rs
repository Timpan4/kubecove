//! Ownership map panel, mirroring `OwnershipMap.svelte`,
//! `OwnershipResourceNode.svelte` and `StandaloneKindGroupNode.svelte` as
//! xyflow draws them. GPUI has no element transforms, so every flow length
//! is multiplied by the zoom when the node is built.

mod cards;
mod paint;
mod viewport;
use paint::EdgePaint;

use super::Inspector;
use crate::theme::Tokens;
use crate::topology::{self, Viewport};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, Pixels, Point, SharedString, div, prelude::*, px};
use kubecove_lib::models::{ResourceSummary, ResourceTopology};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

pub(super) enum TopologyLoad {
    Loading,
    Ready(ResourceTopology),
    Failed(String),
}

type CachedEdges = (Rc<topology::View>, Rc<Vec<EdgePaint>>);

pub(super) struct MapState {
    pub topology: TopologyLoad,
    expanded: HashSet<String>,
    pub network: bool,
    pub cache: Option<Rc<topology::View>>,
    edge_cache: RefCell<Option<CachedEdges>>,
    /// The graph viewport's size, measured during prepaint.
    size: Rc<Cell<(f32, f32)>>,
    /// A pan or zoom, kept while the fitted viewport it started from is current.
    adjusted: Option<(Viewport, Viewport)>,
    drag: Option<(Point<Pixels>, Viewport)>,
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
                    .child(ui::ghost_icon("panel-left-close", 28., 16.))
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
                vec![centered(ui::muted(ui::xs("Loading topology"))).into_any_element()]
            }
            (TopologyLoad::Failed(error), _) => vec![
                div()
                    .p(px(12.))
                    .child(
                        div()
                            .id("map-error")
                            .role(gpui_kit::Role::Alert)
                            .p(px(12.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::destructive().opacity(0.4))
                            .child(ui::semibold(ui::xs(if self.map.network { "Failed to load network flow" } else { "Failed to load ownership map" })))
                            .child(ui::muted(ui::xs(error.clone()))),
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
                        .child(ui::text("skip graph", 11., 16.).text_color(Tokens::primary()))
                        .child(ui::muted(ui::text(".", 11., 16.)))
                        .into_any_element(),
                    self.map_viewport(view, cx),
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
                                ui::muted(ui::text(warning.clone(), 11., 16.)).truncate()
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
}

enum MapClick {
    Resource(Option<Box<ResourceSummary>>),
    Group(String),
}
