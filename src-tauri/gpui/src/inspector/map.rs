//! Ownership map panel, mirroring `OwnershipMap.svelte`,
//! `OwnershipResourceNode.svelte` and `StandaloneKindGroupNode.svelte` as
//! xyflow draws them. GPUI has no element transforms, so every flow length
//! is multiplied by the zoom when the node is built.

mod cards;

use super::Inspector;
use crate::theme::Tokens;
use crate::topology::{self, EdgeState, FlowItem, Segment, Viewport};
use crate::ui;
use cards::{group_card, resource_card, selection_ring};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, Div, MouseButton, PathBuilder, Pixels, Point, SharedString, canvas, div,
    point, prelude::*, px,
};
use kubecove_lib::models::{ResourceSummary, ResourceTopology};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

pub(super) enum TopologyLoad {
    Loading,
    Ready(ResourceTopology),
    Failed(String),
}

pub(super) struct MapState {
    pub topology: TopologyLoad,
    expanded: HashSet<String>,
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
            size: Rc::default(),
            adjusted: None,
            drag: None,
        }
    }
}

const ZOOM_STEP: f32 = 1.2;
/// The handle's 6px circle is centered on the card edge; edges meet its outer side.
const HANDLE_HALF: f32 = 3.;

struct EdgePaint {
    segments: Vec<Segment>,
    end: (f32, f32),
    state: EdgeState,
}

fn paint_edges(
    origin: Point<Pixels>,
    edges: Vec<EdgePaint>,
    viewport: Viewport,
    window: &mut gpui_kit::Window,
) {
    let at = |(x, y): (f32, f32)| {
        point(
            origin.x + px(x * viewport.zoom + viewport.x),
            origin.y + px(y * viewport.zoom + viewport.y),
        )
    };
    for edge in edges {
        let (color, width, opacity, marker) = match edge.state {
            EdgeState::Idle => (Tokens::muted_foreground(), 1.6, 0.62, 14.),
            EdgeState::Active => (Tokens::primary(), 2.6, 1., 18.),
            EdgeState::Inactive => (Tokens::muted_foreground(), 1.6, 0.2, 14.),
        };
        let color = color.opacity(opacity);
        let mut stroke = PathBuilder::stroke(px(width * viewport.zoom));
        if edge.state == EdgeState::Active {
            let dash = px(8. * viewport.zoom);
            stroke = stroke.dash_array(&[dash, dash]);
        }
        for segment in &edge.segments {
            match *segment {
                Segment::Move(x, y) => stroke.move_to(at((x, y))),
                Segment::Line(x, y) => stroke.line_to(at((x, y))),
                Segment::Quad { control, to } => stroke.curve_to(at(to), at(control)),
            }
        }
        if let Ok(path) = stroke.build() {
            window.paint_path(path, color);
        }
        // xyflow's `ArrowClosed` marker: a 20-unit viewBox scaled by the
        // marker size in stroke widths, tip on the target handle.
        let scale = marker * width / 20.;
        let (x, y) = edge.end;
        let mut arrow = PathBuilder::fill();
        arrow.add_polygon(
            &[
                at((x - 5. * scale, y - 4. * scale)),
                at((x, y)),
                at((x - 5. * scale, y + 4. * scale)),
            ],
            true,
        );
        if let Ok(path) = arrow.build() {
            window.paint_path(path, color);
        }
    }
}

