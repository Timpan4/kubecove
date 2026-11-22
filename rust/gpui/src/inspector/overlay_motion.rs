//! Enter and exit motion for overlays: a fade, and for some a slide.
//!
//! The values are the ones written in the web's shared wrappers in
//! `src/components/ui/svelte/`, which use tw-animate-css's `enter` and `exit`
//! keyframes. `PopoverContent` and `TooltipContent` run them on the web.
//! `DialogContent`, `DialogOverlay`, `SheetContent` and `SelectContent` select
//! on `data-open` and `data-closed`, which bits-ui does not set, so the web
//! shows those without motion while this client animates them.
//!
//! The wrappers also zoom from 95%. GPUI cannot scale an element subtree. In
//! its place a popover slides from its trigger's side, and a centered dialog
//! only fades. The detail panel has no motion on the web; its enter is this
//! client's own.
//!
//! Render code samples an overlay with whether it is open and draws the surface
//! while the sample is `Some`, which covers the exit after it closed. Time comes
//! from the executor clock, so test drivers move it. A frame asks for the next
//! one only while a run is in progress, and reduced motion ends every run at once.
use super::Inspector;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, AnyView, App, AppContext, Bounds, ContentMask, Context,
    Element, ElementId, FocusHandle, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    ParentElement, Pixels, Point, Render, SharedString, Styled, Window,
    base::animation::cubic_bezier, div, point, px,
};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// tw-animate-css's `enter` and `exit` keyframes run for `--tw-duration`, or
/// 150 ms when the wrapper names none, with `--tw-ease`, or CSS `ease`.
#[derive(Clone, Copy)]
struct Spec {
    duration: Duration,
    curve: Curve,
    /// Where the closed surface sits relative to its place, in pixels.
    slide: (f32, f32),
}

#[derive(Clone, Copy)]
enum Curve {
    /// CSS `ease`.
    Ease,
    /// Tailwind's `ease-in-out`, `--ease-in-out` in `tailwindcss/theme.css`.
    EaseInOut,
}

impl Curve {
    fn at(self, t: f32) -> f32 {
        match self {
            Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.)(t),
            Self::EaseInOut => cubic_bezier(0.4, 0., 0.2, 1.)(t),
        }
    }
}

/// `slide-in-from-top-2` and its siblings in `SelectContent.svelte` and
/// `TooltipContent.svelte`: two of Tailwind's 4 px spacing steps, from the
/// trigger's side.
const TRIGGER_SLIDE: f32 = 8.;

/// `duration-100` in `DialogContent.svelte` and `DialogOverlay.svelte`.
const DIALOG: Spec = Spec {
    duration: Duration::from_millis(100),
    curve: Curve::Ease,
    slide: (0., 0.),
};
/// `PopoverContent.svelte` names no duration, so tw-animate-css's 150 ms
/// applies. The popover opens below its trigger.
const POPOVER_BELOW: Spec = Spec {
    duration: Duration::from_millis(150),
    curve: Curve::Ease,
    slide: (0., -TRIGGER_SLIDE),
};
/// A popover that opens above its trigger.
const POPOVER_ABOVE: Spec = Spec {
    slide: (0., TRIGGER_SLIDE),
    ..POPOVER_BELOW
};
/// `duration-100` in `SelectContent.svelte`, below its trigger.
const SELECT: Spec = Spec {
    duration: Duration::from_millis(100),
    curve: Curve::Ease,
    slide: (0., -TRIGGER_SLIDE),
};
/// `duration-200 ease-in-out` and, on the left side, `slide-in-from-left-10`
/// in `SheetContent.svelte`: ten 4 px spacing steps.
const SHEET_LEFT: Spec = Spec {
    duration: Duration::from_millis(200),
    curve: Curve::EaseInOut,
    slide: (-40., 0.),
};
/// `TooltipContent.svelte` names no duration, and it opens below its trigger.
const TOOLTIP: Spec = POPOVER_BELOW;
/// The detail panel enters from the window's right edge with the popover's
/// timing. The owner chose this on 2026-11-17; the web panel has no motion.
const DETAIL: Spec = Spec {
    slide: (TRIGGER_SLIDE, 0.),
    ..POPOVER_BELOW
};

/// The overlays that animate. Each has one track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Overlay {
    Palette,
    ArgoConnection,
    ArgoSyncConfirm,
    ExecLeave,
    Drawer,
    DrawerBackdrop,
    LiveSessions,
    LiveStatus,
    LogTail,
    UpdateStatus,
    Usage,
    ArgoRefresh,
    ArgoSyncOptions,
    ExecMenu,
    KindPicker,
    ContextFilter,
    NamespaceFilter,
    OwnerFilter,
    MapContext,
    /// Enter only: the panel leaves in the frame its resource closes.
    Detail,
}

impl Overlay {
    const COUNT: usize = Self::Detail as usize + 1;

