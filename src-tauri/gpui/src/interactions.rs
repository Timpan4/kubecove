//! Opt-in interaction recorder, the native counterpart of
//! `src/lib/interaction-recorder.ts`, with the same JSON line per interaction.
//!
//! Each click, drag, key, scroll and navigation starts one entry that lasts
//! until the next. Targets come from `crate::targets`: element ids, never
//! labels, text, typed characters or cluster data. Entries stay in memory.
//!
//! Native meanings that differ from the web's:
//! - Frame deltas come from a chained next-frame callback, the web's
//!   `requestAnimationFrame` loop, so both pace with the display.
//! - Input-to-paint ends when the first frame drawn after the input finishes
//!   painting. An input that draws nothing records no entry, where the web
//!   records the next frame.
//! - GPUI rebuilds its element tree each frame and has no DOM, so `dom` counts
//!   stay zero and `dom.sources` lists renders per view during the entry.
use crate::diagnostics::{TimingKind, Timings};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Instant,
};

// Same retention as frontend diagnostic events, as on the web.
const MAX_RECORDED: usize = 500;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Click,
    Drag,
    Key,
    Scroll,
    Navigate,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Drag => "drag",
            Self::Key => "key",
            Self::Scroll => "scroll",
            Self::Navigate => "navigate",
        }
    }
}

/// Frame deltas in 0.1 ms buckets, as `interaction-frames.ts`.
#[derive(Default, Clone)]
struct Histogram {
    buckets: BTreeMap<u32, u32>,
    total: u32,
}

impl Histogram {
    fn add(&mut self, delta_ms: f64) {
        *self
            .buckets
            .entry((delta_ms * 10.).round() as u32)
            .or_default() += 1;
        self.total += 1;
    }

    fn percentile(&self, fraction: f64) -> f64 {
        let mut seen = 0;
        for (bucket, count) in &self.buckets {
            seen += count;
            if f64::from(seen) > f64::from(self.total) * fraction {
                return f64::from(*bucket) / 10.;
            }
        }
        0.
    }
}

/// One recorded interaction, `RecordedInteraction` in `interaction-recorder.ts`.
pub(crate) struct Interaction {
    id: u64,
    pub(crate) kind: Kind,
    wall_time: chrono::DateTime<chrono::Utc>,
    pub(crate) target: String,
    started: Instant,
    duration_ms: f64,
    input_to_paint: Vec<(&'static str, f64)>,
    frames: Histogram,
    refresh_ms: f64,
    renders: BTreeMap<&'static str, u32>,
    commands: Option<Vec<Value>>,
}

fn round(value: f64) -> f64 {
    (value * 10.).round() / 10.
}

impl Interaction {
    pub(crate) fn dropped_frames(&self) -> u32 {
        if self.refresh_ms <= 0. {
            return 0;
        }
        self.frames
            .buckets
            .iter()
            .map(|(bucket, count)| {
                let ratio = (f64::from(*bucket) / 10. / self.refresh_ms).round();
                count * (ratio as u32).saturating_sub(1)
            })
            .sum()
    }

    pub(crate) fn max_frame_ms(&self) -> f64 {
        self.frames
            .buckets
            .keys()
            .next_back()
            .map_or(0., |bucket| f64::from(*bucket) / 10.)
    }

    pub(crate) fn max_input_to_paint_ms(&self) -> f64 {
        self.input_to_paint
            .iter()
            .fold(0., |max, (_, ms)| f64::max(max, *ms))
    }

    pub(crate) fn local_time(&self) -> String {
        self.wall_time
            .with_timezone(&chrono::Local)
            .format("%H:%M:%S")
            .to_string()
    }

    /// Renders by view, most first, as the web orders mutation sources.
    pub(crate) fn sources(&self) -> Vec<(&'static str, u32)> {
        let mut sources: Vec<_> = self
            .renders
            .iter()
            .map(|(name, count)| (*name, *count))
            .collect();
        sources.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        sources
    }

    pub(crate) fn json(&self) -> Value {
        let mut value = json!({
            "id": self.id,
            "kind": self.kind.label(),
            "wallTime": self.wall_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "target": self.target,
            "durationMs": self.duration_ms.round(),
            "inputToPaint": self.input_to_paint.iter()
                .map(|(kind, ms)| json!({ "type": kind, "ms": ms }))
                .collect::<Vec<_>>(),
            "frames": {
                "count": self.frames.total,
                "refreshMs": round(self.refresh_ms),
                "p50Ms": round(self.frames.percentile(0.5)),
                "p95Ms": round(self.frames.percentile(0.95)),
                "maxMs": round(self.max_frame_ms()),
                "droppedFrames": self.dropped_frames(),
            },
            "dom": {
                "added": 0,
                "removed": 0,
                "attributes": 0,
                "text": 0,
                "sources": self.sources().into_iter()
                    .map(|(source, mutations)| json!({ "source": source, "mutations": mutations }))
                    .collect::<Vec<_>>(),
            },
        });
        if let Some(commands) = &self.commands {
            value["commands"] = Value::Array(commands.clone());
        }
        value
    }
}

#[derive(Default)]
pub(crate) struct Recorder {
    recording: bool,
    /// Bumped on every start, so a stopped session's frame chain ends.
    session: u64,
    recorded: VecDeque<Interaction>,
    current: Option<Interaction>,
    /// Input-to-paint measurements waiting for a drawn frame: entry id, input type, dispatch time.
    pending_paints: Vec<(u64, &'static str, Instant)>,
    /// The median of all frame deltas is the refresh interval.
    session_frames: Histogram,
    last_frame: Option<Instant>,
    next_id: u64,
    pointer_down: bool,
}

impl Recorder {
    pub(crate) fn recording(&self) -> bool {
        self.recording
    }

