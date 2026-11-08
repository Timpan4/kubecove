//! Pan, zoom, viewport culling and node focus for `OwnershipMap.svelte`.
use super::cards::{group_card, resource_card, selection_ring};
use super::paint::{EdgePaint, PreparedEdges};
use super::{HANDLE_HALF, Inspector, ZOOM_STEP};
use crate::targets::TestSupportExt;
use crate::{
    theme::Tokens,
    topology::{self, FlowItem, MAX_ZOOM, MIN_ZOOM, Viewport},
    ui,
};
use gpui_kit::{
    AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton, SharedString, Window, canvas, div,
    prelude::*, px,
};
use std::rc::Rc;
// Selection ring plus the shadow spread retained by the frame optimization.
const CULL_MARGIN: f32 = 2. + 3. * 10.;
/// xyflow's node click distance, `nodeClickDistance = 1` in
/// `node_modules/@xyflow/svelte/dist/lib/container/SvelteFlow/SvelteFlow.svelte`.
/// A press that moves farther than this is a pan, not a click.
const NODE_CLICK_DISTANCE: f32 = 1.;
/// Zoom per pixel of wheel delta, from `wheelDelta` in
/// `node_modules/@xyflow/system/dist/esm/index.js`: `-deltaY * 0.002`, applied as `2 ** delta`
/// by d3-zoom. A wheel reports positive `delta.y` when it turns up, so the sign is kept.
const WHEEL_PIXEL_RATE: f32 = 0.002;
/// The same `wheelDelta` branch for `deltaMode === 1`, where the delta counts lines.
const WHEEL_LINE_RATE: f32 = 0.05;
/// The same `wheelDelta` factor for Ctrl with the wheel on macOS.
const MAC_CTRL_WHEEL_FACTOR: f32 = 10.;
/// `OwnershipMap.svelte`: graphs below this many nodes and edges stay fully
/// mounted so Tab reaches offscreen nodes. Larger graphs render visible nodes only.
const VISIBLE_ELEMENT_RENDER_THRESHOLD: usize = 120;

pub(super) fn node_id(item: &FlowItem, map: &super::MapState) -> SharedString {
    match item {
        FlowItem::Resource(index) => map.node_ids[*index].clone(),
        FlowItem::Group { kind, .. } => format!("map-group:{kind}").into(),
    }
}

/// The zoom controls' actions, run by a click or by Enter or Space while focused.
#[derive(Clone, Copy)]
enum ControlAction {
    ZoomIn,
    ZoomOut,
    Fit,
}

/// A zoom control: a focusable button labeled as xyflow labels its controls.
/// A disabled control leaves Tab order, as a disabled `<button>` does on the web.
fn zoom_control(
    cx: &mut Context<Inspector>,
    action: ControlAction,
    (id, label, icon): (&'static str, &'static str, &'static str),
    last: bool,
    focus: Option<&FocusHandle>,
    disabled: bool,
) -> impl IntoElement {
    let button = div()
        .id(id)
        .test_support()
        .aria_label(label)
        .role(gpui_kit::Role::Button)
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .when(!disabled, |button| button.cursor_pointer())
        .when(disabled, |button| button.opacity(0.5))
        .when(!last, |button| {
            button.border_r_1().border_color(Tokens::border())
        })
        .hover(|button| button.bg(Tokens::muted()))
        .child(ui::icon(icon, 12., Tokens::muted_foreground()));
    let button = match focus {
        Some(focus) if !disabled => button
            .track_focus(focus)
            .focus(|style| style.border_2().border_color(Tokens::ring())),
        _ => button,
    };
    // A click focuses the control, as a `<button>` does on the web.
    let clicked_focus = focus.filter(|_| !disabled).cloned();
    button
        .on_click(
            cx.listener(move |this, _: &gpui_kit::ClickEvent, window, cx| {
                if let Some(focus) = &clicked_focus {
                    focus.focus(window, cx);
                }
                this.run_control(action, cx);
            }),
        )
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                this.run_control(action, cx);
                cx.stop_propagation();
            }
        }))
}

impl Inspector {
    /// The fitted viewport and the viewport in use, which a pan or zoom may have adjusted.
    fn map_viewports(&self, view: &topology::View) -> (Option<Viewport>, Option<Viewport>) {
        let (width, height) = self.map.size.get();
        let fit = topology::fit(&view.nodes, width, height);
        let current = match (self.map.adjusted, fit) {
            (Some((base, current)), Some(fit)) if base == fit => Some(current),
            _ => fit,
        };
        (fit, current)
    }

    fn run_control(&mut self, action: ControlAction, cx: &mut Context<Self>) {
        match action {
            ControlAction::ZoomIn => self.zoom_map(ZOOM_STEP, cx),
            ControlAction::ZoomOut => self.zoom_map(1. / ZOOM_STEP, cx),
            ControlAction::Fit => {
                self.map.adjusted = None;
                cx.notify();
            }
        }
    }

