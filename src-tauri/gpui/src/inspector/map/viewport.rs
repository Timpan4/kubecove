//! Pan, zoom, viewport culling and node focus for `OwnershipMap.svelte`.
use super::cards::{group_card, resource_card, selection_ring};
use super::paint::{EdgePaint, paint_edges};
use super::{HANDLE_HALF, Inspector, ZOOM_STEP};
use crate::{
    theme::Tokens,
    topology::{self, FlowItem, Viewport},
    ui,
};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, Div, KeyDownEvent, MouseButton, SharedString, Window, canvas, div,
    prelude::*, px,
};
use std::rc::Rc;

/// `OwnershipMap.svelte`: graphs below this many nodes and edges stay fully
/// mounted so Tab reaches offscreen nodes. Larger graphs render visible nodes only.
const VISIBLE_ELEMENT_RENDER_THRESHOLD: usize = 120;

impl Inspector {
    pub(super) fn map_viewport(
        &self,
        view: &Rc<topology::View>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (width, height) = self.map.size.get();
        let fit = topology::fit(&view.nodes, width, height);
        let viewport = match (self.map.adjusted, fit) {
            (Some((base, current)), Some(fit)) if base == fit => Some(current),
            _ => fit,
        };
        let cull = view.nodes.len() + view.edges.len() >= VISIBLE_ELEMENT_RENDER_THRESHOLD;
        let size = self.map.size.clone();
        let entity = cx.entity_id();
        let mut edge_cache = self.map.edge_cache.borrow_mut();
        if edge_cache
            .as_ref()
            .is_none_or(|(cached, _)| !Rc::ptr_eq(cached, view))
        {
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
            *edge_cache = Some((view.clone(), Rc::new(edges)));
        }
        let edges = edge_cache
            .as_ref()
            .expect("edge cache initialized")
            .1
            .clone();
        drop(edge_cache);
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
                    paint_edges(bounds.origin, &edges, viewport, window);
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
                    .enumerate()
                    .filter(|(_, node)| {
                        !cull
                            || (node.x * z + viewport.x <= width
                                && (node.x + node.width) * z + viewport.x >= 0.
                                && node.y * z + viewport.y <= height
                                && (node.y + node.height) * z + viewport.y >= 0.)
                    })
                    .map(|(index, node)| {
                        let (id, content): (SharedString, Div) = match &node.item {
                            FlowItem::Resource(resource) => (
                                format!("map-node:{}", resource.id).into(),
                                resource_card(
                                    node,
                                    resource,
                                    z,
                                    self.map.network,
                                    self.resource_metric(&resource.summary),
                                ),
                            ),
                            FlowItem::Group {
                                kind,
                                members,
                                expanded,
                            } => (
                                format!("map-group:{kind}").into(),
                                group_card(kind, members.len(), *expanded, node.dimmed, z),
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
                            .when_some(self.map.node_focus.get(index), |item, focus| {
                                item.track_focus(focus)
                                    .tab_stop(true)
                                    .focus(|style| style.border_2().border_color(Tokens::ring()))
                            })
                            .children(selection_ring(node, z))
                            .child(content)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate_map_node(index, window, cx);
                            }))
                            .on_key_down(cx.listener(
                                move |this, event: &KeyDownEvent, window, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        this.activate_map_node(index, window, cx);
                                        cx.stop_propagation();
                                    }
                                },
                            ))
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
            .children(view.edges.iter().map(|edge| {
                let source = match &view.nodes[edge.source].item {
                    FlowItem::Resource(node) => &node.id,
                    _ => return div().into_any_element(),
                };
                let target = match &view.nodes[edge.target].item {
                    FlowItem::Resource(node) => &node.id,
                    _ => return div().into_any_element(),
                };
                div()
                    .id(SharedString::from(format!("map-edge:{source}->{target}")))
                    .test_support()
                    .aria_label(format!("{source} to {target}"))
                    .absolute()
                    .into_any_element()
            }))
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

    /// Selects a resource or expands a group. Clicks and Enter or Space on a
    /// focused node share this path, so keyboard and pointer behave the same.
    pub(super) fn activate_map_node(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.map.cache.clone() else {
            return;
        };
        let Some(node) = view.nodes.get(index) else {
            return;
        };
        match &node.item {
            FlowItem::Resource(resource) if resource.selectable => {
                self.open_resource(resource.summary.clone(), window, cx);
            }
            FlowItem::Resource(_) => {}
            FlowItem::Group { kind, .. } => {
                if !self.map.expanded.remove(kind) {
                    self.map.expanded.insert(kind.clone());
                }
                self.rebuild_map();
                cx.notify();
            }
        }
    }
}
