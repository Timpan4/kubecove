//! The harness page: header, graph canvas with xyflow's controls, and the
//! run result, styled as `topology-spike.css` and the harness's inline node styles.

use super::super::viewport::{
    MAC_CTRL_WHEEL_FACTOR, NODE_CLICK_DISTANCE, WHEEL_LINE_RATE, WHEEL_PIXEL_RATE,
};
use super::super::{Inspector, ZOOM_STEP};
use super::{FIT_PADDING, MAX_ZOOM, MIN_ZOOM, Spike};
use crate::inspector::controls::{Variant, button};
use crate::inspector::settings_page::settings_row;
use crate::settings::Row;
use crate::targets::TestSupportExt;
use crate::topology::{self, Segment, Viewport};
use crate::{theme::Tokens, ui};
use gpui_kit::{
    AnyElement, BoxShadow, Context, Div, FontWeight, Hsla, MouseButton, PathBuilder, Rgba,
    SharedString, canvas, div, point, prelude::*, px, rgb, rgba,
};
use kubecove_lib::models::ResourceHealth;
use std::rc::Rc;

/// An edge in flow units, painted after the viewport transform.
struct Line {
    from: (f32, f32),
    to: (f32, f32),
    active: bool,
}

fn health_color(health: ResourceHealth) -> Rgba {
    match health {
        ResourceHealth::Healthy => rgb(0x14b8a6),
        ResourceHealth::Attention => rgb(0xf59e0b),
        ResourceHealth::Restarted => rgb(0x8b5cf6),
        _ => rgb(0xef4444),
    }
}

impl Spike {
    /// Nodes and edges inside the viewport, as `onlyRenderVisibleElements` keeps them.
    fn visible(&self, viewport: Viewport) -> (Vec<usize>, Vec<Line>) {
        let (width, height) = self.size.get();
        let (node_width, node_height) = self.options.node_size();
        let flow = |screen: f32, offset: f32| (screen - offset) / viewport.zoom;
        let (min_x, min_y) = (flow(0., viewport.x), flow(0., viewport.y));
        let (max_x, max_y) = (flow(width, viewport.x), flow(height, viewport.y));
        let overlaps = |left: f32, top: f32, right: f32, bottom: f32| {
            right >= min_x && left <= max_x && bottom >= min_y && top <= max_y
        };
        let nodes = &self.graph.nodes;
        let shown = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| overlaps(node.x, node.y, node.x + node_width, node.y + node_height))
            .map(|(index, _)| index)
            .collect();
        let lines = self
            .graph
            .edges
            .iter()
            .filter(|&&(source, target)| {
                let touches = self.selected.is_some_and(|id| id == source || id == target);
                touches || !self.options.selected_edges
            })
            .filter_map(|&(source, target)| {
                let (source_node, target_node) = (&nodes[source], &nodes[target]);
                let left = source_node.x.min(target_node.x);
                let top = source_node.y.min(target_node.y);
                let right = source_node.x.max(target_node.x) + node_width;
                let bottom = source_node.y.max(target_node.y) + node_height;
                overlaps(left, top, right, bottom).then(|| Line {
                    from: (source_node.x + node_width, source_node.y + node_height / 2.),
                    to: (target_node.x, target_node.y + node_height / 2.),
                    active: self.selected.is_some_and(|id| id == source || id == target),
                })
            })
            .collect();
        (shown, lines)
    }

    fn node(&self, index: usize, viewport: Viewport) -> Div {
        let node = &self.graph.nodes[index];
        let z = viewport.zoom;
        let (width, height) = self.options.node_size();
        let selected = self.selected == Some(index);
        let base = div()
            .absolute()
            .left(px(node.x * z + viewport.x))
            .top(px(node.y * z + viewport.y))
            .w(px(width * z))
            .h(px(height * z))
            .overflow_hidden()
            .rounded(px(6. * z))
            .border(px(if selected { 2. } else { 1. } * z))
            .border_color(if selected {
                rgb(0xbae6fd)
            } else {
                rgba(0x94a3b88c)
            })
            .when(selected, |node| {
                node.shadow(vec![BoxShadow {
                    color: rgba(0x38bdf838).into(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(3. * z),
                    inset: false,
                }])
            })
            .text_color(rgb(0xe5e7eb));
        if self.options.compact {
            return base
                .flex()
                .bg(rgba(0x0f172af5))
                .child(div().w(px(5. * z)).h_full().bg(health_color(node.health)))
                .child(
                    ui::text(node.abbreviation.clone(), 10. * z, 20. * z)
                        .flex_1()
                        .text_center()
                        .font_weight(FontWeight::BOLD),
                );
        }
        base.bg(if node.health == ResourceHealth::Healthy {
            rgba(0x0f172af0)
        } else {
            rgba(0x1e293bf5)
        })
        .px(px(10. * z))
        .py(px(8. * z))
        .children(
            node.lines
                .iter()
                .map(|line| ui::text(line.clone(), 11. * z, 13.75 * z).truncate()),
        )
    }
}