    pub(crate) fn session(&self) -> u64 {
        self.session
    }

    pub(crate) fn interactions(&self) -> impl Iterator<Item = &Interaction> {
        self.recorded.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.recorded.len()
    }

    pub(crate) fn start(&mut self) {
        if self.recording {
            return;
        }
        self.recording = true;
        self.session += 1;
        self.last_frame = None;
        // Ids start at 1, as on the web.
        self.next_id = self.next_id.max(1);
    }

    pub(crate) fn stop(&mut self, timings: Option<&Timings>) {
        if !self.recording {
            return;
        }
        self.flush(timings);
        self.recording = false;
        self.pointer_down = false;
        self.last_frame = None;
    }

    pub(crate) fn clear(&mut self) {
        self.recorded.clear();
        self.current = None;
        self.pending_paints.clear();
        self.session_frames = Histogram::default();
    }

    /// `timings` is the diagnostics trace while diagnostics is enabled.
    fn flush(&mut self, timings: Option<&Timings>) {
        let Some(mut interaction) = self.current.take() else {
            return;
        };
        let ended = Instant::now();
        interaction.duration_ms = (ended - interaction.started).as_secs_f64() * 1000.;
        interaction.refresh_ms = self.session_frames.percentile(0.5);
        interaction.commands = timings.map(|timings| {
            timings
                .snapshot()
                .iter()
                .filter(|timing| {
                    timing.kind == TimingKind::Command
                        && timing.started >= interaction.started
                        && timing.started < ended
                })
                .map(|timing| {
                    json!({
                        "cmd": timing.name,
                        "ms": timing.duration_ms.round(),
                        "ok": timing.success,
                        "startOffsetMs": ((timing.started - interaction.started).as_secs_f64() * 1000.).round(),
                    })
                })
                .collect()
        });
        self.recorded.push_back(interaction);
        while self.recorded.len() > MAX_RECORDED {
            self.recorded.pop_front();
        }
    }

    /// Ends the current entry and starts one for an input dispatched at
    /// `started`. `measure` names the input type whose input-to-paint time the
    /// entry records.
    pub(crate) fn begin(
        &mut self,
        kind: Kind,
        target: String,
        measure: Option<&'static str>,
        started: Instant,
        timings: Option<&Timings>,
    ) {
        if !self.recording {
            return;
        }
        self.flush(timings);
        // A new input means earlier inputs drew nothing; they record no
        // input-to-paint time. Navigation starts while its input's frame draws.
        if kind != Kind::Navigate {
            self.pending_paints.clear();
        }
        let id = self.next_id;
        self.next_id += 1;
        self.current = Some(Interaction {
            id,
            kind,
            wall_time: chrono::Utc::now(),
            target,
            started,
            duration_ms: 0.,
            input_to_paint: Vec::new(),
            frames: Histogram::default(),
            refresh_ms: 0.,
            renders: BTreeMap::new(),
            commands: None,
        });
        if let Some(input) = measure {
            self.pending_paints.push((id, input, started));
        }
    }

    pub(crate) fn current_kind(&self) -> Option<Kind> {
        self.current.as_ref().map(|current| current.kind)
    }

    /// A pointer press starts a click; movement while pressed makes it a drag.
    pub(crate) fn pointer(&mut self, down: bool) {
        self.pointer_down = down;
    }

    pub(crate) fn pointer_moved(&mut self) {
        if self.pointer_down
            && let Some(current) = &mut self.current
            && current.kind == Kind::Click
        {
            current.kind = Kind::Drag;
        }
    }

    /// Another measured input type within the current entry, as the web's `click` after `pointerdown`.
    pub(crate) fn measure(&mut self, input: &'static str) {
        if let Some(current) = &self.current {
            self.pending_paints
                .push((current.id, input, Instant::now()));
        }
    }

    pub(crate) fn count_renders(&mut self, names: &[&'static str]) {
        if let Some(current) = &mut self.current {
            for name in names {
                *current.renders.entry(name).or_default() += 1;
            }
        }
    }

    pub(crate) fn awaiting_paint(&self) -> bool {
        !self.pending_paints.is_empty()
    }

    /// A frame finished painting: resolves every waiting input-to-paint time.
    pub(crate) fn painted(&mut self) {
        let now = Instant::now();
        for (id, input, dispatched) in std::mem::take(&mut self.pending_paints) {
            let ms = round((now - dispatched).as_secs_f64() * 1000.);
            let entry = self
                .current
                .iter_mut()
                .chain(self.recorded.iter_mut().rev())
                .find(|entry| entry.id == id);
            if let Some(entry) = entry {
                entry.input_to_paint.push((input, ms));
            }
        }
    }

    /// One display frame. `visible` is false while the window is inactive or
    /// hidden; the first frame after that spans the gap, not a frame, and is skipped.
    pub(crate) fn frame(&mut self, now: Instant, visible: bool) {
        if let Some(previous) = self.last_frame.replace(now)
            && visible
        {
            let delta = (now - previous).as_secs_f64() * 1000.;
            self.session_frames.add(delta);
            if let Some(current) = &mut self.current {
                current.frames.add(delta);
            }
        }
        if !visible {
            self.last_frame = None;
        }
    }

    /// JSON lines for `bun run perf:interactions`.
    pub(crate) fn json_lines(&self) -> String {
        self.recorded
            .iter()
            .map(|interaction| interaction.json().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
