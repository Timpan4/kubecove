//! Native command round trips. Payloads never enter the trace.
use crate::backend::Read;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

// Match MAX_FRONTEND_EVENTS in src/lib/diagnostics.ts.
const MAX_EVENTS: usize = 500;
#[derive(Clone, Debug, serde::Serialize)]
pub struct Timing {
    pub name: &'static str,
    pub duration_ms: f64,
    pub success: bool,
}
#[derive(Default)]
struct State {
    enabled: bool,
    epoch: u64,
    events: VecDeque<Timing>,
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
                state.events.push_back(Timing {
                    name,
                    duration_ms: started.elapsed().as_secs_f64() * 1000.,
                    success: result.is_ok(),
                });
                while state.events.len() > MAX_EVENTS {
                    state.events.pop_front();
                }
            }
            result
        })
    }
}