impl Inspector {
    fn map_viewport(&self, view: &topology::View, cx: &mut Context<Self>) -> AnyElement {
        let (width, height) = self.map.size.get();
        let fit = topology::fit(&view.nodes, width, height);
        let viewport = match (self.map.adjusted, fit) {
            (Some((base, current)), Some(fit)) if base == fit => Some(current),
            _ => fit,
        };
        let size = self.map.size.clone();
        let entity = cx.entity_id();
        let edges: Vec<EdgePaint> = view
            .edges
            .iter()
            .map(|edge| {
                let source = &view.nodes[edge.source];
                let target = &view.nodes[edge.target];
                let end = (target.x - HANDLE_HALF, target.y + target.height / 2.);
                EdgePaint {
                    segments: topology::edge_path(
                        (
                            source.x + source.width + HANDLE_HALF,
                            source.y + source.height / 2.,
                        ),
                        end,
                    ),
                    end,
                    state: edge.state,
                }
            })
            .collect();
        let edge_layer = canvas(
            move |bounds, window, _| {
                let measured = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                if size.get() != measured {
                    size.set(measured);
                    window.on_next_frame(move |_, cx| cx.notify(entity));
                }
            },
            move |bounds, (), window, _| {
                if let Some(viewport) = viewport {
                    paint_edges(bounds.origin, edges, viewport, window);
                }
            },
        )
        .absolute()
        .size_full();
        let nodes: Vec<AnyElement> = viewport
            .map(|viewport| {
                let z = viewport.zoom;
                view.nodes
                    .iter()
                    .map(|node| {
                        let (id, content, click): (SharedString, Div, MapClick) = match &node.item {
                            FlowItem::Resource(resource) => (
                                format!("map-node:{}", resource.id).into(),
                                resource_card(node, resource, z),
                                MapClick::Resource(
                                    resource
                                        .selectable
                                        .then(|| Box::new(resource.summary.clone())),
                                ),
                            ),
                            FlowItem::Group {
                                kind,
                                members,
                                expanded,
                            } => (
                                format!("map-group:{kind}").into(),
                                group_card(kind, members.len(), *expanded, node.dimmed, z),
                                MapClick::Group(kind.clone()),
                            ),
                        };
                        div()
                            .id(id)
                            .test_support()
                            .absolute()
                            .left(px(node.x * z + viewport.x))
                            .top(px(node.y * z + viewport.y))
                            .w(px(node.width * z))
                            .h(px(node.height * z))
                            .cursor_pointer()
                            .children(selection_ring(node, z))
                            .child(content)
                            .on_click(cx.listener(move |this, _, window, cx| match &click {
                                MapClick::Resource(Some(resource)) => {
                                    this.open_resource((**resource).clone(), window, cx)
                                }
                                MapClick::Resource(None) => {}
                                MapClick::Group(kind) => {
                                    if !this.map.expanded.remove(kind) {
                                        this.map.expanded.insert(kind.clone());
                                    }
                                    cx.notify();
                                }
                            }))
                            .into_any_element()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let control = |id: &'static str, icon: &'static str, last: bool| {
            div()
                .id(id)
                .test_support()
                .flex()
                .items_center()
                .justify_center()
                .size(px(26.))
                .cursor_pointer()
                .when(!last, |button| {
                    button.border_r_1().border_color(Tokens::border())
                })
                .hover(|button| button.bg(Tokens::muted()))
                .child(ui::icon(icon, 12., Tokens::muted_foreground()))
        };
        let zoom = move |factor: f32| {
            move |this: &mut Self,
                  _: &gpui_kit::ClickEvent,
                  _: &mut gpui_kit::Window,
                  cx: &mut Context<Self>| {
                let (Some(fit), Some(current)) = (fit, viewport) else {
                    return;
                };
                let zoom = Viewport::clamp_zoom(current.zoom * factor);
                let (cx_, cy_) = (width / 2., height / 2.);
                let ratio = zoom / current.zoom;
                this.map.adjusted = Some((
                    fit,
                    Viewport {
                        x: cx_ - (cx_ - current.x) * ratio,
                        y: cy_ - (cy_ - current.y) * ratio,
                        zoom,
                    },
                ));
                cx.notify();
            }
        };
        div()
            .id("map-viewport")
            .test_support()
            .relative()
            .flex_1()
            .min_h(px(320.))
            .overflow_hidden()
            .child(edge_layer)
            .children(nodes)
            .child(
                div()
                    .absolute()
                    .top(px(15.))
                    .left(px(15.))
                    .flex()
                    .overflow_hidden()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::card().opacity(0.92))
                    .child(
                        control("map-zoom-in", "plus", false)
                            .on_click(cx.listener(zoom(ZOOM_STEP))),
                    )
                    .child(
                        control("map-zoom-out", "minus", false)
                            .on_click(cx.listener(zoom(1. / ZOOM_STEP))),
                    )
                    .child(control("map-fit", "scan", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.map.adjusted = None;
                            cx.notify();
                        },
                    ))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, _, _| {
                    this.map.drag = viewport.map(|viewport| (event.position, viewport));
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                    let (Some((start, base)), Some(fit)) = (this.map.drag, fit) else {
                        return;
                    };
                    if !event.dragging() {
                        this.map.drag = None;
                        return;
                    }
                    let delta = event.position - start;
                    this.map.adjusted = Some((
                        fit,
                        Viewport {
                            x: base.x + f32::from(delta.x),
                            y: base.y + f32::from(delta.y),
                            zoom: base.zoom,
                        },
                    ));
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.map.drag = None),
            )
            .into_any_element()
    }

    pub(super) fn map_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let topology = match &self.map.topology {
            TopologyLoad::Ready(topology) => Some(topology),
            _ => None,
        };
        let view = topology.map(|topology| {
            topology::view(
                topology,
                &self.visible,
                self.detail.resource(),
                &self.map.expanded,
            )
        });
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
        let segment = |icon: &str, label: &str, active: bool| {
            div()
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
                    .child(ui::icon("git-branch", 24., Tokens::foreground()))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::semibold(ui::xs("Ownership Map")).truncate())
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
                            .child(segment("git-branch", "Ownership", true))
                            .child(segment("network", "Network Flow", false)),
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
                            .child(ui::semibold(ui::xs("Failed to load ownership map")))
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
                        .child(ui::medium(ui::sm("No ownership graph")))
                        .child(ui::muted(ui::xs(
                            "No workload ownership relationships were found in this scope.",
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
