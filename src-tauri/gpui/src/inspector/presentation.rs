//! Native presentation timings for the latency report, and the interaction
//! recorder's input hooks. The web's counterparts are the frontend spans and
//! render counters in `src/lib/diagnostics.ts` and `interaction-recorder.ts`.
//!
//! - `<view>.present` spans run from the request that changes the page or the
//!   detail tab to the end of painting the first frame that draws it. A
//!   zero-size probe painted last in the window marks that point. It exists
//!   only while a span or input-to-paint time waits, or while recording.
//! - `app.render`, `<page>.render` and `detail.render` count Inspector renders
//!   while that view shows. GPUI renders the window's whole element tree in one
//!   render, so a count is window renders while the view showed, where the web
//!   counts renders of that component alone.
use super::{
    Inspector, Page,
    detail::{Detail, DetailTab},
};
use crate::{
    diagnostics::Timings,
    interactions::{Kind, Recorder},
    targets,
};
use gpui_kit::{
    AnyElement, App, DispatchPhase, KeyDownEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollEvent, Window, canvas, prelude::*,
};
use std::{cell::RefCell, rc::Rc, time::Instant};

const UNATTRIBUTED: &str = "(unattributed)";

#[derive(Default)]
pub(super) struct Presentation {
    pub(super) recorder: Rc<RefCell<Recorder>>,
    /// The last interaction-report action's result, shown under the report.
    pub(super) recorder_status: Option<String>,
    shown: Option<(Page, Option<DetailTab>)>,
    requested: Option<Instant>,
}

/// The page's span and render counter names.
fn page_names(page: Page) -> (&'static str, &'static str) {
    match page {
        Page::Overview => ("overview.present", "overview.render"),
        Page::Resources => ("resources.present", "resources.render"),
        Page::ClusterOverview => ("cluster-overview.present", "cluster-overview.render"),
        Page::Incidents => ("incidents.present", "incidents.render"),
        Page::PortForwards => ("port-forwards.present", "port-forwards.render"),
        Page::Rbac => ("rbac.present", "rbac.render"),
        Page::Settings => ("settings.present", "settings.render"),
        Page::GitOps => ("gitops.present", "gitops.render"),
        Page::Helm => ("helm.present", "helm.render"),
        Page::Namespaces => ("namespaces.present", "namespaces.render"),
    }
}

fn tab_span(tab: DetailTab) -> &'static str {
    match tab {
        DetailTab::Details => "detail.details.present",
        DetailTab::Events => "detail.events.present",
        DetailTab::Logs => "detail.logs.present",
        DetailTab::Revisions => "detail.revisions.present",
        DetailTab::Argo => "detail.argo.present",
        DetailTab::Actions => "detail.actions.present",
        DetailTab::Yaml => "detail.yaml.present",
        DetailTab::Forward => "detail.forward.present",
        DetailTab::Exec => "detail.exec.present",
    }
}

fn hovered(window: &Window) -> String {
    targets::hovered(window).unwrap_or_else(|| UNATTRIBUTED.to_owned())
}

/// Window-level input listeners for the frame being painted, in the capture
/// phase so a control that stops propagation is still recorded.
fn listen(recorder: &Rc<RefCell<Recorder>>, timings: Option<Timings>, window: &mut Window) {
    let down = recorder.clone();
    let down_timings = timings.clone();
    window.on_mouse_event(move |_: &MouseDownEvent, phase, window, _| {
        if phase == DispatchPhase::Capture {
            let mut recorder = down.borrow_mut();
            recorder.pointer(true);
            let target = hovered(window);
            recorder.begin(
                Kind::Click,
                target,
                Some("pointerdown"),
                Instant::now(),
                down_timings.as_ref(),
            );
        }
    });
    let up = recorder.clone();
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, _| {
        if phase == DispatchPhase::Capture {
            let mut recorder = up.borrow_mut();
            recorder.pointer(false);
            recorder.measure("click");
        }
    });
    let moved = recorder.clone();
    window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, _| {
        if phase == DispatchPhase::Capture {
            moved.borrow_mut().pointer_moved();
        }
    });
    let wheel = recorder.clone();
    window.on_mouse_event(move |_: &ScrollEvent, phase, window, _| {
        if phase != DispatchPhase::Capture {
            return;
        }
        let mut recorder = wheel.borrow_mut();
        if recorder.current_kind() != Some(Kind::Scroll) {
            let target = hovered(window);
            recorder.begin(
                Kind::Scroll,
                target,
                Some("wheel"),
                Instant::now(),
                timings.as_ref(),
            );
        }
    });
}

