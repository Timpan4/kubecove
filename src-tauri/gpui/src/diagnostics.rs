//! Native command round trips, view presentation spans and render counters.
//! Payloads never enter the trace.
use crate::backend::Read;
use crate::interactions::CommandCapture;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

// Match MAX_FRONTEND_EVENTS in src/lib/diagnostics.ts.
pub(crate) const MAX_EVENTS: usize = 500;

#[derive(Clone, Debug)]
pub struct Timing {
    pub name: &'static str,
    pub duration_ms: f64,
    pub success: bool,
}
#[derive(Default)]
struct State {
    enabled: bool,
    // Latency invalidates in-flight timings when cleared or preferences are reapplied.
    latency_epoch: u64,
    // A pending recording stays eligible until diagnostics is toggled.
    recording_epoch: u64,
    events: VecDeque<Timing>,
    interaction_commands: Weak<CommandCapture>,
    /// Renders per name, as the web's `*.render` diagnostic counters.
    counters: BTreeMap<&'static str, u64>,
}
#[derive(Clone, Default)]
pub struct Timings(Arc<Mutex<State>>);
impl Timings {
    pub fn set_enabled(&self, enabled: bool) {
        let mut state = self.0.lock().expect("timing state");
        if state.enabled != enabled {
            state.recording_epoch = state.recording_epoch.wrapping_add(1);
        }
        state.enabled = enabled;
        state.latency_epoch = state.latency_epoch.wrapping_add(1);
    }
    pub fn clear(&self) {
        let mut state = self.0.lock().expect("timing state");
        state.events.clear();
        state.counters.clear();
        state.latency_epoch = state.latency_epoch.wrapping_add(1);
    }
    pub(crate) fn capture_interaction_commands(&self, capture: &Arc<CommandCapture>) {
        self.0.lock().expect("timing state").interaction_commands = Arc::downgrade(capture);
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
        let latency_epoch = state.latency_epoch;
        let recording_epoch = state.recording_epoch;
        let capture = state.interaction_commands.upgrade();
        drop(state);
        let (started, owner) = match capture {
            Some(capture) => capture.start_command(),
            None => (Instant::now(), Weak::new()),
        };
        let collector = self.clone();
        Box::pin(async move {
            let result = read.await;
            let success = result.is_ok();
            let duration_ms = started.elapsed().as_secs_f64() * 1000.;
            let record = {
                let mut state = collector.0.lock().expect("timing state");
                if state.enabled && state.latency_epoch == latency_epoch {
                    push(
                        &mut state,
                        Timing {
                            name,
                            duration_ms,
                            success,
                        },
                    );
                }
                state.enabled && state.recording_epoch == recording_epoch
            };
            if record && let Some(owner) = owner.upgrade() {
                owner.complete(name, started, duration_ms, success);
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
