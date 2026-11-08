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
use super::HANDLE_HALF;
use crate::{
    theme::Tokens,
    topology::{self, EdgeState, FlowNode, Segment, Viewport},
};
use gpui_kit::{Bounds, Hsla, Path, PathBuilder, PathVertex, Pixels, Point, Window, point, px};
use std::cell::RefCell;
use std::ops::Range;

struct EdgePaint {
    segments: Vec<Segment>,
    end: (f32, f32),
    state: EdgeState,
    bounds: (f32, f32, f32, f32),
}

pub(super) struct PreparedEdges {
    source: Vec<EdgePaint>,
    geometry: RefCell<Option<Geometry>>,
}

/// Edge triangles at one zoom, in flow coordinates times the zoom. Edges are
/// tessellated the first time they are visible at that zoom.
struct Geometry {
    zoom: f32,
    /// Triangle corners of the tessellated edges, each stroke before its arrow.
    vertices: Vec<Point<Pixels>>,
    /// Per edge, once tessellated: its range in `vertices` and the bounds of those corners.
    edges: Vec<Option<(Range<usize>, Bounds<Pixels>)>>,
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
                    bounds: edge_bounds(source, target),
                }
            })
            .collect();
        Self {
            source,
            geometry: RefCell::default(),
        }
    }

    /// The edges whose bounds pass `shown`, in paint order.
    pub fn visible(&self, shown: impl Fn((f32, f32, f32, f32)) -> bool) -> Vec<usize> {
        (0..self.source.len())
            .filter(|&index| shown(self.source[index].bounds))
            .collect()
    }

    pub fn paint(
        &self,
        origin: Point<Pixels>,
        indices: &[usize],
        viewport: Viewport,
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
            if let Some(path) = geometry.path(&rest[..run], offset) {
                window.paint_path(path, style(&state).0);
            }
            rest = &rest[run..];
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
            let mut keep = |path: Path<Pixels>| {
                if path.vertices.is_empty() {
                    return;
                }
                bounds = Some(bounds.map_or(path.bounds, |bounds| bounds.union(&path.bounds)));
                vertices.extend(path.vertices.iter().map(|vertex| vertex.xy_position));
            };
            let (_, width, marker) = style(&edge.state);
            let mut stroke = PathBuilder::stroke(px(width * zoom));
            if edge.state == EdgeState::Active {
                let dash = px(8. * zoom);
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
                keep(path);
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
                keep(path);
            }
            self.edges[index] = Some((start..vertices.len(), bounds.unwrap_or_default()));
        }
    }

    /// One path holding the triangles of `indices`, moved by `offset`.
    fn path(&self, indices: &[usize], offset: Point<Pixels>) -> Option<Path<Pixels>> {
        let edges = || {
            indices.iter().map(|&index| {
                self.edges[index]
                    .as_ref()
                    .expect("visible edge tessellated")
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
                .extend(self.vertices[range.clone()].iter().map(|&xy| PathVertex {
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
