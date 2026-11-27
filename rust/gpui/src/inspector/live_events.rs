//! Selected-resource event stream, owned by the detail panel.
use super::{
    Inspector,
    detail::Detail,
    events::EventsLoad,
    live_data::{RECOVERY, WatchInput, receive_or_tick},
};
use crate::backend::WatchTarget;
use futures_util::future::{Either, select};
use gpui_kit::{Context, Window};
use kubecove_lib::models::{StreamMessage, StreamStatus, redact_sensitive_detail};
use tokio::sync::mpsc;

impl Inspector {
    pub(super) fn watch_events(
        &mut self,
        request_refresh: mpsc::Sender<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let resource = detail.data.summary.clone();
        let backend = self.backend.clone();
        // The detail owns this watch, so it only writes into the detail that started it.
        detail.events.watch = Some(cx.spawn_in(window, async move |this, cx| {
            let mut timer = Box::pin(cx.background_executor().timer(RECOVERY));
            loop {
                let mut opening = backend.watch(WatchTarget::Events(Box::new(resource.clone())));
                let opened = loop {
                    match select(opening.as_mut(), timer.as_mut()).await {
                        Either::Left((result, _)) => break result,
                        Either::Right(((), _)) => {
                            timer = Box::pin(cx.background_executor().timer(RECOVERY));
                            let _ = request_refresh.try_send(());
                        }
                    }
                };
                let mut stream = match opened {
                    Ok(stream) => stream,
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| {
                            if let Detail::Ready(detail) = &mut this.detail {
                                detail.events.watch_error =
                                    Some(redact_sensitive_detail(&error.message));
                            }
                            cx.notify();
                        });
                        timer.as_mut().await;
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                        let _ = request_refresh.try_send(());
                        continue;
                    }
                };
                // A new watch can replay current events, but not changes that
                // removed a row from the list taken while it was opening.
                let mut catch_up = true;
                loop {
                    let input = receive_or_tick(&mut stream, &mut timer).await;
                    let tick = matches!(input, WatchInput::Tick);
                    let closed = matches!(input, WatchInput::Closed);
                    if tick {
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                    }
                    let first = match input {
                        WatchInput::Message(message) => Some(message),
                        WatchInput::Closed | WatchInput::Tick => None,
                    };
                    // Replays can leave a backlog while the list is pending.
                    // Fold the current batch off the UI executor in stream order.
                    let (resumed, caught_up, mut refresh, watch_error, ended) = cx
                        .background_executor()
                        .spawn(async move {
                            let queued = stream.messages.len();
                            let pending =
                                std::iter::from_fn(|| stream.messages.try_recv().ok()).take(queued);
                            let mut needs_catch_up = catch_up;
                            let mut refresh = tick && needs_catch_up;
                            let mut error = None;
                            let mut ended = closed;
                            for event in first.into_iter().chain(pending) {
                                match event {
                                    StreamMessage::ResourceEventsChanged { .. } => refresh = true,
                                    StreamMessage::Error { message, .. } => {
                                        error = Some(Some(message));
                                        needs_catch_up = true;
                                    }
                                    StreamMessage::Status {
                                        status: StreamStatus::Connected,
                                        ..
                                    } => {
                                        error = Some(None);
                                        refresh |= needs_catch_up;
                                        needs_catch_up = false;
                                    }
                                    StreamMessage::Status { .. } => needs_catch_up = true,
                                    StreamMessage::Stopped { .. } => {
                                        ended = true;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            let error = error
                                .map(|item| item.map(|message| redact_sensitive_detail(&message)));
                            (stream, needs_catch_up, refresh, error, ended)
                        })
                        .await;
                    stream = resumed;
                    catch_up = caught_up;
                    let warning_changed = watch_error.is_some();
                    let updated = this.update(cx, |this, cx| {
                        if let Detail::Ready(detail) = &mut this.detail {
                            refresh |= tick
                                && (matches!(detail.events.load, EventsLoad::Failed(_))
                                    || detail.events.refresh_error.is_some());
                            if let Some(error) = watch_error {
                                detail.events.watch_error = error;
                            }
                        }
                        if warning_changed {
                            cx.notify();
                        }
                    });
                    if updated.is_err() {
                        return;
                    }
                    if refresh {
                        let _ = request_refresh.try_send(());
                    }
                    if ended {
                        break;
                    }
                }
                drop(stream);
                timer.as_mut().await;
                timer = Box::pin(cx.background_executor().timer(RECOVERY));
                let _ = request_refresh.try_send(());
            }
        }));
    }
}
