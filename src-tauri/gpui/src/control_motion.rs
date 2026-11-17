//! Short, finite motion for state changes on controls, as CSS transitions run it
//! on the web: a switch, a segment, a tab or a hovered row moves to its new state
//! instead of jumping.
//!
//! [`eased`] builds an element from the progress of a state the caller knows,
//! and [`HoverFill`] paints a background that follows the pointer. Both keep their
//! state per element, start settled, and ask for frames only while they move, so
//! an idle window draws nothing. Time comes from the executor clock, so test
//! drivers move it, and `reduce_motion` makes every change instant.
use gpui_kit::base::{Easing, MotionStatus, Transition, transition_with_status};
use gpui_kit::{
    AnyElement, App, BorderStyle, Bounds, BoxShadow, Corners, DispatchPhase, Element, ElementId,
    Global, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement,
    LayoutId, MouseMoveEvent, Pixels, Point, Position, Rgba, Style, Window, px, quad,
};
use std::collections::HashMap;
use std::time::Duration;

mod slide;
pub use slide::{Slide, SlideIndicator};

/// Tailwind's `--default-transition-duration` in `node_modules/tailwindcss/theme.css`,
/// which `transition-colors`, `transition-transform` and `transition-opacity` use.
/// `SegmentedControl.svelte` names the same value as `duration-150`.
pub const DURATION: Duration = Duration::from_millis(150);

/// Tailwind's `--default-transition-timing-function`: `cubic-bezier(0.4, 0, 0.2, 1)`.
const EASING: Easing = Easing::CubicBezier {
    x1: 0.4,
    y1: 0.,
    x2: 0.2,
    y2: 1.,
};

/// [`EASING`] at `t` of the duration, for motion that render code samples
/// itself instead of leaving to an element.
pub fn curve(t: f32) -> f32 {
    gpui_kit::base::animation::cubic_bezier(0.4, 0., 0.2, 1.)(t)
}

/// The color `t` of the way from `from` to `to`. Channels mix premultiplied, as
/// CSS interpolates colors, so a fade from transparent keeps the target's hue.
pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    if t <= 0. {
        return from;
    }
    if t >= 1. {
        return to;
    }
    let (a, b) = (from.to_rgb(), to.to_rgb());
    let alpha = a.a + (b.a - a.a) * t;
    if alpha <= 0. {
        return gpui_kit::transparent_black();
    }
    let channel = |a_channel: f32, b_channel: f32| {
        (a_channel * a.a + (b_channel * b.a - a_channel * a.a) * t) / alpha
    };
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
    .into()
}

/// Tailwind's `shadow-sm` at `strength` of its opacity, which a box-shadow
/// transition passes through. GPUI's `shadow_sm` holds the same two layers.
pub fn shadow_sm(strength: f32) -> Vec<BoxShadow> {
    if strength <= 0. {
        return Vec::new();
    }
    let color = gpui_kit::hsla(0., 0., 0., 0.1 * strength);
    vec![
        BoxShadow::new(px(0.), px(1.), color).blur_radius(px(3.)),
        BoxShadow::new(px(0.), px(1.), color)
            .blur_radius(px(2.))
            .spread_radius(px(-1.)),
    ]
}

/// The transitions that moved in the last frame. Colors and painted fills are
/// not elements, so UI tests ask here.
#[derive(Default)]
pub struct Moving {
    /// The progress each moving transition showed, by element id. The view
    /// clears it before each frame, so it never holds a settled transition.
    progress: HashMap<ElementId, f32>,
    /// A test driver asked the next frame to end every transition.
    finish: bool,
    /// This frame ends every transition.
    finishing: bool,
    /// The state keys of the fills this frame painted. Two fills under one
    /// element id share one state and turn it back and forth without end as
    /// the pointer hovers one of them, so a debug build refuses the second.
    #[cfg(debug_assertions)]
    fills: std::collections::HashSet<GlobalElementId>,
}

impl Global for Moving {}

