//! Continuous edge motion on the ownership map. The dashes of the selected
//! path move from owner to owned, and a pulse travels along each edge that
//! leads to a resource in progress. `App.css` runs the same two animations on
//! the web (`topology-edge-flow` and `topology-edge-pulse`).
//!
//! Only maps small enough to stay fully mounted move, and only in the active
//! window, as `edgeMotion` in `OwnershipMap.svelte` decides. Time comes from
//! the executor clock, so test drivers move it.
//!
//! Every frame renders the whole Inspector, and a selection can last for hours.
//! So the dashes get a frame only when they have moved one device pixel
//! (`flow_step`), from a timer. A pulse moves many pixels per frame and lasts
//! only as long as a rollout, so it asks for every frame, as the table's
//! activity icon does.
use super::Inspector;
use crate::topology::{EdgeState, FlowEdge, FlowItem, Segment};
use gpui_kit::{Context, Task, Window};
use std::time::{Duration, Instant};

/// The dash and the gap of an active edge in flow units (`stroke-dasharray: 8 8`).
pub(super) const DASH: f32 = 8.;
/// `topology-edge-flow` moves the dashes two dash periods in 1.8 s.
const FLOW_PERIOD: Duration = Duration::from_millis(900);
/// `topology-edge-pulse`: one trip per turn of the activity icon.
const PULSE_PERIOD: Duration = Duration::from_millis(1150);
/// `topology-edge-pulse` fades the pulse in until this share of a trip, and out after `PULSE_FADE_OUT`.
const PULSE_FADE_IN: f32 = 0.12;
const PULSE_FADE_OUT: f32 = 0.85;
/// Straight pieces per rounded corner. A corner's radius is at most 10 flow
/// units (`EDGE_RADIUS`), so a piece is under 2 units long.
const CORNER_PIECES: usize = 8;

/// This frame's phases, each from 0 to 1.
#[derive(Clone, Copy)]
pub(super) struct EdgeMotion {
    /// How far the dashes of an active edge have moved through one dash period.
    pub flow: f32,
    /// How far each pulse is along its edge. None while live updates are not
    /// current, as the activity icons pause then.
    pub pulse: Option<f32>,
}

#[derive(Default)]
pub(super) struct EdgeClock {
    /// The first frame of the current run of motion. Phases count from it.
    epoch: Option<Instant>,
    /// The motion of the frame being rendered, or None while no edge moves.
    motion: Option<EdgeMotion>,
    /// The wake for the dashes' next step and when it fires. None while a pulse
    /// asks for every frame.
    wake: Option<(Instant, Task<()>)>,
}

impl EdgeClock {
    pub(super) fn motion(&self) -> Option<EdgeMotion> {
        self.motion
    }
}

/// `buildFlowTopologyPulses`: a pulse leads to a resource in progress, and
/// edges off a selected path stay dimmed and get none.
pub(super) fn pulses(edge: &FlowEdge) -> bool {
    edge.progress && edge.state != EdgeState::Inactive
}

fn phase(elapsed: Duration, period: Duration) -> f32 {
    (elapsed.as_secs_f64() % period.as_secs_f64() / period.as_secs_f64()) as f32
}

/// How long the dashes take to move one device pixel, at `scale` device pixels
/// per flow unit. A dash period is two dashes long.
fn flow_step(scale: f32) -> Duration {
    FLOW_PERIOD.div_f32(DASH * 2. * scale)
}

/// `topology-edge-pulse`'s opacity at `turn` of a trip.
pub(super) fn pulse_opacity(turn: f32) -> f32 {
    if turn < PULSE_FADE_IN {
        turn / PULSE_FADE_IN
    } else if turn > PULSE_FADE_OUT {
        (1. - turn) / (1. - PULSE_FADE_OUT)
    } else {
        1.
    }
}

/// The dashes of an active edge of `length` at `phase` of a dash period, as
/// distances along the edge. At phase 0 the first dash starts the edge, as the
/// still dash array does.
pub(super) fn dashes(length: f32, phase: f32) -> impl Iterator<Item = (f32, f32)> {
    let period = DASH * 2.;
    let first = phase * period - period;
    (0u32..)
        .map(move |dash| first + dash as f32 * period)
        .take_while(move |&start| start < length)
        .filter_map(move |start| {
            let (start, end) = (start.max(0.), (start + DASH).min(length));
            (end > start).then_some((start, end))
        })
}

/// An edge's path as a polyline in flow units, with the distance to each point.
pub(super) struct Track {
    points: Vec<(f32, f32)>,
    /// `distances[i]` is the length of the path up to `points[i]`.
    distances: Vec<f32>,
}

impl Track {
    pub(super) fn new(segments: &[Segment]) -> Self {
        let mut points: Vec<(f32, f32)> = Vec::new();
        for segment in segments {
            match *segment {
                Segment::Move(x, y) | Segment::Line(x, y) => points.push((x, y)),
                Segment::Quad { control, to } => {
                    let from = points.last().copied().unwrap_or(control);
                    for piece in 1..=CORNER_PIECES {
                        let t = piece as f32 / CORNER_PIECES as f32;
                        let u = 1. - t;
                        points.push((
                            u * u * from.0 + 2. * u * t * control.0 + t * t * to.0,
                            u * u * from.1 + 2. * u * t * control.1 + t * t * to.1,
                        ));
                    }
                }
            }
        }
        let mut length = 0.;
        let mut previous = points.first().copied();
        let distances = points
            .iter()
            .map(|&point| {
                if let Some(from) = previous.replace(point) {
                    length += ((point.0 - from.0).powi(2) + (point.1 - from.1).powi(2)).sqrt();
                }
                length
            })
            .collect();
        Self { points, distances }
    }

