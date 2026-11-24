//! Event state of the resource detail panel. The web reads events once the
//! details supply the UID (`ResourceDetailPanel.svelte`, `eventsQuery`); the
//! Details tab's incident summary and timeline and the Events tab share them.

use super::Inspector;
use super::detail::Detail;
use super::friendly_error::redact_detail;
use super::live_data::COALESCE;
use super::windowed::{Windowed, row_id};
use crate::incident::{self, EventFilter, SignalTone};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext, Context, Div, Entity, FontWeight, Hsla, SharedString, Subscription, Task, Window,
    div, prelude::*, px,
};
use kubecove_lib::models::{AppError, ResourceEventSummary};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

pub(super) enum EventsLoad {
    Pending,
    /// Sorted by `sortIncidentEvents`.
    Ready(Vec<ResourceEventSummary>),
    Failed(String),
}

pub(super) struct EventsState {
    pub load: EventsLoad,
    pub layout_key: u64,
    pub layout_source: Arc<[u8]>,
    pub search: Entity<InputState>,
    pub filter: EventFilter,
    /// `ResourceTimeline.svelte` expansion: entries follow the default
    /// (`expandTimelineByDefault`, off) unless toggled.
    pub default_open: bool,
    pub overrides: HashMap<String, bool>,
    /// The Events tab's rows.
    pub rows: Windowed,
    task: Option<Task<()>>,
    pub watch: Option<Task<()>>,
    pub watch_error: Option<String>,
    pub refresh_error: Option<String>,
    _search_changed: Subscription,
}

impl EventsState {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder("Filter message, reason, source..."));
        let _search_changed = cx.subscribe_in(&search, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            load: EventsLoad::Pending,
            layout_key: 0,
            layout_source: Arc::default(),
            search,
            filter: EventFilter::All,
            default_open: false,
            overrides: HashMap::new(),
            rows: Windowed::default(),
            task: None,
            watch: None,
            watch_error: None,
            refresh_error: None,
            _search_changed,
        }
    }

    pub fn events(&self) -> &[ResourceEventSummary] {
        match &self.load {
            EventsLoad::Ready(events) => events,
            EventsLoad::Pending | EventsLoad::Failed(_) => &[],
        }
    }

    fn set_ready(&mut self, mut events: Vec<ResourceEventSummary>) {
        incident::sort_events(&mut events);
        let source: Arc<[u8]> = serde_json::to_vec(&events)
            .expect("event models serialize")
            .into();
        if self.layout_source != source {
            self.rows = Windowed::default();
        }
        self.layout_key = row_id(&source);
        self.layout_source = source;
        self.load = EventsLoad::Ready(events);
        self.refresh_error = None;
    }

    fn set_failed(&mut self, error: String) {
        let error = redact_detail(&error);
        if matches!(self.load, EventsLoad::Ready(_)) {
            self.refresh_error = Some(error);
        } else {
            self.load = EventsLoad::Failed(error);
        }
    }

    pub fn is_open(&self, key: &str) -> bool {
        self.overrides
            .get(key)
            .copied()
            .unwrap_or(self.default_open)
    }
}

impl Inspector {
    /// Reads the open resource's events, matched by the UID from its details.
    pub(super) fn load_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let uid = detail.data.metadata["uid"].as_str().map(str::to_owned);
        let resource = detail.data.summary.clone();
        let backend = self.backend.clone();
        let read = self
            .timings
            .measure("events", backend.events(resource.clone(), uid.clone()));
        detail.events.load = EventsLoad::Pending;
        detail.events.refresh_error = None;
        detail.events.layout_key = 0;
        detail.events.layout_source = Arc::default();
        detail.events.rows = Windowed::default();
        // A single slot keeps one trailing refresh while the detail reads or cools down.
        let (refresh, mut requested) = mpsc::channel(1);
        detail.events.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            if this
                .update(cx, |this, cx| this.apply_events_read(result, cx))
                .is_err()
            {
                return;
            }
            while requested.recv().await.is_some() {
                let started = Instant::now();
                let result = backend.events(resource.clone(), uid.clone()).await;
                if this
                    .update(cx, |this, cx| this.apply_events_read(result, cx))
                    .is_err()
                {
                    return;
                }
                cx.background_executor()
                    .timer(COALESCE.saturating_sub(started.elapsed()))
                    .await;
            }
        }));
        self.watch_events(refresh, window, cx);
    }

    pub(super) fn events_state(&mut self) -> Option<&mut EventsState> {
        match &mut self.detail {
            Detail::Ready(detail) => Some(&mut detail.events),
            _ => None,
        }
    }

    fn apply_events_read(
        &mut self,
        result: Result<Vec<ResourceEventSummary>, AppError>,
        cx: &mut Context<Self>,
    ) {
        let Some(events) = self.events_state() else {
            return;
        };
        match result {
            Ok(rows) => events.set_ready(rows),
            Err(error) => events.set_failed(error.message),
        }
        cx.notify();
    }
}

/// `ui::badge` with a text size and an optional leading `size-1.5` dot.
pub(super) fn badge(
    (border, background, foreground): (Hsla, Hsla, Hsla),
    size: f32,
    dot: Option<Hsla>,
    content: impl Into<SharedString>,
) -> Div {
    ui::badge_frame(border, background)
        .gap(px(4.))
        .when_some(dot, |badge, color| {
            badge.child(div().size(px(6.)).flex_none().rounded_full().bg(color))
        })
        .child(
            ui::text(content, size, size * 1.4)
                .flex_none()
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(foreground),
        )
}

/// `Badge variant="outline"`: `border-border bg-input/30 text-foreground`.
pub(super) fn outline() -> (Hsla, Hsla, Hsla) {
    (
        Tokens::border(),
        Tokens::input().opacity(0.3),
        Tokens::foreground(),
    )
}

/// `toneBadgeVariant` plus `CHIP_BADGE_STYLES` classes.
pub(super) fn tone_colors(tone: SignalTone) -> (Hsla, Hsla, Hsla) {
    let clear = gpui_kit::transparent_black();
    match tone {
        SignalTone::Neutral => (clear, Tokens::secondary(), Tokens::foreground()),
        SignalTone::Success => (
            Palette::emerald500(0.3),
            Palette::emerald500(0.15),
            Palette::emerald300(),
        ),
        SignalTone::Warning => (
            Palette::amber500(0.3),
            Palette::amber500(0.15),
            Palette::amber300(),
        ),
        SignalTone::Error => (
            clear,
            Tokens::destructive().opacity(0.2),
            Tokens::destructive(),
        ),
        SignalTone::Info => (
            Palette::sky500(0.3),
            Palette::sky500(0.15),
            Palette::sky300(),
        ),
    }
}
