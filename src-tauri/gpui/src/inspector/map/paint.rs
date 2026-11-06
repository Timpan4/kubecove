//! Smooth-step edges and closed arrows from `topology-implementation.ts`.
use crate::{
    theme::Tokens,
    topology::{EdgeState, Segment, Viewport},
};
use gpui_kit::{Hsla, Path, PathBuilder, Pixels, Point, point, px};
use std::cell::RefCell;

pub(super) struct EdgePaint {
    pub segments: Vec<Segment>,
    pub end: (f32, f32),
    pub state: EdgeState,
    pub bounds: (f32, f32, f32, f32),
}

pub(super) struct PreparedEdges {
    pub source: Vec<EdgePaint>,
    geometry: RefCell<Option<Geometry>>,
}

struct Geometry {
    origin: Point<Pixels>,
    viewport: Viewport,
    indices: Vec<usize>,
    paths: Vec<(usize, Path<Pixels>)>,
}

impl PreparedEdges {
    pub fn new(source: Vec<EdgePaint>) -> Self {
        Self {
            source,
            geometry: RefCell::default(),
        }
    }

    pub fn paint(
        &self,
        origin: Point<Pixels>,
        indices: &[usize],
        viewport: Viewport,
        window: &mut gpui_kit::Window,
    ) {
        let _span = crate::frame_profile::scope("map", "paint_edges");
        let mut geometry = self.geometry.borrow_mut();
        if geometry.as_ref().is_none_or(|previous| {
            previous.origin != origin
                || previous.viewport != viewport
                || previous.indices.as_slice() != indices
        }) {
            *geometry = Some(Geometry {
                origin,
                viewport,
                indices: indices.to_vec(),
                paths: build_edges(origin, &self.source, indices, viewport),
            });
        }
        for (index, path) in &geometry.as_ref().expect("edge geometry prepared").paths {
            let (color, _, _) = style(&self.source[*index].state);
            window.paint_path(path.clone(), color);
        }
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

fn build_edges(
    origin: Point<Pixels>,
    edges: &[EdgePaint],
    indices: &[usize],
    viewport: Viewport,
) -> Vec<(usize, Path<Pixels>)> {
    let _span = crate::frame_profile::scope("map", "prepare_edges");
    let at = |(x, y): (f32, f32)| {
        point(
            origin.x + px(x * viewport.zoom + viewport.x),
            origin.y + px(y * viewport.zoom + viewport.y),
        )
    };
    let mut paths = Vec::new();
    for &index in indices {
        let edge = &edges[index];
        let (_, width, marker) = style(&edge.state);
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
            paths.push((index, path));
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
            paths.push((index, path));
        }
    }
    paths
}