    /// Zooms about the viewport's center, as `zoomIn` and `zoomOut` do.
    fn zoom_map(&mut self, factor: f32, cx: &mut Context<Self>) {
        let (width, height) = self.map.size.get();
        self.zoom_map_about(factor, (width / 2., height / 2.), cx);
    }

    /// Zooms by `factor` about `anchor`, a point in viewport coordinates, so the graph
    /// point under it stays under it. d3-zoom's `scaleBy` does the same.
    fn zoom_map_about(&mut self, factor: f32, anchor: (f32, f32), cx: &mut Context<Self>) {
        let Some(view) = self.map.cache.clone() else {
            return;
        };
        let (Some(fit), Some(current)) = self.map_viewports(&view) else {
            return;
        };
        let zoom = Viewport::clamp_zoom(current.zoom * factor);
        let (anchor_x, anchor_y) = anchor;
        let ratio = zoom / current.zoom;
        self.map.adjusted = Some((
            fit,
            Viewport {
                x: anchor_x - (anchor_x - current.x) * ratio,
                y: anchor_y - (anchor_y - current.y) * ratio,
                zoom,
            },
        ));
        cx.notify();
    }

    /// `autoPanOnNodeFocus`: a node that focus reaches while outside the viewport
    /// is centered at the current zoom. Nodes that overlap the viewport stay put.
    /// Runs before the page renders, so the render shows the new viewport.
    pub(super) fn pan_map_node_into_view(&mut self, key: &SharedString) {
        let Some(view) = self.map.cache.clone() else {
            return;
        };
        let Some(node) = view
            .nodes
            .iter()
            .find(|node| node_id(&node.item, &self.map) == *key)
        else {
            return;
        };
        let (width, height) = self.map.size.get();
        let (Some(fit), Some(current)) = self.map_viewports(&view) else {
            return;
        };
        let zoom = current.zoom;
        let left = node.x * zoom + current.x;
        let right = (node.x + node.width) * zoom + current.x;
        let top = node.y * zoom + current.y;
        let bottom = (node.y + node.height) * zoom + current.y;
        if left <= width && right >= 0. && top <= height && bottom >= 0. {
            return;
        }
        let (center_x, center_y) = (node.x + node.width / 2., node.y + node.height / 2.);
        self.map.adjusted = Some((
            fit,
            Viewport {
                x: width / 2. - center_x * zoom,
                y: height / 2. - center_y * zoom,
                zoom,
            },
        ));
    }

