use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use tauri::async_runtime::JoinHandle;

#[derive(Default)]
pub struct StreamRegistry {
    next_id: AtomicU64,
    handles: Mutex<HashMap<String, Vec<JoinHandle<()>>>>,
}

impl StreamRegistry {
    pub(super) fn stream_id(&self, prefix: &str) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{id}")
    }

    pub(super) fn insert(&self, stream_id: String, handles: Vec<JoinHandle<()>>) {
        self.handles
            .lock()
            .expect("stream registry lock")
            .insert(stream_id, handles);
    }

    pub(super) fn stop(&self, stream_id: &str) -> bool {
        let handles = self
            .handles
            .lock()
            .expect("stream registry lock")
            .remove(stream_id);
        if let Some(handles) = handles {
            for handle in handles {
                handle.abort();
            }
            true
        } else {
            false
        }
    }
}
