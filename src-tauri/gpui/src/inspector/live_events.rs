//! Selected-resource event stream, owned by the detail panel.
use super::{
    Inspector,
    detail::Detail,
    events::EventsLoad,
    live_data::{COALESCE, RECOVERY, WatchInput, receive_or_tick},
};
use crate::{backend::WatchTarget, incident};
use gpui_kit::{Context, Window};
use kubecove_lib::models::{StreamMessage, StreamStatus};
use std::rc::Rc;

impl Inspector {
    pub(super) fn watch_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let resource = detail.data.summary.clone();
        let uid = detail.data.metadata["uid"].as_str().map(str::to_owned);
        let request = self.detail_request.clone();
        let backend = self.backend.clone();
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
                        let _ = this.update(cx, |this, cx| {
                            if Rc::ptr_eq(&this.detail_request, &request)
                                && let Detail::Ready(detail) = &mut this.detail
                            {
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
                    let closed = matches!(input, WatchInput::Closed);
                    if tick {
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                    }
                    let event = match input {
                        WatchInput::Message(message) => Some(message),
                        WatchInput::Closed | WatchInput::Tick => None,
                    };
                    let ended = matches!(event, Some(StreamMessage::Stopped { .. })) || closed;
                    let mut refresh = tick && catch_up;
                    let mut error = None;
                    let mut clear_error = false;
                    match event {
                        Some(StreamMessage::ResourceEventsChanged { .. }) => refresh = true,
                        Some(StreamMessage::Error { message, .. }) => {
                            error = Some(message);
                            catch_up = true;
                        }
                        Some(StreamMessage::Status {
                            status: StreamStatus::Connected,
                            ..
                        }) => {
                            clear_error = true;
                            refresh = catch_up;
                            catch_up = false;
                        }
                        Some(StreamMessage::Status { .. }) => catch_up = true,
                        _ => {}
                    }
                    let valid = this
                        .update(cx, |this, cx| {
                            if !Rc::ptr_eq(&this.detail_request, &request) {
                                return false;
                            }
                            if let Detail::Ready(detail) = &mut this.detail {
                                refresh |=
                                    tick && matches!(detail.events.load, EventsLoad::Failed(_));
                                if clear_error {
                                    detail.events.watch_error = None;
                                }
                                if let Some(error) = error {
                                    detail.events.watch_error = Some(error);
                                }
                            }
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !valid {
                        return;
                    }
                    if refresh {
                        let started = std::time::Instant::now();
                        let result = backend.events(resource.clone(), uid.clone()).await;
                        if result.is_err() {
                            catch_up = true;
                        }
                        let valid = this
                            .update(cx, |this, cx| {
                                if !Rc::ptr_eq(&this.detail_request, &request) {
                                    return false;
                                }
                                if let Detail::Ready(detail) = &mut this.detail {
                                    match result {
                                        Ok(mut rows) => {
                                            incident::sort_events(&mut rows);
                                            detail.events.load = EventsLoad::Ready(rows);
                                            detail.events.watch_error = None;
                                        }
                                        Err(error) => {
                                            detail.events.watch_error = Some(error.message)
                                        }
                                    }
                                }
                                cx.notify();
                                true
                            })
                            .unwrap_or(false);
                        if !valid {
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
