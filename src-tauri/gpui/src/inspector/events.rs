//! Event state of the resource detail panel. The web reads events once the
//! details supply the UID (`ResourceDetailPanel.svelte`, `eventsQuery`); the
//! Details tab's incident summary and timeline and the Events tab share them.

use super::Inspector;
use super::detail::Detail;
use super::windowed::{Windowed, row_id};
use crate::incident::{self, EventFilter, SignalTone};
use crate::theme::{Tokens, oklch};
use crate::ui;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext, Context, Div, Entity, FontWeight, Hsla, SharedString, Subscription, Task, Window,
    div, prelude::*, px,
};
use kubecove_lib::models::ResourceEventSummary;
use std::collections::HashMap;
use std::sync::Arc;

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
            _search_changed,
        }
    }

    pub fn events(&self) -> &[ResourceEventSummary] {
        match &self.load {
            EventsLoad::Ready(events) => events,
            EventsLoad::Pending | EventsLoad::Failed(_) => &[],
        }
    }

    /// Stores a complete read and the layout source the timeline cache compares.
    /// Every path that writes `EventsLoad::Ready` goes through here.
    pub fn set_ready(&mut self, mut events: Vec<ResourceEventSummary>) {
        incident::sort_events(&mut events);
        let source: Arc<[u8]> = serde_json::to_vec(&events)
            .expect("event models serialize")
            .into();
        self.layout_key = row_id(&source);
        self.layout_source = source;
        self.load = EventsLoad::Ready(events);
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
        let read = self.backend.events(detail.data.summary.clone(), uid);
        let read = self.timings.measure("events", read);
        detail.events.load = EventsLoad::Pending;
        detail.events.layout_key = 0;
        detail.events.layout_source = Arc::default();
        // The detail owns this read, so it only settles into the detail that started it.
        detail.events.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Detail::Ready(detail) = &mut this.detail {
                    detail.events.rows = Windowed::default();
                    match result {
                        Ok(events) => detail.events.set_ready(events),
                        Err(error) => detail.events.load = EventsLoad::Failed(error.message),
                    }
                }
                cx.notify();
            });
        }));
        self.watch_events(window, cx);
    }

    pub(super) fn events_state(&mut self) -> Option<&mut EventsState> {
        match &mut self.detail {
            Detail::Ready(detail) => Some(&mut detail.events),
            _ => None,
        }
    }
}

/// Tailwind v4 palette entries the event views use beyond `theme::Palette`.
pub(super) mod tw {
    use super::{Hsla, oklch};

    pub fn amber200() -> Hsla {
        oklch(0.924, 0.12, 95.746, 1.)
    }
    pub fn amber300() -> Hsla {
        oklch(0.879, 0.169, 91.605, 1.)
    }
    pub fn amber400(alpha: f32) -> Hsla {
        oklch(0.828, 0.189, 84.429, alpha)
    }
    pub fn amber500(alpha: f32) -> Hsla {
        oklch(0.769, 0.188, 70.08, alpha)
    }
    pub fn emerald200() -> Hsla {
        oklch(0.905, 0.093, 164.15, 1.)
    }
    pub fn emerald300() -> Hsla {
        oklch(0.845, 0.143, 164.978, 1.)
    }
    pub fn emerald400(alpha: f32) -> Hsla {
        oklch(0.765, 0.177, 163.223, alpha)
    }
    pub fn emerald500(alpha: f32) -> Hsla {
        oklch(0.696, 0.17, 162.48, alpha)
    }
    pub fn red500(alpha: f32) -> Hsla {
        oklch(0.637, 0.237, 25.331, alpha)
    }
    pub fn sky300() -> Hsla {
        oklch(0.828, 0.111, 230.318, 1.)
    }
    pub fn sky500(alpha: f32) -> Hsla {
        oklch(0.685, 0.169, 237.323, alpha)
    }
}

/// `Badge` (`h-5 rounded-full border px-2 font-medium whitespace-nowrap`) with
/// explicit colors, text size and an optional leading `size-1.5` dot.
pub(super) fn badge(
    (border, background, foreground): (Hsla, Hsla, Hsla),
    size: f32,
    dot: Option<Hsla>,
    content: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .overflow_hidden()
        .h(px(20.))
        .px(px(8.))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
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
        SignalTone::Success => (tw::emerald500(0.3), tw::emerald500(0.15), tw::emerald300()),
        SignalTone::Warning => (tw::amber500(0.3), tw::amber500(0.15), tw::amber300()),
        SignalTone::Error => (
            clear,
            Tokens::destructive().opacity(0.2),
            Tokens::destructive(),
        ),
        SignalTone::Info => (tw::sky500(0.3), tw::sky500(0.15), tw::sky300()),
    }
}
