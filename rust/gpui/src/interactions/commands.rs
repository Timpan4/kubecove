use serde::Serialize;
use std::{
    mem,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordedCommand {
    cmd: &'static str,
    ms: f64,
    ok: bool,
    start_offset_ms: f64,
}

pub(crate) struct PendingCommands {
    interaction_started: Instant,
    commands: Mutex<Vec<RecordedCommand>>,
}

impl PendingCommands {
    pub(super) fn new(interaction_started: Instant) -> Self {
        Self {
            interaction_started,
            commands: Mutex::default(),
        }
    }

    pub(super) fn take_completed(&self) -> Vec<RecordedCommand> {
        mem::take(&mut *self.commands.lock().expect("interaction commands"))
    }

    pub(crate) fn complete(&self, cmd: &'static str, started: Instant, duration_ms: f64, ok: bool) {
        self.commands
            .lock()
            .expect("interaction commands")
            .push(RecordedCommand {
                cmd,
                ms: duration_ms.round(),
                ok,
                start_offset_ms: (started
                    .duration_since(self.interaction_started)
                    .as_secs_f64()
                    * 1000.)
                    .round(),
            });
    }
}

/// A command takes its owner at dispatch. Only the recorder keeps the entry alive.
#[derive(Default)]
pub(crate) struct CommandCapture(Mutex<Weak<PendingCommands>>);

impl CommandCapture {
    pub(super) fn set_current(&self, next: Option<&Arc<PendingCommands>>) -> Instant {
        let next = next.map_or_else(Weak::new, Arc::downgrade);
        let mut current = self.0.lock().expect("current interaction");
        let switched_at = Instant::now();
        *current = next;
        switched_at
    }

    pub(crate) fn start_command(&self) -> (Instant, Weak<PendingCommands>) {
        let current = self.0.lock().expect("current interaction");
        (Instant::now(), current.clone())
    }
}
