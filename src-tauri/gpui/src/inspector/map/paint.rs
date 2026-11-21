//! Smooth-step edges and closed arrows from `topology-implementation.ts`.
//!
//! Edges are painted, not elements. Their triangles are tessellated once per
//! view and zoom, relative to the map's flow origin. GPUI paths take no
//! transform and lyon flattens curves to a screen-space tolerance, so a zoom
//! tessellates again to keep the same triangles. Each frame translates them
//! by the pan and the canvas origin while copying them into the scene, and
//! paints each run of consecutive edges in one state as a single path. The
//! renderer rasterizes every path of a batch into one texture in paint order,
//! so a run of one color blends exactly as its edges did one by one.
//!
//! While edges move (`flow.rs`), the dashes of an active run are built again
//! each frame at their phase, and only its arrows come from the stored triangles.
use super::HANDLE_HALF;
use super::flow::{self, EdgeMotion, Track};
use crate::{
    theme::{Palette, Tokens},
    topology::{self, EdgeState, FlowNode, Segment, Viewport},
};
use gpui_kit::{
    Bounds, Hsla, Path, PathBuilder, PathVertex, Pixels, Point, Window, fill, point, px, size,
};
use std::cell::{OnceCell, RefCell};
use std::ops::Range;
use std::rc::Rc;

/// The pulse's dot and its halo in flow units, from `.ownership-map-pulse` and
/// `.ownership-map-pulse-halo` in `App.css`, and the halo's opacity.
const PULSE_DOT: f32 = 6.;
const PULSE_HALO: f32 = 12.;
const PULSE_HALO_OPACITY: f32 = 0.25;

struct EdgePaint {
    segments: Vec<Segment>,
    end: (f32, f32),
    state: EdgeState,
    /// A pulse travels along the edge while live updates are current.
    pulses: bool,
    bounds: (f32, f32, f32, f32),
    /// The path as a polyline, built the first time the edge moves.
    track: OnceCell<Track>,
}

impl EdgePaint {
    fn track(&self) -> &Track {
        self.track.get_or_init(|| Track::new(&self.segments))
    }
}

pub(super) struct PreparedEdges {
    source: Vec<EdgePaint>,
    geometry: RefCell<Option<Geometry>>,
    visible: RefCell<Option<VisibleViewport>>,
}

struct VisibleViewport {
    viewport: Option<Viewport>,
    canvas: (f32, f32),
    items: Rc<VisibleItems>,
}

pub(super) struct VisibleItems {
    pub nodes: Vec<usize>,
    pub edges: Vec<usize>,
}

/// Edge triangles at one zoom, in flow coordinates times the zoom. Edges are
/// tessellated the first time they are visible at that zoom.
struct Geometry {
    zoom: f32,
    /// Triangle corners of the tessellated edges, each stroke before its arrow.
    vertices: Vec<Point<Pixels>>,
    /// Per edge, once tessellated.
    edges: Vec<Option<Triangles>>,
}

/// One edge's place in `Geometry::vertices`.
struct Triangles {
    /// The stroke's corners, which end where the arrow's start.
    stroke: Range<usize>,
    arrow: Range<usize>,
    /// The bounds of the stroke and the arrow together.
    bounds: Bounds<Pixels>,
}

/// The triangles of an edge that a path takes.
#[derive(Clone, Copy)]
enum Part {
    StrokeAndArrow,
    Arrow,
}

impl Triangles {
    fn range(&self, part: Part) -> Range<usize> {
        match part {
            Part::StrokeAndArrow => self.stroke.start..self.arrow.end,
            Part::Arrow => self.arrow.clone(),
        }
    }
}

/// The flow-space box an edge stays inside: both node cards, which
/// viewport culling tests against the visible area.
pub(super) fn edge_bounds(source: &FlowNode, target: &FlowNode) -> (f32, f32, f32, f32) {
    (
        source.x.min(target.x),
        source.y.min(target.y),
        (source.x + source.width).max(target.x + target.width),
        (source.y + source.height).max(target.y + target.height),
    )
}

