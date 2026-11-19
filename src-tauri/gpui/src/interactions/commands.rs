use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex, time::Instant};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordedCommand {
    cmd: &'static str,
    ms: f64,
    ok: bool,
    start_offset_ms: f64,
}

struct Window {
    started: Instant,
    ended: Option<Instant>,
    commands: Option<Vec<RecordedCommand>>,
}

/// Retained interaction windows, shared with the backend tasks that complete commands.
#[derive(Default)]
pub(crate) struct CommandCapture(Mutex<VecDeque<Window>>);

impl CommandCapture {
    pub(super) fn transition(
        &self,
        next: Option<Instant>,
        include_commands: bool,
        evict_oldest: bool,
    ) -> Instant {
        let mut windows = self.0.lock().expect("interaction commands");
        let ended = Instant::now();
        if let Some(window) = windows.back_mut().filter(|window| window.ended.is_none()) {
            window.ended = Some(ended);
            if !include_commands {
                window.commands = None;
            }
        }
        if evict_oldest {
            windows.pop_front();
        }
        if let Some(started) = next {
            windows.push_back(Window {
                started,
                ended: None,
                commands: Some(Vec::new()),
            });
        }
        ended
    }

    pub(super) fn clear(&self) {
        self.0.lock().expect("interaction commands").clear();
    }

    pub(super) fn snapshot(&self) -> Vec<Option<Vec<RecordedCommand>>> {
        self.0
            .lock()
            .expect("interaction commands")
            .iter()
            .take_while(|window| window.ended.is_some())
            .map(|window| window.commands.clone())
            .collect()
    }

    pub(crate) fn complete(&self, cmd: &'static str, started: Instant, duration_ms: f64, ok: bool) {
        let mut windows = self.0.lock().expect("interaction commands");
        let Some(window) = windows
            .iter_mut()
            .rev()
            .find(|window| window.started <= started)
        else {
            return;
        };
        if window.ended.is_some_and(|ended| started >= ended) {
            return;
        }
        if let Some(commands) = &mut window.commands {
            commands.push(RecordedCommand {
                cmd,
                ms: duration_ms.round(),
                ok,
                start_offset_ms: (started.duration_since(window.started).as_secs_f64() * 1000.)
                    .round(),
            });
        }
    }
}
