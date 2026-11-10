//! Tells local views that a remote session changed state, so they read its
//! status again without waiting for a failed call (ADR 0031).
//!
//! Signal after the state is stored. A woken reader then sees the new state.

use std::sync::OnceLock;
use tokio::sync::watch;

fn changes() -> &'static watch::Sender<u64> {
    static CHANGES: OnceLock<watch::Sender<u64>> = OnceLock::new();
    CHANGES.get_or_init(|| watch::channel(0).0)
}

/// Marks one session state change. Each subscriber's `changed` resolves once
/// per call.
pub fn signal_session_changed() {
    changes().send_modify(|count| *count = count.wrapping_add(1));
}

/// Subscribes to session state changes. The receiver starts at the current
/// value, so only later changes wake it.
pub fn subscribe_session_changes() -> watch::Receiver<u64> {
    changes().subscribe()
}
