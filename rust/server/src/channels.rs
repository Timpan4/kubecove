use axum::extract::ws::Utf8Bytes;
use kubecove_lib::models::{CHANNEL_MESSAGE_ENVELOPE_BYTES, MAX_WEBSOCKET_MESSAGE_BYTES};
use kubecove_lib::runtime::{Channel, ChannelClosed};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::Write as _,
    ops::Bound::{Excluded, Unbounded},
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

// Match src/features/resource-detail/log-helpers.ts's retained log line limit.
const RETAINED_MESSAGES: usize = 1_000;
// Retain one desktop WebSocket frame's payload capacity, excluding its wrapper.
const RETAINED_BYTES: usize = MAX_WEBSOCKET_MESSAGE_BYTES - CHANNEL_MESSAGE_ENVELOPE_BYTES;

struct PendingMessage {
    frame: Utf8Bytes,
    payload_bytes: usize,
    // Coalesce oversized drops at their position without allocating queue entries.
    dropped_after: usize,
}

fn encode_message<T: Serialize>(
    id: u64,
    message: &T,
) -> Result<Option<PendingMessage>, ChannelClosed> {
    // Keep the payload's initial serde_json::to_vec capacity plus room for the envelope.
    let mut frame = Vec::with_capacity(128 + CHANNEL_MESSAGE_ENVELOPE_BYTES);
    write!(frame, "{{\"channelId\":{id},\"message\":").map_err(|_| ChannelClosed)?;
    let payload_start = frame.len();
    serde_json::to_writer(&mut frame, message).map_err(|_| ChannelClosed)?;
    let payload_bytes = frame.len() - payload_start;
    if payload_bytes > RETAINED_BYTES {
        return Ok(None);
    }
    frame.extend_from_slice(b",\"type\":\"channel\"}");
    Ok(Some(PendingMessage {
        frame: Utf8Bytes::try_from(frame).map_err(|_| ChannelClosed)?,
        payload_bytes,
        dropped_after: 0,
    }))
}

#[derive(Default)]
struct PendingChannel {
    messages: VecDeque<PendingMessage>,
    bytes: usize,
    dropped: usize,
}

impl PendingChannel {
    fn has_output(&self) -> bool {
        self.dropped != 0 || !self.messages.is_empty()
    }
}

#[cfg(test)]
#[path = "channels_tests.rs"]
mod tests;

#[derive(Default)]
struct Pending {
    channels: BTreeMap<u64, PendingChannel>,
    ready: BTreeSet<u64>,
    last_channel: Option<u64>,
    closed: bool,
}

#[derive(Default)]
pub struct ChannelHub {
    pending: Mutex<Pending>,
    pub ready: Notify,
}

impl ChannelHub {
    pub fn channel<T: Serialize + Send + 'static>(self: &Arc<Self>, id: u64) -> Channel<T> {
        self.pending
            .lock()
            .expect("channel hub lock")
            .channels
            .entry(id)
            .or_default();
        let hub = Arc::clone(self);
        Channel::new(move |message| {
            let message = encode_message(id, &message)?;
            let mut pending = hub.pending.lock().map_err(|_| ChannelClosed)?;
            if pending.closed {
                return Err(ChannelClosed);
            }
            let channel = pending.channels.get_mut(&id).ok_or(ChannelClosed)?;
            let was_idle = !channel.has_output();
            if let Some(message) = message {
                while channel.messages.len() == RETAINED_MESSAGES
                    || channel.bytes + message.payload_bytes > RETAINED_BYTES
                {
                    let evicted = channel.messages.pop_front().expect("retained message");
                    channel.bytes -= evicted.payload_bytes;
                    channel.dropped = channel
                        .dropped
                        .saturating_add(1)
                        .saturating_add(evicted.dropped_after);
                }
                channel.bytes += message.payload_bytes;
                channel.messages.push_back(message);
            } else if let Some(last) = channel.messages.back_mut() {
                last.dropped_after = last.dropped_after.saturating_add(1);
            } else {
                channel.dropped = channel.dropped.saturating_add(1);
            }
            if was_idle {
                pending.ready.insert(id);
            }
            drop(pending);
            hub.ready.notify_one();
            Ok(())
        })
    }

    pub fn pop(&self) -> Option<Utf8Bytes> {
        let mut pending = self.pending.lock().expect("channel hub lock");
        let next = pending
            .last_channel
            .and_then(|last| pending.ready.range((Excluded(last), Unbounded)).next());
        let id = *next.or_else(|| pending.ready.first())?;
        pending.last_channel = Some(id);
        let channel = pending
            .channels
            .get_mut(&id)
            .expect("selected pending channel");
        let dropped = std::mem::take(&mut channel.dropped);
        let frame = if dropped == 0 {
            let message = channel.messages.pop_front().expect("ready channel message");
            channel.bytes -= message.payload_bytes;
            channel.dropped = message.dropped_after;
            Some(message.frame)
        } else {
            None
        };
        if !channel.has_output() {
            pending.ready.remove(&id);
        }
        drop(pending);
        Some(frame.unwrap_or_else(|| {
            format!("{{\"channelId\":{id},\"dropped\":{dropped},\"type\":\"channel_gap\"}}").into()
        }))
    }

    pub fn remove(&self, id: u64) {
        let mut pending = self.pending.lock().expect("channel hub lock");
        pending.channels.remove(&id);
        pending.ready.remove(&id);
    }

    pub fn close(&self) {
        let mut pending = self.pending.lock().expect("channel hub lock");
        pending.closed = true;
        pending.channels.clear();
        pending.ready.clear();
        drop(pending);
        self.ready.notify_waiters();
    }
}