    fn spec(self) -> Spec {
        match self {
            Self::Palette
            | Self::ArgoConnection
            | Self::ArgoSyncConfirm
            | Self::ExecLeave
            // `SheetContent.svelte` draws a `DialogOverlay` behind the sheet.
            | Self::DrawerBackdrop => DIALOG,
            Self::Drawer => SHEET_LEFT,
            // `MapContextSelect.svelte` is a `Select`.
            Self::MapContext => SELECT,
            Self::LiveSessions
            | Self::LiveStatus
            | Self::LogTail
            | Self::UpdateStatus
            | Self::ArgoRefresh
            | Self::ArgoSyncOptions
            | Self::ExecMenu
            | Self::KindPicker
            | Self::ContextFilter
            | Self::NamespaceFilter
            | Self::OwnerFilter => POPOVER_BELOW,
            // The usage popover opens upward from the footer.
            Self::Usage => POPOVER_ABOVE,
            Self::Detail => DETAIL,
        }
    }
}

/// One run toward open or closed.
#[derive(Clone, Copy)]
struct Track {
    open: bool,
    /// The shown share when the run started: 0 is closed and 1 is open.
    from: f32,
    start: Instant,
    /// Whether this frame sampled the overlay.
    seen: bool,
    /// The share the last sample drew.
    drawn: f32,
}

impl Track {
    fn closed(start: Instant) -> Self {
        Self {
            open: false,
            from: 0.,
            start,
            seen: true,
            drawn: 0.,
        }
    }

    fn target(&self) -> f32 {
        if self.open { 1. } else { 0. }
    }

    /// The shown share at `now` and whether the run still moves. A full run is
    /// the keyframes' `ease(t)` or `1 - ease(t)`. A run that starts part way,
    /// because the overlay changed its mind, covers the share that is left in
    /// the same share of the duration, where the web restarts from a keyframe.
    fn at(&self, now: Instant, spec: Spec) -> (f32, bool) {
        let target = self.target();
        let run = spec.duration.mul_f32((target - self.from).abs());
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= run {
            return (target, false);
        }
        let t = elapsed.as_secs_f32() / run.as_secs_f32();
        (self.from + (target - self.from) * spec.curve.at(t), true)
    }
}

/// An overlay to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shown {
    overlay: Overlay,
    share: f32,
    exiting: bool,
    slide: (f32, f32),
}

impl Shown {
    /// The overlay closed and plays its exit. It keeps no focus.
    pub(super) fn exiting(self) -> bool {
        self.exiting
    }

    /// The surface with this frame's fade and slide. An exiting surface takes
    /// no input: a click during the exit reaches what is behind it, and focus
    /// leaves the surface, so a key press does not reach its handlers.
    pub(super) fn surface(self, surface: impl IntoElement + Styled) -> Shifted {
        let away = 1. - self.share;
        Shifted {
            overlay: self.overlay,
            child: surface.opacity(self.share).into_any_element(),
            offset: point(px(self.slide.0 * away), px(self.slide.1 * away)),
            inert: self.exiting,
        }
    }
}

/// Draws its child moved by an offset, without changing the layout.
pub(super) struct Shifted {
    overlay: Overlay,
    child: AnyElement,
    offset: Point<Pixels>,
    inert: bool,
}

impl IntoElement for Shifted {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Shifted {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // A handle on this element's dispatch node tells whether focus is
        // inside the surface. An exiting surface gives that focus up, so a
        // key press reaches the window as if the overlay had left the tree.
        let key = ElementId::from(("overlay-surface", self.overlay as usize));
        let within = window.with_global_id(key, |id, window| {
            window.with_element_state::<FocusHandle, _>(id, |handle, _| {
                let handle = handle.unwrap_or_else(|| cx.focus_handle());
                (handle.clone(), handle)
            })
        });
        window.set_focus_handle(&within, cx);
        if self.inert && within.contains_focused(window, cx) {
            window.blur(cx);
        }
        // A hitbox takes the pointer only inside the content mask it was
        // prepainted under, and painting applies its own mask later. An empty
        // mask here leaves the child drawn and out of reach.
        let mask = self.inert.then(|| ContentMask {
            bounds: Bounds::default(),
        });
        window.with_content_mask(mask, |window| {
            window.with_element_offset(self.offset, |window| self.child.prepaint(window, cx));
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// The app's text tooltip: the component library's, with `TooltipContent.svelte`'s
/// enter. GPUI drops a tooltip in the frame its trigger loses the pointer, so
/// it has no exit. GPUI's animation element reads the executor clock, asks for
/// frames only while it runs and shows the end state under reduced motion.
pub(super) struct Tooltip(SharedString);

impl Tooltip {
    pub(super) fn new(text: impl Into<SharedString>) -> Self {
        Self(text.into())
    }

    pub(super) fn build(self, window: &mut Window, cx: &mut App) -> AnyView {
        let content = gpui_kit::component::tooltip::Tooltip::new(self.0).build(window, cx);
        cx.new(|_| EnteringTooltip(content)).into()
    }
}

struct EnteringTooltip(AnyView);

impl Render for EnteringTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.0.clone()).with_animation(
            "enter",
            Animation::new(TOOLTIP.duration).with_easing(|t| TOOLTIP.curve.at(t)),
            |surface, share| {
                surface
                    .top(px(TOOLTIP.slide.1 * (1. - share)))
                    .opacity(share)
            },
        )
    }
}

struct Frame {
    now: Option<Instant>,
    reduce_motion: bool,
    tracks: [Option<Track>; Overlay::COUNT],
    running: bool,
}

pub(super) struct OverlayMotion {
    frame: RefCell<Frame>,
}

impl Default for OverlayMotion {
    fn default() -> Self {
        Self {
            frame: RefCell::new(Frame {
                now: None,
                reduce_motion: false,
                tracks: [None; Overlay::COUNT],
                running: false,
            }),
        }
    }
}

impl OverlayMotion {
    pub(super) fn begin_frame(&mut self, now: Instant, reduce_motion: bool) {
        let frame = self.frame.get_mut();
        frame.now = Some(now);
        frame.reduce_motion = reduce_motion;
        frame.running = false;
        for track in frame.tracks.iter_mut().flatten() {
            track.seen = false;
        }
    }

