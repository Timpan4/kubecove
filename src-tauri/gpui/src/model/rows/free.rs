//! Frees replaced rows on a background thread. A namespace switch or watch
//! rebuild drops the previous snapshot on the UI thread, and freeing every
//! string of 10k rows there stalls that frame for milliseconds.

use super::PreparedRows;
use kubecove_lib::models::ResourceSummary;
use std::sync::{OnceLock, mpsc::Sender};

impl Drop for PreparedRows {
    fn drop(&mut self) {
        free_later(std::mem::take(&mut self.rows));
    }
}

/// Sends `rows` to the freeing thread. When that thread cannot start, or has
/// stopped, `rows` is freed here.
fn free_later(rows: Vec<ResourceSummary>) {
    static FREE: OnceLock<Option<Sender<Vec<ResourceSummary>>>> = OnceLock::new();
    if rows.is_empty() {
        return;
    }
    let sender = FREE.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel::<Vec<ResourceSummary>>();
        std::thread::Builder::new()
            .name("kubecove-rows-free".into())
            .spawn(move || receiver.into_iter().for_each(drop))
            .ok()?;
        Some(sender)
    });
    if let Some(sender) = sender {
        // A failed send hands `rows` back inside the error, which drops here.
        let _ = sender.send(rows);
    }
}