/// One of xyflow's `Controls` buttons.
fn control(
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    disabled: bool,
    pressed: Option<bool>,
) -> crate::targets::ObservedElement<gpui_kit::Stateful<Div>> {
    div()
        .id(id)
        .test_support()
        .aria_label(label)
        .when_some(pressed, |button, pressed| button.aria_selected(pressed))
        .role(gpui_kit::Role::Button)
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .bg(rgb(0xfefefe))
        .border_b_1()
        .border_color(rgb(0xeeeeee))
        .when(disabled, |button| button.opacity(0.4))
        .when(!disabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(rgb(0xf4f4f4)))
        })
        .child(ui::icon(icon, 12., Hsla::from(rgb(0x1a192b))))
}

impl Inspector {
    /// The Diagnostics row that opens the harness.
    pub(in crate::inspector) fn topology_spike_control(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = "Open 4k LOD";
        let open = button(Variant::Outline, 24., false)
            .id(SharedString::from(format!("settings-button:{label}")))
            .test_support()
            .aria_label(label)
            .gap(px(6.))
            .child(ui::icon("external-link", 14., Tokens::foreground()))
            .child(label)
            .on_click(cx.listener(|this, _, window, cx| this.open_topology_spike(window, cx)));
        settings_row(row.title, row.description.into(), open.into_any_element()).into_any_element()
    }

