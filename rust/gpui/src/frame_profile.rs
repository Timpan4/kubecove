//! Opt-in element timings for diagnosing frame work without kernel counters.
use gpui_kit::AnyElement;
#[cfg(feature = "frame-profile")]
use std::cell::RefCell;
use std::{cell::Cell, ops::RangeInclusive, time::Instant};

thread_local! {
    static TRACK_MAP_PULSE: Cell<bool> = const { Cell::new(false) };
    static MAP_PULSE_INVALIDATION: Cell<Option<RangeInclusive<Instant>>> = const { Cell::new(None) };
}

/// Enables map pulse attribution for an action sample on this UI thread.
pub fn track_map_pulse_invalidation(enabled: bool) {
    TRACK_MAP_PULSE.set(enabled);
    MAP_PULSE_INVALIDATION.set(None);
}

pub fn take_map_pulse_invalidation() -> Option<RangeInclusive<Instant>> {
    MAP_PULSE_INVALIDATION.take()
}

pub(crate) fn map_pulse_invalidation(notify: impl FnOnce()) {
    if TRACK_MAP_PULSE.get() {
        let start = Instant::now();
        notify();
        MAP_PULSE_INVALIDATION.set(Some(start..=Instant::now()));
    } else {
        notify();
    }
}

#[derive(serde::Serialize)]
pub struct Span {
    sample: u64,
    component: &'static str,
    phase: &'static str,
    ms: f64,
}
#[cfg(feature = "frame-profile")]
thread_local! {
    static SPANS: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
    static RECORDING: Cell<bool> = const { Cell::new(true) };
    static SAMPLE: Cell<u64> = const { Cell::new(0) };
}
pub fn set_recording(enabled: bool) {
    #[cfg(feature = "frame-profile")]
    {
        RECORDING.set(enabled);
        if enabled {
            SAMPLE.set(SAMPLE.get() + 1);
        }
    }
    #[cfg(not(feature = "frame-profile"))]
    let _ = enabled;
}

pub struct Scope {
    #[cfg(feature = "frame-profile")]
    sample: u64,
    #[cfg(feature = "frame-profile")]
    component: &'static str,
    #[cfg(feature = "frame-profile")]
    phase: &'static str,
    #[cfg(feature = "frame-profile")]
    start: Instant,
    #[cfg(feature = "frame-profile")]
    recording: bool,
}
pub fn scope(component: &'static str, phase: &'static str) -> Scope {
    #[cfg(feature = "frame-profile")]
    {
        Scope {
            sample: SAMPLE.get(),
            component,
            phase,
            start: Instant::now(),
            recording: RECORDING.get(),
        }
    }
    #[cfg(not(feature = "frame-profile"))]
    {
        let _ = (component, phase);
        Scope {}
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        #[cfg(feature = "frame-profile")]
        if self.recording {
            SPANS.with_borrow_mut(|spans| {
                spans.push(Span {
                    sample: self.sample,
                    component: self.component,
                    phase: self.phase,
                    ms: self.start.elapsed().as_secs_f64() * 1000.,
                })
            });
        }
    }
}
pub fn take() -> Vec<Span> {
    #[cfg(feature = "frame-profile")]
    {
        SAMPLE.set(0);
        SPANS.with_borrow_mut(std::mem::take)
    }
    #[cfg(not(feature = "frame-profile"))]
    {
        Vec::new()
    }
}
pub(crate) fn element(component: &'static str, element: AnyElement) -> AnyElement {
    #[cfg(feature = "frame-profile")]
    {
        use gpui_kit::IntoElement;
        Profiled { component, element }.into_any_element()
    }
    #[cfg(not(feature = "frame-profile"))]
    {
        let _ = component;
        element
    }
}
#[cfg(feature = "frame-profile")]
struct Profiled {
    component: &'static str,
    element: AnyElement,
}
#[cfg(feature = "frame-profile")]
impl gpui_kit::IntoElement for Profiled {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
#[cfg(feature = "frame-profile")]
impl gpui_kit::Element for Profiled {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<gpui_kit::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) -> (gpui_kit::LayoutId, ()) {
        let _span = scope(self.component, "request_layout");
        (self.element.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) {
        let _span = scope(self.component, "prepaint");
        let _ = self.element.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) {
        let _span = scope(self.component, "paint");
        self.element.paint(window, cx);
    }
}
