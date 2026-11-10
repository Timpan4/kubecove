//! Native command round trips, view presentation spans and render counters.
//! Payloads never enter the trace.
use crate::backend::Read;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};

// Match MAX_FRONTEND_EVENTS in src/lib/diagnostics.ts.
const MAX_EVENTS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingKind {
    /// A backend command's round trip, from the UI's request to its result.
    Command,
    /// A view's request to the end of painting the first frame that draws it.
    Presentation,
}

#[derive(Clone, Debug)]
pub struct Timing {
    pub name: &'static str,
    pub kind: TimingKind,
    pub started: Instant,
    pub duration_ms: f64,
    pub success: bool,
}
#[derive(Default)]
struct State {
    enabled: bool,
    epoch: u64,
    events: VecDeque<Timing>,
    /// Renders per name, as the web's `*.render` diagnostic counters.
    counters: BTreeMap<&'static str, u64>,
}
#[derive(Clone, Default)]
pub struct Timings(Arc<Mutex<State>>);
impl Timings {
    pub fn set_enabled(&self, enabled: bool) {
        let mut state = self.0.lock().expect("timing state");
        state.enabled = enabled;
        state.epoch = state.epoch.wrapping_add(1);
    }
    pub fn clear(&self) {
        let mut state = self.0.lock().expect("timing state");
        state.events.clear();
        state.counters.clear();
        state.epoch = state.epoch.wrapping_add(1);
    }
    pub fn snapshot(&self) -> Vec<Timing> {
        self.0
            .lock()
            .expect("timing state")
            .events
            .iter()
            .cloned()
            .collect()
    }
    pub fn counters(&self) -> BTreeMap<&'static str, u64> {
        self.0.lock().expect("timing state").counters.clone()
    }
    /// Adds one render to each name while diagnostics is enabled.
    pub fn count_renders(&self, names: &[&'static str]) {
        let mut state = self.0.lock().expect("timing state");
        if !state.enabled {
            return;
        }
        for name in names {
            *state.counters.entry(name).or_default() += 1;
        }
    }
    /// Records a presentation span that ended now.
    pub fn record_presentation(&self, name: &'static str, started: Instant) {
        let duration_ms = started.elapsed().as_secs_f64() * 1000.;
        let mut state = self.0.lock().expect("timing state");
        if state.enabled {
            push(
                &mut state,
                Timing {
                    name,
                    kind: TimingKind::Presentation,
                    started,
                    duration_ms,
                    success: true,
                },
            );
        }
    }
    pub fn measure<T: Send + 'static>(&self, name: &'static str, read: Read<T>) -> Read<T> {
        let state = self.0.lock().expect("timing state");
        if !state.enabled {
            return read;
        }
        let epoch = state.epoch;
        drop(state);
        let started = Instant::now();
        let collector = self.clone();
        Box::pin(async move {
            let result = read.await;
            let mut state = collector.0.lock().expect("timing state");
            if state.enabled && state.epoch == epoch {
                push(
                    &mut state,
                    Timing {
                        name,
                        kind: TimingKind::Command,
                        started,
                        duration_ms: started.elapsed().as_secs_f64() * 1000.,
                        success: result.is_ok(),
                    },
                );
            }
            result
        })
    }
}

fn push(state: &mut State, timing: Timing) {
    state.events.push_back(timing);
    while state.events.len() > MAX_EVENTS {
        state.events.pop_front();
    }
}
