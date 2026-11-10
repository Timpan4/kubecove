//! One log line per completed scope or map load: wall time split into network
//! (some Kubernetes HTTP request in flight, process-wide) and processing.
//!
//! A load starts at its action and completes when the frame after its final
//! data is applied has been drawn. A superseded load is replaced before it
//! applies, so it never logs.

use super::Inspector;
use gpui_kit::Window;
use kubecove_lib::commands::helpers::http_activity::{HTTP_ACTIVITY, HttpActivitySnapshot};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(super) enum LoadKind {
    Scope,
    Map,
}

impl LoadKind {
    fn label(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Map => "map",
        }
    }

    /// The `LoadingDelays` key of the indicator that stands in for the
    /// load's content (`resources.rs` and `map.rs`).
    fn indicator(self) -> &'static str {
        match self {
            Self::Scope => "resources",
            Self::Map => "map",
        }
    }
}

pub(super) struct LoadTrace {
    kind: LoadKind,
    context: String,
    namespace: String,
    started: Instant,
    http_start: HttpActivitySnapshot,
    apply: Duration,
    render: Duration,
    awaiting_frame: bool,
}

impl LoadTrace {
    pub(super) fn start(kind: LoadKind, context: &str, namespace: Option<&str>) -> Self {
        Self {
            kind,
            context: context.to_owned(),
            namespace: namespace.unwrap_or("all").to_owned(),
            started: Instant::now(),
            http_start: HTTP_ACTIVITY.snapshot(),
            apply: Duration::ZERO,
            render: Duration::ZERO,
            awaiting_frame: false,
        }
    }

    /// Logs the line once the completing frame has been drawn. GPUI runs next-frame
    /// callbacks at the start of the following frame, so this is the earliest point
    /// after the completing paint that the window exposes.
    fn log(self) {
        let wall = self.started.elapsed();
        let http = HTTP_ACTIVITY.snapshot();
        let network = http.busy_since(self.http_start);
        let processing = wall.saturating_sub(network);
        eprintln!(
            "[kubecove:load] kind={} context={} namespace={} wall_ms={} network_ms={} processing_ms={} requests={} apply_ms={} render_ms={}",
            self.kind.label(),
            self.context,
            self.namespace,
            wall.as_millis(),
            network.as_millis(),
            processing.as_millis(),
            http.requests_since(self.http_start),
            self.apply.as_millis(),
            self.render.as_millis(),
        );
    }
}

impl Inspector {
    /// Adds the time one apply callback spent on the load's final data. `complete`
    /// marks the load as finished, so the next render records its frame.
    pub(super) fn note_apply(&mut self, kind: LoadKind, elapsed: Duration, complete: bool) {
        let slot = match kind {
            LoadKind::Scope => &mut self.scope_trace,
            LoadKind::Map => &mut self.map_trace,
        };
        if let Some(trace) = slot {
            trace.apply += elapsed;
            trace.awaiting_frame |= complete;
        }
    }

    /// Called at the end of each render. Traces whose data is applied are finished
    /// by the frame that draws it, and log once that frame is done. While the
    /// loading indicator still stands in for the content, the trace waits.
    pub(super) fn record_load_frames(&mut self, window: &mut Window, render: Duration) {
        let scope_held = self.loading.holds(LoadKind::Scope.indicator());
        let map_held = self.loading.holds(LoadKind::Map.indicator());
        for mut trace in [
            (!scope_held)
                .then(|| take_awaiting(&mut self.scope_trace))
                .flatten(),
            (!map_held)
                .then(|| take_awaiting(&mut self.map_trace))
                .flatten(),
        ]
        .into_iter()
        .flatten()
        {
            trace.render = render;
            window.on_next_frame(move |_, _| trace.log());
        }
    }
}

fn take_awaiting(slot: &mut Option<LoadTrace>) -> Option<LoadTrace> {
    if slot.as_ref().is_some_and(|trace| trace.awaiting_frame) {
        slot.take()
    } else {
        None
    }
}
