//! Pan, zoom, viewport culling and node focus for `OwnershipMap.svelte`.
use super::cards::{group_card, resource_card, selection_ring};
use super::paint::{EdgePaint, paint_edges};
use super::{HANDLE_HALF, Inspector, ZOOM_STEP};
use crate::{
    theme::Tokens,
    topology::{self, FlowItem, MAX_ZOOM, MIN_ZOOM, Viewport},
    ui,
};
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton, SharedString, Window, canvas, div,
    prelude::*, px,
};
use std::rc::Rc;

/// `OwnershipMap.svelte`: graphs below this many nodes and edges stay fully
/// mounted so Tab reaches offscreen nodes. Larger graphs render visible nodes only.
const VISIBLE_ELEMENT_RENDER_THRESHOLD: usize = 120;

pub(super) fn node_id(item: &FlowItem) -> SharedString {
    match item {
        FlowItem::Resource(resource) => format!("map-node:{}", resource.id).into(),
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
            .tab_stop(true)
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
        let Some(view) = self.map.cache.clone() else {
            return;
        };
        let (width, height) = self.map.size.get();
        let (Some(fit), Some(current)) = self.map_viewports(&view) else {
            return;
        };
        let zoom = Viewport::clamp_zoom(current.zoom * factor);
        let (cx_, cy_) = (width / 2., height / 2.);
        let ratio = zoom / current.zoom;
        self.map.adjusted = Some((
            fit,
            Viewport {
                x: cx_ - (cx_ - current.x) * ratio,
                y: cy_ - (cy_ - current.y) * ratio,
                zoom,
            },
        ));
        cx.notify();
    }

    /// `autoPanOnNodeFocus`: a node that focus reaches while outside the viewport
    /// is centered at the current zoom. Nodes that overlap the viewport stay put.
    pub(super) fn pan_map_node_into_view(&mut self, key: &SharedString, cx: &mut Context<Self>) {
        let Some(view) = self.map.cache.clone() else {
            return;
        };
        let Some(node) = view.nodes.iter().find(|node| node_id(&node.item) == *key) else {
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
        cx.notify();
    }

    pub(super) fn map_viewport(
        &self,
        view: &Rc<topology::View>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (width, height) = self.map.size.get();
        let (fit, viewport) = self.map_viewports(view);
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
                        let content = match &node.item {
                            FlowItem::Resource(resource) => resource_card(
                                node,
                                resource,
                                z,
                                self.map.network,
                                self.resource_metric(&resource.summary),
                            ),
                            FlowItem::Group {
                                kind,
                                members,
                                expanded,
                            } => group_card(kind, members.len(), *expanded, node.dimmed, z),
                        };
                        let id = node_id(&node.item);
                        div()
                            .id(id.clone())
                            .test_support()
                            .absolute()
                            .left(px(node.x * z + viewport.x))
                            .top(px(node.y * z + viewport.y))
                            .w(px(node.width * z))
                            .h(px(node.height * z))
                            .cursor_pointer()
                            .when_some(self.map.node_focus.get(&id), |item, focus| {
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
        if let Some(focus) = self.map.node_focus.get(&node_id(&node.item)) {
            focus.focus(window, cx);
        }
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
