//! Preserve layout while skipping preparation and paint outside ancestor clips.

use gpui_kit::{
    AnyElement, App, Bounds, Edges, Element, ElementId, GlobalElementId, InspectorElementId,
    LayoutId, Pixels, Window, px,
};

pub(super) struct Clipped {
    element: AnyElement,
    overflow: Edges<Pixels>,
}

impl Clipped {
    pub(super) fn new(element: AnyElement, overflow: Edges<Pixels>) -> Self {
        Self { element, overflow }
    }
}

impl gpui_kit::IntoElement for Clipped {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Clipped {
    type RequestLayoutState = ();
    type PrepaintState = bool;

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
        (self.element.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        mut bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        bounds.origin.x -= self.overflow.left;
        bounds.origin.y -= self.overflow.top;
        bounds.size.width += self.overflow.left + self.overflow.right;
        bounds.size.height += self.overflow.top + self.overflow.bottom;
        let clip = window.content_mask().bounds;
        let visible =
            clip.size.width > px(0.) && clip.size.height > px(0.) && bounds.intersects(&clip);
        if visible {
            let _ = self.element.prepaint(window, cx);
        }
        visible
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        visible: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *visible {
            self.element.paint(window, cx);
        }
    }
}