impl PreparedEdges {
    pub fn new(view: &topology::View) -> Self {
        let source = view
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
                    pulses: flow::pulses(edge),
                    bounds: edge_bounds(source, target),
                    track: OnceCell::new(),
                }
            })
            .collect();
        Self {
            source,
            geometry: RefCell::default(),
            visible: RefCell::default(),
        }
    }

    /// The owning graph has its own cache. Edge animation reuses its viewport
    /// membership while the canvas and transform stay fixed.
    pub fn visible(
        &self,
        view: &topology::View,
        viewport: Option<Viewport>,
        canvas: (f32, f32),
        shown: impl Fn((f32, f32, f32, f32)) -> bool,
    ) -> Rc<VisibleItems> {
        if let Some(cached) = self.visible.borrow().as_ref()
            && cached.viewport == viewport
            && cached.canvas == canvas
        {
            return cached.items.clone();
        }
        let items = Rc::new(VisibleItems {
            nodes: view
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| {
                    shown((node.x, node.y, node.x + node.width, node.y + node.height))
                        .then_some(index)
                })
                .collect(),
            edges: self
                .source
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| shown(edge.bounds).then_some(index))
                .collect(),
        });
        *self.visible.borrow_mut() = Some(VisibleViewport {
            viewport,
            canvas,
            items: items.clone(),
        });
        items
    }

    pub fn paint(
        &self,
        origin: Point<Pixels>,
        indices: &[usize],
        viewport: Viewport,
        motion: Option<EdgeMotion>,
        window: &mut Window,
    ) {
        let _span = crate::frame_profile::scope("map", "paint_edges");
        let mut geometry = self.geometry.borrow_mut();
        let geometry = match &mut *geometry {
            Some(geometry) if geometry.zoom == viewport.zoom => geometry,
            stale => stale.insert(Geometry {
                zoom: viewport.zoom,
                vertices: Vec::new(),
                edges: (0..self.source.len()).map(|_| None).collect(),
            }),
        };
        geometry.tessellate(&self.source, indices);
        let offset = point(origin.x + px(viewport.x), origin.y + px(viewport.y));
        let mut rest = indices;
        while let Some(&first) = rest.first() {
            let state = self.source[first].state;
            let run = rest
                .iter()
                .position(|&index| self.source[index].state != state)
                .unwrap_or(rest.len());
            let (color, width, _) = style(&state);
            let flowing = motion.filter(|_| state == EdgeState::Active);
            let part = if flowing.is_some() {
                Part::Arrow
            } else {
                Part::StrokeAndArrow
            };
            if let Some(path) = geometry.path(&rest[..run], offset, part) {
                window.paint_path(path, color);
            }
            if let Some(motion) = flowing
                && let Some(path) =
                    self.flowing_dashes(&rest[..run], offset, viewport.zoom, width, motion.flow)
            {
                window.paint_path(path, color);
            }
            rest = &rest[run..];
        }
        if let Some(turn) = motion.and_then(|motion| motion.pulse) {
            self.paint_pulses(indices, offset, viewport.zoom, turn, window);
        }
    }

    /// One stroke holding the dashes of the active edges of `indices` at `phase`
    /// of a dash period, in window coordinates.
    fn flowing_dashes(
        &self,
        indices: &[usize],
        offset: Point<Pixels>,
        zoom: f32,
        width: f32,
        phase: f32,
    ) -> Option<Path<Pixels>> {
        let at = |(x, y): (f32, f32)| point(offset.x + px(x * zoom), offset.y + px(y * zoom));
        let mut stroke = PathBuilder::stroke(px(width * zoom));
        let mut dash = Vec::new();
        let mut drawn = false;
        for &index in indices {
            let track = self.source[index].track();
            for (start, end) in flow::dashes(track.length(), phase) {
                track.slice(start, end, &mut dash);
                let Some((&first, rest)) = dash.split_first() else {
                    continue;
                };
                stroke.move_to(at(first));
                for &to in rest {
                    stroke.line_to(at(to));
                }
                drawn = true;
            }
        }
        drawn.then(|| stroke.build().ok()).flatten()
    }

    /// A dot and its halo on each pulsing edge of `indices`, `turn` of the way along it.
    fn paint_pulses(
        &self,
        indices: &[usize],
        offset: Point<Pixels>,
        zoom: f32,
        turn: f32,
        window: &mut Window,
    ) {
        let color = Palette::sky300();
        let opacity = flow::pulse_opacity(turn);
        for &index in indices {
            let edge = &self.source[index];
            if !edge.pulses {
                continue;
            }
            let track = edge.track();
            let Some((x, y)) = track.at(track.length() * turn) else {
                continue;
            };
            let center = point(offset.x + px(x * zoom), offset.y + px(y * zoom));
            for (diameter, alpha) in [(PULSE_HALO, PULSE_HALO_OPACITY), (PULSE_DOT, 1.)] {
                let diameter = px(diameter * zoom);
                window.paint_quad(
                    fill(
                        Bounds::centered_at(center, size(diameter, diameter)),
                        color.opacity(alpha * opacity),
                    )
                    .corner_radii(diameter / 2.),
                );
            }
        }
    }
}