/// Repeats once per display frame while `session` records, as the web's
/// `requestAnimationFrame` loop does.
fn tick(recorder: Rc<RefCell<Recorder>>, session: u64, window: &Window) {
    window.on_next_frame(move |window, _| {
        {
            let mut state = recorder.borrow_mut();
            if !state.recording() || state.session() != session {
                return;
            }
            state.frame(Instant::now(), window.is_window_active());
        }
        tick(recorder, session, window);
    });
}

impl Inspector {
    /// Starts a view change's span. The render that shows the new view ends the request.
    pub(super) fn request_view(&mut self) {
        self.presentation.requested.get_or_insert_with(Instant::now);
    }

    fn diagnostics_timings(&self) -> Option<Timings> {
        self.persisted
            .preferences
            .debug_mode_enabled
            .then(|| self.timings.clone())
    }

    pub(super) fn set_interaction_recording(&mut self, recording: bool, window: &mut Window) {
        let timings = self.diagnostics_timings();
        let mut recorder = self.presentation.recorder.borrow_mut();
        if recording {
            recorder.start();
            // Elements painted before the start, and cached ones, register on a full redraw.
            targets::set_recording(true);
            window.refresh();
            tick(
                self.presentation.recorder.clone(),
                recorder.session(),
                window,
            );
        } else {
            recorder.stop(timings.as_ref());
            targets::set_recording(false);
        }
    }

    /// Records a key press on the focused element. Never the key itself.
    pub(super) fn record_key(&self, event: &KeyDownEvent, window: &Window, cx: &App) {
        let mut recorder = self.presentation.recorder.borrow_mut();
        if !recorder.recording() || event.is_held && recorder.current_kind() == Some(Kind::Key) {
            return;
        }
        let target = targets::focused(window, cx).unwrap_or_else(|| "(window)".to_owned());
        recorder.begin(
            Kind::Key,
            target,
            Some("keydown"),
            Instant::now(),
            self.diagnostics_timings().as_ref(),
        );
    }

    /// Counts this render, starts spans for a changed view and returns the
    /// probe when this frame's paint ends a measurement or recording runs.
    pub(super) fn presentation_frame(&mut self, render_started: Instant) -> Option<AnyElement> {
        let tab = match &self.detail {
            Detail::Ready(detail) => Some(detail.tab),
            _ => None,
        };
        let view = (self.page, tab);
        let previous = self.presentation.shown.replace(view);
        let requested = self.presentation.requested.take();
        let timings = self.diagnostics_timings();
        let recording = self.presentation.recorder.borrow().recording();
        let (page_span, page_render) = page_names(self.page);
        let renders: &[&'static str] = if matches!(self.detail, Detail::Closed) {
            &["app.render", page_render]
        } else {
            &["app.render", page_render, "detail.render"]
        };
        if let Some(timings) = &timings {
            timings.count_renders(renders);
        }
        let mut spans = Vec::new();
        if previous.is_some_and(|(page, _)| page != view.0) {
            spans.push(page_span);
        }
        if let Some(tab) = tab
            && previous.is_some_and(|(_, shown)| shown != Some(tab))
        {
            spans.push(tab_span(tab));
        }
        let started = requested.unwrap_or(render_started);
        if recording {
            let mut recorder = self.presentation.recorder.borrow_mut();
            recorder.count_renders(&renders[1..]);
            if let Some(span) = spans.last() {
                let view = span.trim_end_matches(".present");
                recorder.begin(
                    Kind::Navigate,
                    format!("[view={view}]"),
                    Some("navigate"),
                    started,
                    timings.as_ref(),
                );
            }
        }
        if timings.is_none() {
            spans.clear();
        }
        if spans.is_empty() && !recording && !self.presentation.recorder.borrow().awaiting_paint() {
            return None;
        }
        let recorder = self.presentation.recorder.clone();
        let span_timings = self.timings.clone();
        Some(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    for span in spans {
                        span_timings.record_presentation(span, started);
                    }
                    recorder.borrow_mut().painted();
                    if recording {
                        listen(&recorder, timings, window);
                    }
                },
            )
            .absolute()
            .size_0()
            .into_any_element(),
        )
    }
}
