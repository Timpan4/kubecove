use super::logs::read_log_line;
use crate::models::{
    LogLineSource, StreamMessage, CHANNEL_MESSAGE_ENVELOPE_BYTES, MAX_WEBSOCKET_MESSAGE_BYTES,
};
use futures_util::io::AsyncBufRead;
use std::{future::poll_fn, future::Future, io, task::Poll};

// JSON string bytes expand at most sixfold (\u00XX); reserve the channel wrapper.
const MAX_BATCH_BYTES: usize = (MAX_WEBSOCKET_MESSAGE_BYTES - CHANNEL_MESSAGE_ENVELOPE_BYTES) / 6;

pub(super) fn emit_log_batch(
    stream_id: &str,
    source: Option<&LogLineSource>,
    batch_lines: bool,
    lines: Vec<String>,
    mut emit: impl FnMut(StreamMessage) -> bool,
) -> bool {
    if batch_lines {
        emit(StreamMessage::LogLines {
            stream_id: stream_id.to_owned(),
            lines,
            source: source.cloned(),
        })
    } else {
        lines.into_iter().all(|line| {
            emit(StreamMessage::LogLine {
                stream_id: stream_id.to_owned(),
                line,
                source: source.cloned(),
            })
        })
    }
}

pub(super) async fn read_log_batches(
    reader: &mut (impl AsyncBufRead + Unpin),
    stream_id: &str,
    source: Option<&LogLineSource>,
    mut emit: impl FnMut(Vec<String>) -> bool,
) -> io::Result<()> {
    let envelope = serde_json::to_vec(&StreamMessage::LogLines {
        stream_id: stream_id.to_owned(),
        lines: Vec::new(),
        source: source.cloned(),
    })?;
    let budget = MAX_BATCH_BYTES
        .checked_sub(envelope.len().div_ceil(6))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "log envelope exceeds frame"))?;
    let mut lines = Vec::new();
    let mut bytes = 0;
    loop {
        // Keep partial-line state alive when a pending read flushes ready lines.
        let read = read_log_line(reader);
        tokio::pin!(read);
        let result = poll_fn(|cx| {
            let result = read.as_mut().poll(cx);
            if result.is_pending() && !lines.is_empty() {
                bytes = 0;
                if !emit(std::mem::take(&mut lines)) {
                    return Poll::Ready(None);
                }
            }
            result.map(Some)
        })
        .await;
        let Some(result) = result else {
            return Ok(());
        };
        match result {
            Ok(Some(line)) => {
                // One six-byte unit covers each line's quotes and array separator.
                let line_bytes = line.len() + 1;
                if !lines.is_empty() && bytes + line_bytes > budget {
                    if !emit(std::mem::take(&mut lines)) {
                        return Ok(());
                    }
                    bytes = 0;
                    tokio::task::yield_now().await;
                }
                if line_bytes > budget {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "log line and envelope exceed frame",
                    ));
                }
                bytes += line_bytes;
                lines.push(line);
            }
            end => {
                if !lines.is_empty() && !emit(lines) {
                    return Ok(());
                }
                return end.map(|_| ());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AggregatedLogStreamRequest, PodLogStreamRequest};
    use futures_util::{io::BufReader, stream, StreamExt, TryStreamExt};
    use std::{cell::RefCell, task::Context};

    // Failure modes: omitted/false opt-in breaks old clients, aggregate requests
    // lose the flag, or splitting batches loses ordering, empty lines, or source.
    #[tokio::test]
    async fn requests_only_emit_batches_when_explicitly_enabled() {
        for flag in [None, Some(false), Some(true)] {
            let mut request = serde_json::json!({
                "clusterContext": "dev", "namespace": "default", "podName": "api",
                "targetKind": "Deployment", "targetName": "api"
            });
            if let Some(flag) = flag {
                request["batchLines"] = flag.into();
            }
            let pod: PodLogStreamRequest = serde_json::from_value(request.clone()).unwrap();
            let aggregate: AggregatedLogStreamRequest = serde_json::from_value(request).unwrap();
            for batch_lines in [pod.batch_lines, aggregate.batch_lines] {
                assert_eq!(batch_lines, flag.unwrap_or(false));
                let source = LogLineSource {
                    pod_name: "api".into(),
                    container: Some("app".into()),
                };
                let mut reader = BufReader::new(&b"first\n\nlast\n"[..]);
                let mut messages = Vec::new();
                read_log_batches(&mut reader, "logs", Some(&source), |lines| {
                    emit_log_batch("logs", Some(&source), batch_lines, lines, |message| {
                        messages.push(serde_json::to_value(message).unwrap());
                        true
                    })
                })
                .await
                .unwrap();
                let source = serde_json::to_value(source).unwrap();
                let expected = if batch_lines {
                    vec![serde_json::json!({
                        "type": "logLines", "streamId": "logs",
                        "lines": ["first", "", "last"], "source": source
                    })]
                } else {
                    ["first", "", "last"].into_iter().map(|line| serde_json::json!({
                        "type": "logLine", "streamId": "logs", "line": line, "source": source
                    })).collect()
                };
                assert_eq!(messages, expected);
            }
        }
    }

    // Failure modes: ready lines wait or split by count, byte bounds omit JSON
    // escaping/envelopes, partial reads get lost, errors/EOF lose ready lines,
    // a closed consumer keeps reading, or an idle stream schedules wakeups.
    #[tokio::test]
    async fn drains_ready_lines_into_one_batch_before_waiting_and_preserves_partial_lines() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let source = stream::poll_fn(move |cx| receiver.poll_recv(cx));
        let mut reader = BufReader::new(source.into_async_read());
        let batches = RefCell::new(Vec::new());
        let future = read_log_batches(&mut reader, "logs", None, |lines| {
            batches.borrow_mut().push(lines);
            true
        });
        tokio::pin!(future);
        let mut cx = Context::from_waker(std::task::Waker::noop());
        assert!(future.as_mut().poll(&mut cx).is_pending());
        sender
            .send(Ok::<_, io::Error>(b"first\nsecond\npar".to_vec()))
            .unwrap();
        assert!(future.as_mut().poll(&mut cx).is_pending());
        assert_eq!(batches.take(), vec![vec!["first", "second"]]);
        sender.send(Ok(b"tial\nlast".to_vec())).unwrap();
        assert!(future.as_mut().poll(&mut cx).is_pending());
        assert_eq!(batches.take(), vec![vec!["partial"]]);
        drop(sender);
        assert!(future.as_mut().poll(&mut cx).is_ready());
        assert_eq!(batches.take(), vec![vec!["last"]]);
    }

    #[tokio::test]
    async fn byte_bound_splits_batches_and_fits_the_serialized_frame() {
        let line = "\0".repeat(super::super::logs::MAX_LOG_LINE_BYTES);
        let input = format!("{line}\n{line}\n");
        // Keep both complete lines ready in the same read buffer.
        let mut reader = BufReader::with_capacity(input.len(), input.as_bytes());
        let source = LogLineSource {
            pod_name: "pod".into(),
            container: Some("app".into()),
        };
        let mut batches = Vec::new();
        read_log_batches(&mut reader, "logs", Some(&source), |lines| {
            batches.push(lines);
            true
        })
        .await
        .unwrap();
        assert_eq!(batches, vec![vec![line.clone()], vec![line]]);
        for lines in batches {
            let message = StreamMessage::LogLines {
                stream_id: "logs".into(),
                lines,
                source: Some(source.clone()),
            };
            let frame =
                serde_json::json!({"channelId": u64::MAX, "message": message, "type": "channel"});
            assert!(serde_json::to_vec(&frame).unwrap().len() <= MAX_WEBSOCKET_MESSAGE_BYTES);
        }
    }

    #[tokio::test]
    async fn ready_lines_flush_before_error_and_consumer_cancellation_stops_reads() {
        let source = stream::iter(vec![
            Ok(b"first\nsecond\n".to_vec()),
            Err(io::Error::other("broken")),
        ]);
        let mut reader = BufReader::new(source.into_async_read());
        let mut batches = Vec::new();
        let error = read_log_batches(&mut reader, "logs", None, |lines| {
            batches.push(lines);
            true
        })
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "broken");
        assert_eq!(batches, vec![vec!["first", "second"]]);

        let source = stream::once(async { Ok::<_, io::Error>(b"first\n".to_vec()) })
            .chain(stream::pending());
        let mut reader = BufReader::new(Box::pin(source).into_async_read());
        read_log_batches(&mut reader, "logs", None, |_| false)
            .await
            .unwrap();
    }
}