impl Geometry {
    /// Tessellates the edges of `indices` that have no triangles at this zoom yet.
    fn tessellate(&mut self, edges: &[EdgePaint], indices: &[usize]) {
        if indices.iter().all(|&index| self.edges[index].is_some()) {
            return;
        }
        let _span = crate::frame_profile::scope("map", "prepare_edges");
        let zoom = self.zoom;
        let at = |(x, y): (f32, f32)| point(px(x * zoom), px(y * zoom));
        for &index in indices {
            if self.edges[index].is_some() {
                continue;
            }
            let edge = &edges[index];
            let vertices = &mut self.vertices;
            let start = vertices.len();
            let mut bounds: Option<Bounds<Pixels>> = None;
            let mut keep = |vertices: &mut Vec<Point<Pixels>>, path: Path<Pixels>| {
                if path.vertices.is_empty() {
                    return;
                }
                bounds = Some(bounds.map_or(path.bounds, |bounds| bounds.union(&path.bounds)));
                vertices.extend(path.vertices.iter().map(|vertex| vertex.xy_position));
            };
            let (_, width, marker) = style(&edge.state);
            let mut stroke = PathBuilder::stroke(px(width * zoom));
            if edge.state == EdgeState::Active {
                let dash = px(flow::DASH * zoom);
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
                keep(vertices, path);
            }
            let arrow_start = vertices.len();
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
                keep(vertices, path);
            }
            self.edges[index] = Some(Triangles {
                stroke: start..arrow_start,
                arrow: arrow_start..vertices.len(),
                bounds: bounds.unwrap_or_default(),
            });
        }
    }

    /// One path holding `part` of the triangles of `indices`, moved by `offset`.
    fn path(&self, indices: &[usize], offset: Point<Pixels>, part: Part) -> Option<Path<Pixels>> {
        let edges = || {
            indices.iter().map(|&index| {
                let edge = self.edges[index]
                    .as_ref()
                    .expect("visible edge tessellated");
                (edge.range(part), &edge.bounds)
            })
        };
        let count = edges().map(|(range, _)| range.len()).sum();
        if count == 0 {
            return None;
        }
        let mut path = Path::new(offset);
        path.vertices.reserve_exact(count);
        let mut bounds: Option<Bounds<Pixels>> = None;
        for (range, edge_bounds) in edges().filter(|(range, _)| !range.is_empty()) {
            bounds = Some(bounds.map_or(*edge_bounds, |bounds| bounds.union(edge_bounds)));
            // `PathBuilder` gives every triangle of a stroke or fill this `st`,
            // which the shader reads as a solid interior.
            path.vertices
                .extend(self.vertices[range].iter().map(|&xy| PathVertex {
                    xy_position: xy + offset,
                    st_position: point(0., 1.),
                    content_mask: Default::default(),
                }));
        }
        let bounds = bounds.expect("a nonempty run has bounds");
        path.bounds = Bounds {
            origin: bounds.origin + offset,
            size: bounds.size,
        };
        Some(path)
    }
}

fn style(state: &EdgeState) -> (Hsla, f32, f32) {
    let (color, width, opacity, marker) = match state {
        EdgeState::Idle => (Tokens::muted_foreground(), 1.6, 0.62, 14.),
        EdgeState::Active => (Tokens::primary(), 2.6, 1., 18.),
        EdgeState::Inactive => (Tokens::muted_foreground(), 1.6, 0.2, 14.),
    };
    (color.opacity(opacity), width, marker)
}