    /// What `overlay` draws this frame: `None` once it is closed and its exit
    /// has ended. Call it once per frame, wherever the overlay's host renders.
    pub(super) fn sample(&self, overlay: Overlay, open: bool) -> Option<Shown> {
        let mut frame = self.frame.borrow_mut();
        let Some(now) = frame.now else {
            return open.then_some(Shown {
                overlay,
                share: 1.,
                exiting: false,
                slide: (0., 0.),
            });
        };
        let spec = overlay.spec();
        let reduce_motion = frame.reduce_motion;
        let slot = &mut frame.tracks[overlay as usize];
        let mut track = match *slot {
            Some(track) if track.open == open => track,
            // The run turns around from where it is.
            Some(track) => Track {
                open,
                from: track.at(now, spec).0,
                ..Track::closed(now)
            },
            None if !open => return None,
            None => Track {
                open,
                ..Track::closed(now)
            },
        };
        track.seen = true;
        if reduce_motion {
            track.from = track.target();
        }
        let (share, running) = track.at(now, spec);
        track.drawn = share;
        if !open && !running {
            *slot = None;
            return None;
        }
        if !running {
            // A run that reached its target has nothing left for `finish` to end.
            track.from = track.target();
        }
        *slot = Some(track);
        frame.running |= running;
        Some(Shown {
            overlay,
            share,
            exiting: !open,
            slide: spec.slide,
        })
    }

    /// Forgets overlays whose host did not render, and reports whether a run
    /// needs another frame.
    fn end_frame(&mut self) -> bool {
        let frame = self.frame.get_mut();
        for slot in &mut frame.tracks {
            if slot.is_some_and(|track| !track.seen) {
                *slot = None;
            }
        }
        frame.now = None;
        frame.running
    }

    /// Ends every run at its target. Returns whether one was still moving.
    fn finish(&mut self) -> bool {
        let mut moved = false;
        for track in self.frame.get_mut().tracks.iter_mut().flatten() {
            moved |= track.from != track.target();
            track.from = track.target();
        }
        moved
    }
}

impl Inspector {
    pub(super) fn end_overlay_motion_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay_motion.end_frame() {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
    }

    /// Ends every overlay's enter or exit at its target, as waiting out its
    /// duration does. UI test drivers call this where the web driver waits for
    /// an overlay to settle. Returns whether a run was unfinished.
    #[doc(hidden)]
    pub fn finish_overlay_motion(&mut self, cx: &mut Context<Self>) -> bool {
        let moved = self.overlay_motion.finish();
        if moved {
            cx.notify();
        }
        moved
    }

    /// The opacity the last frame drew an overlay with, or `None` when it drew
    /// none, for UI tests that follow a run. `update-status` is the shared
    /// scenarios' name (`OVERLAY_NAMES` in `e2e/parity/contract-steps.ts`);
    /// the others serve `tests/native/overlay_motion.rs`.
    #[doc(hidden)]
    pub fn overlay_opacity(&self, name: &str) -> Option<f32> {
        let overlay = match name {
            "palette" => Overlay::Palette,
            "navigation-drawer" => Overlay::Drawer,
            "navigation-backdrop" => Overlay::DrawerBackdrop,
            "update-status" => Overlay::UpdateStatus,
            "kind-picker" => Overlay::KindPicker,
            "detail" => Overlay::Detail,
            other => panic!("no overlay is named {other}"),
        };
        self.overlay_motion.frame.borrow().tracks[overlay as usize].map(|track| track.drawn)
    }
}
