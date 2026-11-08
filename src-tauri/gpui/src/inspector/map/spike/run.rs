//! `BenchmarkController.svelte`: one benchmark run, advanced one step per
//! display frame as the web's `requestAnimationFrame` awaits are.

use super::{Inspector, MAX_ZOOM, MIN_ZOOM};
use crate::topology::Viewport;
use gpui_kit::{Context, SharedString, Window};
use serde::Serialize;
use std::collections::VecDeque;
use std::time::Instant;

/// `fitView({ padding: 0.2 })` at the end of a run.
const RUN_FIT_PADDING: f32 = 0.2;
/// The run's `setViewport` calls.
const VIEWPORT_OPERATIONS: usize = 24;

enum Op {
    /// Wait for the next display frame.
    Frame,
    Select(usize),
    Viewports,
    Fit,
    /// Record a phase boundary.
    Mark,
}

pub(super) struct Run {
    ops: VecDeque<Op>,
    marks: Vec<Instant>,
}

/// `TopologySpikeRunResult`. The native client has no JavaScript heap, so
/// both heap fields are null.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunResult {
    framework: &'static str,
    node_count: usize,
    edge_count: usize,
    initial_render_ms: f64,
    selection_churn_ms: f64,
    viewport_ops_ms: f64,
    fit_view_ms: f64,
    total_interaction_ms: f64,
    used_js_heap_before_bytes: Option<u64>,
    used_js_heap_after_bytes: Option<u64>,
    rendered_node_count: usize,
    rendered_edge_count: usize,
}

impl Run {
    fn new(selection: &[usize]) -> Self {
        // `settle`: Svelte's `tick` and two animation frames.
        let settle = [Op::Frame, Op::Frame];
        let mut ops = VecDeque::from(settle);
        ops.push_back(Op::Mark);
        for &node in selection {
            ops.extend([Op::Select(node), Op::Frame]);
        }
        ops.extend([Op::Frame, Op::Frame, Op::Mark, Op::Viewports]);
        ops.extend([Op::Frame, Op::Frame, Op::Mark, Op::Fit]);
        ops.extend([Op::Frame, Op::Frame, Op::Mark]);
        Self {
            ops,
            marks: Vec::new(),
        }
    }
}

fn milliseconds(from: Instant, to: Instant) -> f64 {
    to.duration_since(from).as_secs_f64() * 1000.
}

impl Inspector {
    /// `firstRender`: the time from opening to the first settled frame. With
    /// `autorun`, the run starts then.
    pub(super) fn measure_spike_first_render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = entity.update(cx, |this, cx| {
                    let Some(spike) = this.spike.as_mut() else {
                        return;
                    };
                    spike.initial_render_ms = Some(milliseconds(spike.opened, Instant::now()));
                    if spike.options.autorun {
                        this.run_topology_spike(window, cx);
                    }
                });
            });
        });
    }

    pub(super) fn run_topology_spike(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spike) = self.spike.as_mut() else {
            return;
        };
        if spike.run.is_some() {
            return;
        }
        spike.result = None;
        spike.run = Some(Run::new(&spike.graph.selection));
        self.advance_topology_spike(window, cx);
    }

    fn advance_topology_spike(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spike) = self.spike.as_mut() else {
            return;
        };
        let Some(mut run) = spike.run.take() else {
            return;
        };
        while let Some(op) = run.ops.pop_front() {
            match op {
                Op::Frame => {
                    spike.run = Some(run);
                    let entity = cx.entity().downgrade();
                    window.on_next_frame(move |window, cx| {
                        let _ =
                            entity.update(cx, |this, cx| this.advance_topology_spike(window, cx));
                    });
                    cx.notify();
                    return;
                }
                Op::Select(node) => spike.selected = Some(node),
                Op::Viewports => {
                    for index in 0..VIEWPORT_OPERATIONS {
                        let step = index as f32;
                        spike.viewport = Some(Viewport {
                            x: -step * 26.,
                            y: -step * 14.,
                            zoom: (0.35 + (index % 5) as f32 * 0.08).clamp(MIN_ZOOM, MAX_ZOOM),
                        });
                    }
                }
                Op::Fit => {
                    if let Some(fit) = spike.fit(RUN_FIT_PADDING) {
                        spike.viewport = Some(fit);
                    }
                }
                Op::Mark => run.marks.push(Instant::now()),
            }
        }
        let [selection, viewports, fit, done] = run.marks[..] else {
            unreachable!("a run records four marks");
        };
        let selection_churn_ms = milliseconds(selection, viewports);
        let viewport_ops_ms = milliseconds(viewports, fit);
        let fit_view_ms = milliseconds(fit, done);
        let (rendered_node_count, rendered_edge_count) = spike.rendered.get();
        let result = RunResult {
            framework: "gpui",
            node_count: spike.graph.nodes.len(),
            edge_count: spike.graph.edges.len(),
            // The first-render callbacks run before the run's first settled frame.
            initial_render_ms: spike
                .initial_render_ms
                .unwrap_or_else(|| milliseconds(spike.opened, selection)),
            selection_churn_ms,
            viewport_ops_ms,
            fit_view_ms,
            total_interaction_ms: selection_churn_ms + viewport_ops_ms + fit_view_ms,
            used_js_heap_before_bytes: None,
            used_js_heap_after_bytes: None,
            rendered_node_count,
            rendered_edge_count,
        };
        spike.result = serde_json::to_string_pretty(&result)
            .ok()
            .map(SharedString::from);
        cx.notify();
    }
}
