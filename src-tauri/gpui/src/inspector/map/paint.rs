//! Smooth-step edges and closed arrows from `topology-implementation.ts`.
use crate::{
    theme::Tokens,
    topology::{EdgeState, Segment, Viewport},
};
use gpui_kit::{PathBuilder, Pixels, Point, point, px};
pub(super) struct EdgePaint {
    pub segments: Vec<Segment>,
    pub end: (f32, f32),
    pub state: EdgeState,
}

pub(super) fn paint_edges(
    origin: Point<Pixels>,
    edges: &[EdgePaint],
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
