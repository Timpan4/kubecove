//! Selected-resource event stream, owned by the detail panel.
use super::{
    Inspector,
    detail::Detail,
    events::EventsLoad,
    friendly_error::redact_detail,
    live_data::{COALESCE, RECOVERY, WatchInput, receive_or_tick},
};
use crate::backend::WatchTarget;
use gpui_kit::{Context, Window};
use kubecove_lib::models::{StreamMessage, StreamStatus};

impl Inspector {
    pub(super) fn watch_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let resource = detail.data.summary.clone();
        let uid = detail.data.metadata["uid"].as_str().map(str::to_owned);
        let backend = self.backend.clone();
        // The detail owns this watch, so it only writes into the detail that started it.
        detail.events.watch = Some(cx.spawn_in(window, async move |this, cx| {
            let mut catch_up = false;
            let mut timer = Box::pin(cx.background_executor().timer(RECOVERY));
            loop {
                let mut stream = match backend
                    .watch(WatchTarget::Events(Box::new(resource.clone())))
                    .await
                {
                    Ok(stream) => stream,
                    Err(error) => {
                        // Events may change while no watch runs, so the next
                        // Connected status lists them again.
                        catch_up = true;
                        let _ = this.update(cx, |this, cx| {
                            if let Detail::Ready(detail) = &mut this.detail {
                                detail.events.watch_error = Some(redact_detail(&error.message));
                            }
                            cx.notify();
                        });
                        timer.as_mut().await;
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                        let Ok(Some(number)) = this.update(cx, |this, _| {
                            this.events_state().map(|events| events.begin_read())
                        }) else {
                            return;
                        };
                        let result = backend.events(resource.clone(), uid.clone()).await;
                        if this
                            .update(cx, |this, cx| this.apply_events_read(number, result, cx))
                            .is_err()
                        {
                            return;
                        }
                        continue;
                    }
                };
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
                            let mut refresh = tick && catch_up;
                            let mut error = None;
                            let mut ended = closed;
                            for event in first.into_iter().chain(pending) {
                                match event {
                                    StreamMessage::ResourceEventsChanged { .. } => refresh = true,
                                    StreamMessage::Error { message, .. } => {
                                        error = Some(Some(redact_detail(&message)));
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
                            (stream, needs_catch_up, refresh, error, ended)
                        })
                        .await;
                    stream = resumed;
                    catch_up = caught_up;
                    let warning_changed = watch_error.is_some();
                    let mut number = 0;
                    let updated = this.update(cx, |this, cx| {
                        if let Detail::Ready(detail) = &mut this.detail {
                            refresh |= tick
                                && (matches!(detail.events.load, EventsLoad::Failed(_))
                                    || detail.events.refresh_error.is_some());
                            if let Some(error) = watch_error {
                                detail.events.watch_error = error;
                            }
                            if refresh {
                                number = detail.events.begin_read();
                            }
                        }
                        if refresh || warning_changed {
                            cx.notify();
                        }
                    });
                    if updated.is_err() {
                        return;
                    }
                    if refresh {
                        let started = std::time::Instant::now();
                        let result = backend.events(resource.clone(), uid.clone()).await;
                        let updated =
                            this.update(cx, |this, cx| this.apply_events_read(number, result, cx));
                        if updated.is_err() {
                            return;
                        }
                        cx.background_executor()
                            .timer(COALESCE.saturating_sub(started.elapsed()))
                            .await;
                    }
                    if ended {
                        catch_up = true;
                        break;
                    }
                }
                drop(stream);
                timer.as_mut().await;
                timer = Box::pin(cx.background_executor().timer(RECOVERY));
            }
        }));
    }
}