    pub(super) fn length(&self) -> f32 {
        self.distances.last().copied().unwrap_or(0.)
    }

    /// The point `distance` along the path, held at its ends.
    pub(super) fn at(&self, distance: f32) -> Option<(f32, f32)> {
        let last = self.points.len().checked_sub(1)?;
        let next = self
            .distances
            .partition_point(|&reached| reached < distance)
            .min(last);
        if next == 0 {
            return Some(self.points[0]);
        }
        let (from, to) = (self.points[next - 1], self.points[next]);
        let span = self.distances[next] - self.distances[next - 1];
        let t = if span > 0. {
            ((distance - self.distances[next - 1]) / span).clamp(0., 1.)
        } else {
            0.
        };
        Some((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t))
    }

    /// Replaces `out` with the polyline from `start` to `end` along the path.
    pub(super) fn slice(&self, start: f32, end: f32, out: &mut Vec<(f32, f32)>) {
        out.clear();
        let (Some(first), Some(last)) = (self.at(start), self.at(end)) else {
            return;
        };
        let from = self.distances.partition_point(|&reached| reached <= start);
        let to = self.distances.partition_point(|&reached| reached < end);
        out.push(first);
        if from < to {
            out.extend_from_slice(&self.points[from..to]);
        }
        out.push(last);
    }
}

impl Inspector {
    /// Sets this frame's edge motion and arranges the next frame while an edge
    /// moves. Runs after `rebuild_map`, before the map renders.
    pub(in crate::inspector) fn sync_map_edge_motion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let live = self.map_updates_current();
        let view = self.map.cache.clone().filter(|view| {
            self.map_is_open()
                && window.is_window_active()
                && !cx.reduce_motion()
                && !super::viewport::culls(view)
        });
        let (flowing, pulsing) = view.as_ref().map_or((false, false), |view| {
            (
                view.edges
                    .iter()
                    .any(|edge| edge.state == EdgeState::Active),
                live && view.edges.iter().any(pulses),
            )
        });
        let Some(view) = view.filter(|_| flowing || pulsing) else {
            self.map.edge_clock = EdgeClock::default();
            return;
        };
        let now = cx.background_executor().now();
        let zoom = self.map_viewports(&view).1.map(|viewport| viewport.zoom);
        let clock = &mut self.map.edge_clock;
        let elapsed = now.duration_since(*clock.epoch.get_or_insert(now));
        clock.motion = Some(EdgeMotion {
            flow: phase(elapsed, FLOW_PERIOD),
            pulse: live.then(|| phase(elapsed, PULSE_PERIOD)),
        });
        if pulsing {
            clock.wake = None;
            cx.on_next_frame(window, |_, _, cx| cx.notify());
            return;
        }
        if clock.wake.as_ref().is_some_and(|(at, _)| *at > now) {
            return;
        }
        // Without a measured viewport nothing is drawn; its first size notifies.
        clock.wake = zoom.map(|zoom| {
            let step = flow_step(zoom * window.scale_factor());
            let task = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(step).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            });
            (now + step, task)
        });
    }

    /// The last frame's dash phase and pulse phase, and when the dashes' next
    /// step wakes. None while no edge moves. No element shows the clock, so its
    /// tests ask here.
    #[doc(hidden)]
    pub fn map_edge_clock(&self) -> Option<(f32, Option<f32>, Option<Instant>)> {
        let clock = &self.map.edge_clock;
        let wake = clock.wake.as_ref().map(|(at, _)| *at);
        clock.motion.map(|motion| (motion.flow, motion.pulse, wake))
    }

    /// Whether the dashes of the edge from the topology node `source` to `target`
    /// are moving, which is the last frame drawing them at a phase with their next
    /// frame arranged, and whether a pulse travels along it. Edges are painted,
    /// not elements, so UI tests ask here.
    #[doc(hidden)]
    pub fn map_edge_motion(&self, source: &str, target: &str) -> Option<(bool, bool)> {
        let (Some(view), super::TopologyLoad::Ready(topology)) =
            (&self.map.cache, &self.map.topology)
        else {
            return None;
        };
        let id = |index: usize| match &view.nodes[index].item {
            FlowItem::Resource(node) => Some(topology.nodes[*node].id.as_str()),
            FlowItem::Group { .. } => None,
        };
        let edge = view
            .edges
            .iter()
            .find(|edge| id(edge.source) == Some(source) && id(edge.target) == Some(target))?;
        let clock = &self.map.edge_clock;
        let pulsing = clock.motion.is_some_and(|motion| motion.pulse.is_some())
            && view.edges.iter().any(pulses);
        Some((
            clock.motion.is_some()
                && (pulsing || clock.wake.is_some())
                && edge.state == EdgeState::Active,
            pulsing && pulses(edge),
        ))
    }
}