    pub(super) fn map_viewport(
        &self,
        view: &Rc<topology::View>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let super::TopologyLoad::Ready(topology) = &self.map.topology else {
            return div().into_any_element();
        };
        let (width, height) = self.map.size.get();
        let (fit, viewport) = self.map_viewports(view);
        let cull = viewport
            .filter(|_| view.nodes.len() + view.edges.len() >= VISIBLE_ELEMENT_RENDER_THRESHOLD)
            .map(|viewport| {
                let flow = |screen: f32, offset: f32| (screen - offset) / viewport.zoom;
                (
                    flow(0., viewport.x) - CULL_MARGIN,
                    flow(0., viewport.y) - CULL_MARGIN,
                    flow(width, viewport.x) + CULL_MARGIN,
                    flow(height, viewport.y) + CULL_MARGIN,
                )
            });
        let shown = |(left, top, right, bottom): (f32, f32, f32, f32)| {
            cull.is_none_or(|(min_x, min_y, max_x, max_y)| {
                right >= min_x && left <= max_x && bottom >= min_y && top <= max_y
            })
        };
        let size = self.map.size.clone();
        let origin = self.map.origin.clone();
        let entity = cx.entity_id();
        let cache = if view.focused {
            &self.map.edge_cache
        } else {
            &self.map.unselected_edge_cache
        };
        let mut edge_cache = cache.borrow_mut();
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
                        bounds: (
                            source.x.min(target.x),
                            source.y.min(target.y),
                            (source.x + source.width).max(target.x + target.width),
                            (source.y + source.height).max(target.y + target.height),
                        ),
                    }
                })
                .collect();
            *edge_cache = Some((view.clone(), Rc::new(PreparedEdges::new(edges))));
        }
        let edges = edge_cache
            .as_ref()
            .expect("edge cache initialized")
            .1
            .clone();
        drop(edge_cache);
        let visible_edges: Rc<[usize]> = edges
            .source
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| shown(edge.bounds).then_some(index))
            .collect::<Vec<_>>()
            .into();
        let painted_edges = visible_edges.clone();
        let edge_layer = canvas(
            move |bounds, window, _| {
                origin.set((f32::from(bounds.origin.x), f32::from(bounds.origin.y)));
                let measured = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                if size.get() != measured {
                    size.set(measured);
                    window.on_next_frame(move |_, cx| cx.notify(entity));
                }
            },
            move |bounds, (), window, _| {
                if let Some(viewport) = viewport {
                    edges.paint(bounds.origin, &painted_edges, viewport, window);
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
                        shown((node.x, node.y, node.x + node.width, node.y + node.height))
                    })
                    .map(|(index, node)| {
                        let content = match &node.item {
                            FlowItem::Resource(index) => {
                                let resource = &topology.nodes[*index];
                                resource_card(
                                    node,
                                    resource,
                                    z,
                                    self.map.network,
                                    self.resource_metric(&resource.summary),
                                )
                            }
                            FlowItem::Group {
                                kind,
                                members,
                                expanded,
                            } => group_card(kind, members.len(), *expanded, node.dimmed, z),
                        };
                        let id = node_id(&node.item, &self.map);
                        div()
                            .id(id.clone())
                            .test_support()
                            .absolute()
                            .left(px(node.x * z + viewport.x))
                            .top(px(node.y * z + viewport.y))
                            .w(px(node.width * z))
                            .h(px(node.height * z))
                            .cursor_pointer()
                            .when_some(self.map.node_focus(&node.item), |item, focus| {
                                item.track_focus(focus)
                                    .focus(|style| style.border_2().border_color(Tokens::ring()))
                            })
                            .children(selection_ring(node, z))
                            .child(content)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                // The release of a pan lands on the node that moved under the pointer.
                                if !this.map.pan_moved {
                                    this.activate_map_node(index, window, cx);
                                }
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
        let zoom_focus = self.map.zoom_focus.as_ref();
        let focus_at = |index: usize| zoom_focus.map(|handles| &handles[index]);
        div()
            .id("map-viewport")
            .test_support()
            // The zoom in use, to two decimals, as the parity driver reads it.
            .aria_value(
                viewport.map_or_else(String::new, |viewport| format!("{:.2}", viewport.zoom)),
            )
            .relative()
            .flex_1()
            .min_h(px(320.))
            .overflow_hidden()
            .child(edge_layer)
            .children(visible_edges.iter().map(|&index| {
                let edge = &view.edges[index];
                let source = match &view.nodes[edge.source].item {
                    FlowItem::Resource(node) => &topology.nodes[*node].id,
                    _ => return div().into_any_element(),
                };
                let target = match &view.nodes[edge.target].item {
                    FlowItem::Resource(node) => &topology.nodes[*node].id,
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
                    .child(zoom_control(
                        cx,
                        ControlAction::ZoomIn,
                        ("map-zoom-in", "Zoom In", "plus"),
                        false,
                        focus_at(0),
                        viewport.is_some_and(|viewport| viewport.zoom >= MAX_ZOOM),
                    ))
                    .child(zoom_control(
                        cx,
                        ControlAction::ZoomOut,
                        ("map-zoom-out", "Zoom Out", "minus"),
                        false,
                        focus_at(1),
                        viewport.is_some_and(|viewport| viewport.zoom <= MIN_ZOOM),
                    ))
                    .child(zoom_control(
                        cx,
                        ControlAction::Fit,
                        ("map-fit", "Fit View", "scan"),
                        true,
                        focus_at(2),
                        false,
                    )),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, _, _| {
                    this.map.drag = viewport.map(|viewport| (event.position, viewport));
                    this.map.pan_moved = false;
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
                    let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                    if dx * dx + dy * dy > NODE_CLICK_DISTANCE * NODE_CLICK_DISTANCE {
                        this.map.pan_moved = true;
                    }
                    this.map.adjusted = Some((
                        fit,
                        Viewport {
                            x: base.x + dx,
                            y: base.y + dy,
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
            .on_scroll(
                cx.listener(move |this, event: &gpui_kit::ScrollEvent, _, cx| {
                    // A wheel turn up reports positive `delta.y`, the opposite sign of the web's `deltaY`.
                    let (delta, rate) = match event.delta {
                        gpui_kit::ScrollDelta::Pixels(delta) => {
                            (f32::from(delta.y), WHEEL_PIXEL_RATE)
                        }
                        gpui_kit::ScrollDelta::Lines(delta) => (delta.y, WHEEL_LINE_RATE),
                    };
                    let factor = if cfg!(target_os = "macos") && event.modifiers.control {
                        MAC_CTRL_WHEEL_FACTOR
                    } else {
                        1.
                    };
                    let (origin_x, origin_y) = this.map.origin.get();
                    let anchor = (
                        f32::from(event.position.x) - origin_x,
                        f32::from(event.position.y) - origin_y,
                    );
                    this.zoom_map_about(2f32.powf(delta * rate * factor), anchor, cx);
                    cx.stop_propagation();
                }),
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
        if let Some(focus) = self.map.node_focus(&node.item) {
            focus.focus(window, cx);
        }
        let super::TopologyLoad::Ready(topology) = &self.map.topology else {
            return;
        };
        match &node.item {
            FlowItem::Resource(index) if topology.nodes[*index].selectable => {
                self.open_resource(topology.nodes[*index].summary.clone(), window, cx);
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
