//! Selected-resource event stream, owned by the detail panel.
use super::{
    Inspector,
    detail::Detail,
    events::EventsLoad,
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
                                detail.events.watch_error = Some(error.message);
                            }
                            cx.notify();
                        });
                        timer.as_mut().await;
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                        continue;
                    }
                };
                loop {
                    let input = receive_or_tick(&mut stream, &mut timer).await;
                    let tick = matches!(input, WatchInput::Tick);
                    let mut ended = matches!(input, WatchInput::Closed);
                    if tick {
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                    }
                    let first = match input {
                        WatchInput::Message(message) => Some(message),
                        WatchInput::Closed | WatchInput::Tick => None,
                    };
                    let mut refresh = tick && catch_up;
                    let mut consumed = !tick;
                    let mut watch_error = None;
                    // A replay can queue many change notices for one resource.
                    // Fold them into one list and apply statuses in stream order.
                    let pending = std::iter::from_fn(|| stream.messages.try_recv().ok());
                    for event in first.into_iter().chain(pending) {
                        consumed = true;
                        match event {
                            StreamMessage::ResourceEventsChanged { .. } => refresh = true,
                            StreamMessage::Error { message, .. } => {
                                watch_error = Some(Some(message));
                                catch_up = true;
                            }
                            StreamMessage::Status {
                                status: StreamStatus::Connected,
                                ..
                            } => {
                                watch_error = Some(None);
                                refresh |= catch_up;
                                catch_up = false;
                            }
                            StreamMessage::Status { .. } => catch_up = true,
                            StreamMessage::Stopped { .. } => {
                                ended = true;
                                break;
                            }
                            _ => {}
                        }
                    }
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
                        // A recovery tick changes nothing shown unless it refreshes.
                        if consumed || refresh {
                            cx.notify();
                        }
                    });
                    if updated.is_err() {
                        return;
                    }
                    if refresh {
                        let started = std::time::Instant::now();
                        let result = backend.events(resource.clone(), uid.clone()).await;
                        let updated = this.update(cx, |this, cx| {
                            if let Detail::Ready(detail) = &mut this.detail {
                                match result {
                                    Ok(rows) => detail.events.set_ready(number, rows),
                                    Err(error) => detail.events.set_failed(number, error.message),
                                }
                            }
                            cx.notify();
                        });
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