impl Moving {
    /// Forgets the last frame. The view calls this before it renders.
    pub fn begin_frame(cx: &mut App) {
        let stale = cx
            .try_global::<Self>()
            .is_some_and(|moving| !moving.progress.is_empty() || moving.finish || moving.finishing);
        if stale {
            let moving = cx.global_mut::<Self>();
            moving.progress.clear();
            moving.finishing = std::mem::take(&mut moving.finish);
        }
        #[cfg(debug_assertions)]
        if let Some(moving) = cx.try_global::<Self>()
            && !moving.fills.is_empty()
        {
            cx.global_mut::<Self>().fills.clear();
        }
    }

    #[cfg(debug_assertions)]
    fn claim_fill(key: &GlobalElementId, cx: &mut App) {
        assert!(
            cx.default_global::<Self>().fills.insert(key.clone()),
            "two hover fills share the state {key}: give the filled element an id"
        );
    }

    /// The progress of the transition `id` while it moves.
    #[doc(hidden)]
    pub fn progress(id: &ElementId, cx: &App) -> Option<f32> {
        cx.try_global::<Self>()?.progress.get(id).copied()
    }

    /// Ends every transition in the next frame, as waiting out `DURATION` does.
    /// UI test drivers call this where the web driver waits for the page to settle.
    #[doc(hidden)]
    pub fn finish(cx: &mut App) {
        cx.default_global::<Self>().finish = true;
        // A cached view paints again only when told to, and a transition ends
        // in a frame that paints it.
        cx.refresh_windows();
    }

    /// Whether this frame ends every transition, after a test driver's `finish`.
    pub(crate) fn finishing(cx: &App) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|moving| moving.finishing)
    }

    pub(crate) fn record(id: &ElementId, value: f32, cx: &mut App) {
        cx.default_global::<Self>()
            .progress
            .insert(id.clone(), value);
    }
}

/// The progress of `on`, from 0 to 1, of the transition `id` under the current
/// element. `gpui_kit`'s transition keeps the state: it starts settled, restarts
/// from the progress shown as CSS does, reads the executor clock, asks for the
/// next frame while it moves and jumps under reduced motion. `jump` ends it in
/// this frame. Tests read the progress under `name`.
fn advance(
    id: ElementId,
    name: &ElementId,
    on: bool,
    jump: bool,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let jump = jump || Moving::finishing(cx);
    let timing = Transition::new(if jump { Duration::ZERO } else { DURATION }).easing(EASING);
    let target = if on { 1. } else { 0. };
    let moved = transition_with_status(id, target, timing, window, cx);
    if matches!(moved.status, MotionStatus::Delayed | MotionStatus::Running) {
        Moving::record(name, moved.value, cx);
    }
    moved.value
}

/// The element `build` returns for the progress of `on`, from 0 to 1. `id` must
/// be unique among the transitions under the nearest element with an id.
pub fn eased<E: IntoElement>(
    id: impl Into<ElementId>,
    on: bool,
    build: impl FnOnce(f32) -> E + 'static,
) -> Eased {
    Eased {
        id: id.into(),
        on,
        build: Some(Box::new(move |progress| build(progress).into_any_element())),
    }
}

pub struct Eased {
    id: ElementId,
    on: bool,
    build: Option<Box<dyn FnOnce(f32) -> AnyElement>>,
}

