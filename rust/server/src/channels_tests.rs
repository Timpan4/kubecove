use super::*;
use serde_json::{Value, json};

fn pop(hub: &ChannelHub) -> Value {
    serde_json::from_str(&hub.pop().unwrap()).unwrap()
}

// Failure modes: byte eviction must report loss before replay; JSON escaping must
// count toward the budget; draining must release capacity; oversized frames must
// not close the producer or move a gap before older retained messages; evicting
// gap markers must preserve the loss count and both queue bounds.
#[test]
fn payload_budget_includes_the_largest_channel_envelope() {
    let hub = Arc::new(ChannelHub::default());
    let channel = hub.channel::<String>(u64::MAX);
    // JSON string quotes consume two bytes of the payload capacity.
    let message = "x".repeat(RETAINED_BYTES - 2);
    channel.send(message.clone()).unwrap();
    let frame = hub.pop().unwrap();
    assert_eq!(frame.len(), MAX_WEBSOCKET_MESSAGE_BYTES);
    assert_eq!(
        serde_json::from_str::<Value>(&frame).unwrap()["message"],
        message
    );
}

#[test]
fn serialized_byte_budget_reports_loss_and_releases_drained_capacity() {
    let hub = Arc::new(ChannelHub::default());
    let channel = hub.channel::<String>(1);
    let message = "\0".repeat(256 * 1024 / 6);
    let bytes = serde_json::to_string(&message).unwrap().len();
    let retained = RETAINED_BYTES / bytes;
    for _ in 0..=retained {
        channel.send(message.clone()).unwrap();
    }
    assert_eq!(
        pop(&hub),
        json!({"type":"channel_gap","channelId":1,"dropped":1})
    );
    assert_eq!(pop(&hub)["message"], message);
    channel.send(message.clone()).unwrap();
    for _ in 0..retained {
        assert_eq!(pop(&hub)["message"], message);
    }
    assert!(hub.pop().is_none());
}

#[test]
fn oversized_message_reports_a_gap_without_closing_the_channel() {
    let hub = Arc::new(ChannelHub::default());
    let channel = hub.channel::<String>(1);
    channel.send("retained".into()).unwrap();
    channel.send("also retained".into()).unwrap();
    channel.send("x".repeat(RETAINED_BYTES)).unwrap();
    channel.send("later".into()).unwrap();
    assert_eq!(pop(&hub)["message"], "retained");
    assert_eq!(pop(&hub)["message"], "also retained");
    assert_eq!(
        pop(&hub),
        json!({"type":"channel_gap","channelId":1,"dropped":1})
    );
    assert_eq!(pop(&hub)["message"], "later");
    assert!(hub.pop().is_none());
}

#[test]
fn evicting_queued_gaps_preserves_loss_counts_and_order() {
    let hub = Arc::new(ChannelHub::default());
    let channel = hub.channel::<String>(1);
    channel.send("oldest".into()).unwrap();
    let oversized = "x".repeat(RETAINED_BYTES);
    channel.send(oversized.clone()).unwrap();
    channel.send(oversized).unwrap();
    for n in 0..RETAINED_MESSAGES {
        channel.send(n.to_string()).unwrap();
    }
    assert_eq!(
        pop(&hub),
        json!({"type":"channel_gap","channelId":1,"dropped":3})
    );
    for n in 0..RETAINED_MESSAGES {
        assert_eq!(pop(&hub)["message"], n.to_string());
    }
    assert!(hub.pop().is_none());
}

#[test]
fn a_replenished_channel_cannot_starve_another_channel() {
    let hub = Arc::new(ChannelHub::default());
    let first = hub.channel::<u8>(1);
    let second = hub.channel::<u8>(2);
    first.send(1).unwrap();
    second.send(2).unwrap();
    assert_eq!(pop(&hub)["channelId"], 1);
    first.send(3).unwrap();
    assert_eq!(pop(&hub)["channelId"], 2);
    assert_eq!(pop(&hub)["message"], 3);
}

#[test]
fn ready_channels_wrap_by_id_and_keep_the_exact_text_envelope() {
    let hub = Arc::new(ChannelHub::default());
    let low = hub.channel::<&str>(10);
    let _idle = hub.channel::<&str>(20);
    let middle = hub.channel::<&str>(30);
    let high = hub.channel::<&str>(40);
    middle.send("first").unwrap();
    assert_eq!(pop(&hub)["channelId"], 30);
    low.send("last\n").unwrap();
    high.send("removed").unwrap();
    hub.remove(40);
    let high = hub.channel::<&str>(40);
    high.send("next").unwrap();
    assert_eq!(pop(&hub)["message"], "next");
    assert_eq!(
        hub.pop().unwrap().as_str(),
        r#"{"channelId":10,"message":"last\n","type":"channel"}"#
    );
    assert!(hub.pop().is_none());
}

#[test]
#[ignore = "manual timing comparison in release mode"]
fn measure_sparse_channels_with_batched_logs() {
    use kubecove_lib::models::StreamMessage;
    use std::{hint::black_box, time::Instant};

    let hub = Arc::new(ChannelHub::default());
    let idle_count = u64::try_from(RETAINED_MESSAGES).unwrap();
    for id in 0..idle_count {
        drop(hub.channel::<StreamMessage>(id));
    }
    let active = [idle_count, idle_count + 1].map(|id| hub.channel::<StreamMessage>(id));
    let batch = StreamMessage::LogLines {
        stream_id: "logs".into(),
        lines: vec!["pod initialized\tcontainer started".into(); RETAINED_MESSAGES],
        source: None,
    };
    let started = Instant::now();
    for _ in 0..RETAINED_MESSAGES {
        for channel in &active {
            channel.send(batch.clone()).unwrap();
        }
        for _ in &active {
            black_box(hub.pop().unwrap());
        }
        assert!(hub.pop().is_none());
    }
    eprintln!(
        "encoded and drained {} log batches with {idle_count} idle channels in {:?}",
        RETAINED_MESSAGES * active.len(),
        started.elapsed()
    );
}

#[test]
fn retention_reports_a_gap_and_revocation_rejects_late_output() {
    let hub = Arc::new(ChannelHub::default());
    let channel = hub.channel::<usize>(1);
    for message in 0..=RETAINED_MESSAGES {
        channel.send(message).unwrap();
    }
    let gap = pop(&hub);
    assert_eq!(gap["type"], "channel_gap");
    assert_eq!(gap["dropped"], 1);
    assert_eq!(pop(&hub)["message"], 1);
    hub.remove(1);
    assert!(channel.send(0).is_err());
    assert!(hub.pop().is_none());
    let channel = hub.channel::<u8>(2);
    hub.close();
    assert!(channel.send(0).is_err());
}
