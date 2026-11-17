//! One indicator that slides between the options of a group: the pill of a
//! segmented control and the line under a row of tabs.
//!
//! The group's active option reports its bounds while the frame is laid out,
//! and the indicator, painted before the options, eases to them. Only a newly
//! chosen option slides it. A layout change, such as a resized pane that wraps
//! the tabs, moves it at once.
use super::{DURATION, EASING, Moving, shadow_sm};
use gpui_kit::base::{MotionStatus, Transition, transition_with_status};
use gpui_kit::{
    AnyElement, App, BorderStyle, Bounds, Corners, Element, ElementId, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Position, Style, Window, point, px, quad,
    size,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// Joins a group's indicator to its active option for one frame.
#[derive(Clone, Default)]
pub struct Slide(Rc<Cell<Option<Bounds<Pixels>>>>);

impl Slide {
    pub fn new() -> Self {
        Self::default()
    }

    /// One option of the group. The indicator moves to the `active` one.
    pub fn option(&self, active: bool, option: impl IntoElement) -> AnyElement {
        let option = option.into_any_element();
        if !active {
            return option;
        }
        SlideTarget {
            option,
            slide: self.clone(),
        }
        .into_any_element()
    }

    /// The indicator, as the first child of a `relative` group. `id` must be
    /// unique among the indicators under the nearest element with an id, and
    /// `active` is the active option's place in the group.
    pub fn indicator(
        &self,
        id: impl Into<ElementId>,
        active: usize,
        color: Hsla,
    ) -> SlideIndicator {
        SlideIndicator {
            id: id.into(),
            active,
            color,
            radius: px(0.),
            shadow: false,
            line: None,
            slide: self.clone(),
        }
    }
}

/// The active option of a group. It lays out and paints as the option does.
struct SlideTarget {
    option: AnyElement,
    slide: Slide,
}

impl IntoElement for SlideTarget {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for SlideTarget {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
    ) -> (LayoutId, ()) {
        (self.option.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.option.prepaint(window, cx);
        self.slide.0.set(Some(bounds));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.option.paint(window, cx);
    }
}

pub struct SlideIndicator {
    id: ElementId,
    active: usize,
    color: Hsla,
    radius: Pixels,
    shadow: bool,
    /// A line this far under the option and this high, instead of its whole box.
    line: Option<(Pixels, Pixels)>,
    slide: Slide,
}

/// What the indicator's last paint saw, in the group's coordinates.
struct Placed {
    active: usize,
    /// Where the active option put the indicator.
    target: Bounds<Pixels>,
    /// Where the indicator was when its option last changed.
    from: Bounds<Pixels>,
    /// Where the indicator was painted.
    shown: Bounds<Pixels>,
}

impl SlideIndicator {
    /// The indicator's corner radius.
    pub fn rounded(mut self, radius: Pixels) -> Self {
        self.radius = radius;
        self
    }

    /// `shadow-sm`.
    pub fn shadow_sm(mut self) -> Self {
        self.shadow = true;
        self
    }

    /// A line `below` the option and `height` high, as wide as the option.
    pub fn line(mut self, below: Pixels, height: Pixels) -> Self {
        self.line = Some((below, height));
        self
    }
}

impl IntoElement for SlideIndicator {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for SlideIndicator {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
        let mut style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        style.inset.top = px(0.).into();
        style.inset.right = px(0.).into();
        style.inset.bottom = px(0.).into();
        style.inset.left = px(0.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // No option is active: nothing to mark.
        let Some(option) = self.slide.0.get() else {
            return;
        };
        let mut target = Bounds {
            origin: option.origin - bounds.origin,
            size: option.size,
        };
        if let Some((below, height)) = self.line {
            target.origin.y += target.size.height + below;
            target.size.height = height;
        }
        let global_id = global_id.expect("the indicator has an id");
        let finishing = Moving::finishing(cx);
        let (active, id) = (self.active, &self.id);
        let shown = window.with_element_state(global_id, |placed: Option<Placed>, window| {
            let (jump, from) = match &placed {
                // The first paint: the transitions start settled on the target.
                None => (false, target),
                Some(placed) if placed.active != active => (false, placed.shown),
                Some(placed) if placed.target != target => (true, target),
                Some(placed) => (false, placed.from),
            };
            let duration = if jump || finishing {
                Duration::ZERO
            } else {
                DURATION
            };
            let mut moving = false;
            let mut ease = |axis: &'static str, to: Pixels| {
                let timing = Transition::new(duration).easing(EASING);
                let axis = ElementId::Name(axis.into());
                let moved = transition_with_status(axis, f32::from(to), timing, window, cx);
                moving |= matches!(moved.status, MotionStatus::Delayed | MotionStatus::Running);
                px(moved.value)
            };
            let shown = Bounds {
                origin: point(ease("x", target.origin.x), ease("y", target.origin.y)),
                size: size(
                    ease("width", target.size.width),
                    ease("height", target.size.height),
                ),
            };
            if moving {
                Moving::record(id, travelled(from, target, shown), cx);
            }
            (
                shown,
                Placed {
                    active,
                    target,
                    from,
                    shown,
                },
            )
        });
        let painted = Bounds {
            origin: shown.origin + bounds.origin,
            size: shown.size,
        };
        let radii = Corners::all(self.radius).clamp_radii_for_quad_size(painted.size);
        if self.shadow {
            window.paint_drop_shadows(painted, radii, &shadow_sm(1.));
        }
        window.paint_quad(quad(
            painted,
            radii,
            self.color,
            px(0.),
            gpui_kit::transparent_black(),
            BorderStyle::default(),
        ));
    }
}

/// How far `shown` is from `from` to `to`, from 0 to 1, read on the edge that
/// travels furthest. Every edge follows one curve, so they agree.
fn travelled(from: Bounds<Pixels>, to: Bounds<Pixels>, shown: Bounds<Pixels>) -> f32 {
    let edges = |bounds: Bounds<Pixels>| {
        [
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height,
        ]
        .map(f32::from)
    };
    let (from, to, shown) = (edges(from), edges(to), edges(shown));
    let mut progress = 1.;
    let mut furthest = 0.;
    for edge in 0..4 {
        let travel = to[edge] - from[edge];
        if travel.abs() > furthest {
            furthest = travel.abs();
            progress = (shown[edge] - from[edge]) / travel;
        }
    }
    progress
}