impl IntoElement for Eased {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Eased {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    /// No id of its own: the built element keeps its place in the id path, so
    /// its state and its test locators stay where they were.
    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let value = advance(self.id.clone(), &self.id, self.on, false, window, cx);
        let mut element = self.build.take().expect("laid out once")(value);
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

/// The background of a hoverable element, as its first child: `rest`, fading to
/// `hover` while the pointer is over the element. The element itself sets no
/// background. Tests read the fade under the element's own id.
pub fn hover_fill(rest: Hsla, hover: Hsla) -> HoverFill {
    HoverFill {
        rest,
        hover,
        radius: px(0.),
        outset: [px(0.); 4],
        shadow: false,
    }
}

pub struct HoverFill {
    rest: Hsla,
    hover: Hsla,
    radius: Pixels,
    /// How far the fill reaches past the element's padding box: top, right,
    /// bottom, left.
    outset: [Pixels; 4],
    shadow: bool,
}

/// What a fill's last paint saw: where the pointer was and whether it hovered.
struct Seen {
    pointer: Point<Pixels>,
    hovered: bool,
}

impl HoverFill {
    /// The element's corner radius.
    pub fn rounded(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// The element's border widths, so the fill covers the border box as a
    /// background does.
    pub fn under_border(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.outset = [px(top), px(right), px(bottom), px(left)];
        self
    }

    /// `hover:shadow-sm`.
    pub fn shadow_sm(mut self) -> Self {
        self.shadow = true;
        self
    }
}

impl IntoElement for HoverFill {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for HoverFill {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("hover-fill".into()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let [top, right, bottom, left] = self.outset;
        let mut style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        style.inset.top = (-top).into();
        style.inset.right = (-right).into();
        style.inset.bottom = (-bottom).into();
        style.inset.left = (-left).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        _: &mut App,
    ) -> Hitbox {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let global_id = global_id.expect("the fill has an id");
        #[cfg(debug_assertions)]
        Moving::claim_fill(global_id, cx);
        // A drag hovers nothing, as GPUI's own hover styles hold.
        let hovered = hitbox.is_hovered(window) && !cx.has_active_drag();
        // The filled element's id, or the fill's own under an element without one.
        let name = global_id
            .iter()
            .rev()
            .nth(1)
            .or(global_id.last())
            .cloned()
            .expect("the fill's id is in its path");
        // Content that moves under a still pointer, as a scrolled row does,
        // changes color at once. Only the pointer's own move starts a fade, so
        // scrolling leaves no fading rows behind the pointer. Chromium shows
        // none either: it does not restyle hover while a page scrolls.
        let pointer = window.mouse_position();
        let carried = window.with_element_state(global_id, |seen: Option<Seen>, _| {
            let carried =
                seen.is_some_and(|seen| seen.pointer == pointer && seen.hovered != hovered);
            (carried, Seen { pointer, hovered })
        });
        let value = advance("fade".into(), &name, hovered, carried, window, cx);
        let radii = Corners::all(self.radius).clamp_radii_for_quad_size(bounds.size);
        if self.shadow {
            window.paint_drop_shadows(bounds, radii, &shadow_sm(value));
        }
        let color = mix(self.rest, self.hover, value);
        if color.a > 0. {
            window.paint_quad(quad(
                bounds,
                radii,
                color,
                px(0.),
                gpui_kit::transparent_black(),
                BorderStyle::default(),
            ));
        }
        // The pointer crossing the edge redraws the view, which starts the fade.
        let (hitbox, view) = (hitbox.clone(), window.current_view());
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
            let hovers = hitbox.is_hovered(window) && !cx.has_active_drag();
            if phase == DispatchPhase::Capture && hovers != hovered {
                cx.notify(view);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The shared `control-motion.*` parity scenarios cover that a control moves
    // over time and settles, and `tests/native/motion.rs` covers the curve, the
    // settled first frame and reduced motion, which need a window. What neither
    // sees is a color: a fade from transparent that passes through black.
    #[test]
    fn fades_in_from_transparent_at_the_target_color() {
        let color = gpui_kit::hsla(0.6, 0.5, 0.5, 0.8);
        let half = mix(gpui_kit::transparent_black(), color, 0.5).to_rgb();
        let full = color.to_rgb();
        assert!((half.a - 0.4).abs() < 1e-6);
        for (mixed, target) in [(half.r, full.r), (half.g, full.g), (half.b, full.b)] {
            assert!((mixed - target).abs() < 1e-3, "{mixed} is not {target}");
        }
    }
}