    /// The harness page while it is open. It replaces the workspace.
    pub(in crate::inspector) fn topology_spike_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let spike = self.spike.as_mut()?;
        if spike.viewport.is_none() {
            spike.viewport = spike.fit(FIT_PADDING);
        }
        let spike = self.spike.as_ref()?;
        let viewport = spike.viewport;
        let (nodes, lines) =
            viewport.map_or_else(Default::default, |viewport| spike.visible(viewport));
        spike.rendered.set((nodes.len(), lines.len()));
        let counts = spike.counts();
        let header = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(rgba(0x94a3b83d))
            .bg(rgb(0x0f172a))
            .child(ui::semibold(ui::text("Native topology spike", 12., 16.)))
            .child(
                ui::text(counts.clone(), 12., 16.)
                    .text_color(rgb(0x94a3b8))
                    .id("topology-spike-counts")
                    .test_support()
                    .aria_label(counts),
            )
            .child(div().flex_1())
            .child(
                ui::text("Run benchmark", 12., 16.)
                    .id("topology-spike-run")
                    .test_support()
                    .aria_label("Run benchmark")
                    .role(gpui_kit::Role::Button)
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgba(0x94a3b866))
                    .bg(rgb(0x1e293b))
                    .cursor_pointer()
                    .on_click(
                        cx.listener(|this, _, window, cx| this.run_topology_spike(window, cx)),
                    ),
            )
            .child(
                ui::text("Close", 12., 16.)
                    .id("topology-spike-close")
                    .test_support()
                    .aria_label("Close")
                    .role(gpui_kit::Role::Button)
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgba(0x94a3b866))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.close_topology_spike(cx))),
            );
        let size = spike.size.clone();
        let origin = spike.origin.clone();
        let entity = cx.entity_id();
        let straight = spike.options.straight;
        let lines = Rc::new(lines);
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
                let Some(viewport) = viewport else {
                    return;
                };
                let at = |(x, y): (f32, f32)| {
                    point(
                        bounds.origin.x + px(x * viewport.zoom + viewport.x),
                        bounds.origin.y + px(y * viewport.zoom + viewport.y),
                    )
                };
                for line in lines.iter() {
                    let (color, width) = if line.active {
                        (rgb(0x38bdf8), 2.4)
                    } else {
                        (rgba(0x94a3b87a), 1.4)
                    };
                    let mut stroke = PathBuilder::stroke(px(width * viewport.zoom));
                    if line.active {
                        // xyflow animates the dashes of an `animated` edge; these stay still.
                        let dash = px(5. * viewport.zoom);
                        stroke = stroke.dash_array(&[dash, dash]);
                    }
                    if straight {
                        stroke.move_to(at(line.from));
                        stroke.line_to(at(line.to));
                    } else {
                        for segment in topology::edge_path(line.from, line.to) {
                            match segment {
                                Segment::Move(x, y) => stroke.move_to(at((x, y))),
                                Segment::Line(x, y) => stroke.line_to(at((x, y))),
                                Segment::Quad { control, to } => {
                                    stroke.curve_to(at(to), at(control))
                                }
                            }
                        }
                    }
                    if let Ok(path) = stroke.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .absolute()
        .size_full();
        let mut node_elements = Vec::with_capacity(nodes.len());
        if let Some(viewport) = viewport {
            for &index in &nodes {
                node_elements.push(
                    spike
                        .node(index, viewport)
                        .id(("topology-spike-node", index))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.select_topology_spike_node(Some(index), cx);
                        })),
                );
            }
        }
        let interactive = spike.interactive;
        let controls = div()
            .absolute()
            .left(px(15.))
            .bottom(px(15.))
            .flex()
            .flex_col()
            .shadow_sm()
            .child(
                control(
                    "topology-spike-zoom-in",
                    "Zoom In",
                    "plus",
                    viewport.is_some_and(|v| v.zoom >= MAX_ZOOM),
                    None,
                )
                .on_click(cx.listener(|this, _, _, cx| this.zoom_topology_spike(ZOOM_STEP, cx))),
            )
            .child(
                control(
                    "topology-spike-zoom-out",
                    "Zoom Out",
                    "minus",
                    viewport.is_some_and(|v| v.zoom <= MIN_ZOOM),
                    None,
                )
                .on_click(
                    cx.listener(|this, _, _, cx| this.zoom_topology_spike(1. / ZOOM_STEP, cx)),
                ),
            )
            .child(
                control("topology-spike-fit", "Fit View", "scan", false, None).on_click(
                    cx.listener(|this, _, _, cx| {
                        if let Some(spike) = this.spike.as_mut() {
                            spike.viewport = spike.fit(FIT_PADDING).or(spike.viewport);
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                control(
                    "topology-spike-interactive",
                    "Toggle Interactivity",
                    "lock-keyhole",
                    false,
                    Some(!interactive),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(spike) = this.spike.as_mut() {
                        spike.interactive = !spike.interactive;
                    }
                    cx.notify();
                })),
            );
        let graph = div()
            .id("topology-spike-canvas")
            .test_support()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(edge_layer)
            .children(node_elements)
            .child(controls)
            .on_click(cx.listener(|this, _, _, cx| this.select_topology_spike_node(None, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui_kit::MouseDownEvent, _, _| {
                    if let Some(spike) = this.spike.as_mut() {
                        spike.drag = spike.viewport.map(|viewport| (event.position, viewport));
                        spike.pan_moved = false;
                    }
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                    let Some(spike) = this.spike.as_mut() else {
                        return;
                    };
                    let Some((start, base)) = spike.drag else {
                        return;
                    };
                    if !event.dragging() {
                        spike.drag = None;
                        return;
                    }
                    let delta = event.position - start;
                    let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                    if dx * dx + dy * dy > NODE_CLICK_DISTANCE * NODE_CLICK_DISTANCE {
                        spike.pan_moved = true;
                    }
                    spike.viewport = Some(Viewport {
                        x: base.x + dx,
                        y: base.y + dy,
                        zoom: base.zoom,
                    });
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(spike) = this.spike.as_mut() {
                        spike.drag = None;
                    }
                }),
            )
            .on_scroll(cx.listener(|this, event: &gpui_kit::ScrollEvent, _, cx| {
                let (delta, rate) = match event.delta {
                    gpui_kit::ScrollDelta::Pixels(delta) => (f32::from(delta.y), WHEEL_PIXEL_RATE),
                    gpui_kit::ScrollDelta::Lines(delta) => (delta.y, WHEEL_LINE_RATE),
                };
                let factor = if cfg!(target_os = "macos") && event.modifiers.control {
                    MAC_CTRL_WHEEL_FACTOR
                } else {
                    1.
                };
                if let Some(spike) = this.spike.as_mut() {
                    let (origin_x, origin_y) = spike.origin.get();
                    let anchor = (
                        f32::from(event.position.x) - origin_x,
                        f32::from(event.position.y) - origin_y,
                    );
                    spike.zoom_about(2f32.powf(delta * rate * factor), anchor);
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let result = spike.result.clone().map(|result| {
            div()
                .id("topology-spike-result")
                .test_support()
                .aria_label(result.clone())
                .max_h(px(180.))
                .overflow_y_scroll()
                .border_t_1()
                .border_color(rgba(0x94a3b83d))
                .text_color(rgb(0xbfdbfe))
                .font_family(crate::theme::MONO)
                .child(ui::text(result, 11., 14.))
        });
        Some(
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(rgb(0x020617))
                .text_color(rgb(0xe5e7eb))
                .font_family(crate::theme::SANS)
                .child(header)
                .child(graph)
                .children(result),
        )
    }

    fn zoom_topology_spike(&mut self, factor: f32, cx: &mut Context<Self>) {
        if let Some(spike) = self.spike.as_mut() {
            let (width, height) = spike.size.get();
            spike.zoom_about(factor, (width / 2., height / 2.));
        }
        cx.notify();
    }

    /// A click on a node selects it; a click on the pane clears the selection.
    /// Neither applies while interactivity is off or after a pan.
    fn select_topology_spike_node(&mut self, node: Option<usize>, cx: &mut Context<Self>) {
        let Some(spike) = self.spike.as_mut() else {
            return;
        };
        if spike.interactive && !spike.pan_moved {
            spike.selected = node;
            cx.notify();
        }
    }
}
