//! The stream task behind one resource watch: it reconnects, relists after
//! changes and recovers on `RECOVERY` ticks until its owner changes.
use super::{COALESCE, RECOVERY, Registration, WatchInput, receive_or_tick};
use crate::backend::{KindRows, LogStream, Scope, WatchTarget};
use crate::inspector::Inspector;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{StreamMessage, StreamStatus, WatchResourceKey};

impl Inspector {
    /// Runs the watch `id` of `key` in `scope` for the current owner.
    pub(super) fn resource_watch_task(
        &mut self,
        scope: Scope,
        key: WatchResourceKey,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let backend = self.backend.clone();
        let owner = self.live.owner_epoch;
        cx.spawn_in(window, async move |this, cx| {
            let mut lost = false;
            let mut timer = Box::pin(cx.background_executor().timer(RECOVERY));
            loop {
                // Until the rows first settle, a reopened watch resumes at
                // the version read before them; then the rows' version takes over.
                let resource_version = this
                    .update(cx, |this, _| {
                        this.watch_resume_version(&scope.context, &key)
                    })
                    .ok()
                    .flatten();
                let read = {
                    let _span = crate::frame_profile::scope("watch", "open_factory");
                    backend.watch(WatchTarget::Resource {
                        scope: scope.clone(),
                        key: Box::new(key.clone()),
                        resource_version,
                    })
                };
                let mut stream = match read.await {
                    Ok(stream) => stream,
                    Err(error) => {
                        lost = true;
                        let _ = this.update(cx, |this, cx| {
                            if this.live.owner_epoch == owner
                                && let Some(link) =
                                    this.live.watches.get_mut(&id).and_then(Registration::link)
                            {
                                link.failed(error.message);
                            }
                            cx.notify();
                        });
                        timer.as_mut().await;
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                        let recycled = this
                            .update(cx, |this, _| this.live.take_buffer(&id))
                            .unwrap_or_default();
                        let mut recycled = Some(recycled);
                        let retry_backend = backend.clone();
                        let retry_scope = scope.clone();
                        let retry_key = key.clone();
                        let rows =
                            crate::read_retry::read(cx.background_executor().clone(), move || {
                                retry_backend.kind_resources(
                                    retry_scope.clone(),
                                    retry_key.clone(),
                                    recycled.take().unwrap_or_default(),
                                )
                            })
                            .await;
                        let valid = this
                            .update(cx, |this, cx| {
                                if this.live.owner_epoch != owner {
                                    return false;
                                }
                                match rows {
                                    Ok(KindRows {
                                        rows,
                                        resource_version,
                                    }) => {
                                        this.queue_live_patch(
                                            super::super::resource_observation::Patch {
                                                scope: scope.clone(),
                                                key: key.clone(),
                                                rows,
                                                resource_version,
                                                names: None,
                                                watch: Some(id.clone()),
                                                kind_recovery: None,
                                            },
                                            cx,
                                        );
                                        // As after a reload: the rows are current again.
                                        if let Some(w) = this.live.watches.get_mut(&id) {
                                            w.reload_error = None;
                                        }
                                    }
                                    Err(error) => {
                                        if let Some(w) = this.live.watches.get_mut(&id) {
                                            w.reload_error = Some(error.message);
                                        }
                                    }
                                }
                                true
                            })
                            .unwrap_or(false);
                        if !valid {
                            return;
                        }
                        continue;
                    }
                };
                let mut forbidden = false;
                let mut read_failed = false;
                loop {
                    let input = receive_or_tick(&mut stream, &mut timer).await;
                    let tick = matches!(input, WatchInput::Tick);
                    let batch_profile = crate::frame_profile::scope("watch", "batch");
                    let closed = matches!(input, WatchInput::Closed);
                    if tick {
                        timer = Box::pin(cx.background_executor().timer(RECOVERY));
                    }
                    let mut batch = match input {
                        WatchInput::Message(message) => vec![message],
                        WatchInput::Closed | WatchInput::Tick => Vec::new(),
                    };
                    while let Ok(message) = stream.messages.try_recv() {
                        batch.push(message);
                    }
                    let ended = batch
                        .iter()
                        .any(|event| matches!(event, StreamMessage::Stopped { .. }))
                        || closed;
                    if closed {
                        lost = true;
                    }
                    let missing_kind = if tick {
                        this.update(cx, |this, _| {
                            this.observation.kind_error_started(&scope.context, &id)
                        })
                        .ok()
                        .flatten()
                    } else {
                        None
                    };
                    let mut reload = tick && (lost || forbidden || read_failed);
                    // A recovery tick with nothing to recover leaves the view as it is.
                    if tick && batch.is_empty() && !reload && missing_kind.is_none() {
                        continue;
                    }
                    let mut names = Some(crate::model::ObservedNames::default());
                    let mut transitions = Vec::new();
                    for event in batch {
                        match event {
                            StreamMessage::Status { status, .. } => {
                                forbidden = status == StreamStatus::Forbidden;
                                if status != StreamStatus::Connected {
                                    lost = true;
                                } else if lost {
                                    reload = true;
                                    names = None;
                                    lost = false;
                                }
                                transitions.push(event);
                            }
                            StreamMessage::Error { .. } | StreamMessage::Stopped { .. } => {
                                lost = true;
                                transitions.push(event);
                            }
                            StreamMessage::ResourceChanged { target, .. } => {
                                reload = true;
                                if let (Some(names), Some(name)) = (&mut names, target.name) {
                                    names.insert((target.namespace, name));
                                } else {
                                    names = None;
                                }
                            }
                            _ => {}
                        }
                    }
                    // Connected messages can renew a healthy watch without requesting a relist.
                    let kind_recovery = missing_kind.filter(|_| !reload && !lost && !forbidden);
                    reload |= missing_kind.is_some();
                    // After a failed reload, rows it should have changed may be
                    // stale, so the next reload lists the kind, not just its names.
                    if tick || read_failed {
                        names = None;
                    }
                    drop(batch_profile);
                    let valid = this
                        .update_in(cx, |this, _window, cx| {
                            if this.live.owner_epoch != owner {
                                return false;
                            }
                            if reload
                                && names.is_some()
                                && this.observation.kind_read_failed(&scope.context, &id)
                            {
                                names = None;
                            }
                            let Some(w) = this.live.watches.get_mut(&id) else {
                                return false;
                            };
                            if let Some(link) = w.link() {
                                if closed {
                                    link.stopped();
                                }
                                for event in transitions {
                                    match event {
                                        StreamMessage::Status {
                                            status, message, ..
                                        } => link.status(status, message),
                                        StreamMessage::Error { message, .. } => {
                                            link.failed(message)
                                        }
                                        StreamMessage::Stopped { .. } => link.stopped(),
                                        _ => {}
                                    }
                                }
                            }
                            w.catching_up = reload && names.is_none();
                            if names.is_none() || !reload {
                                cx.notify();
                            }
                            true
                        })
                        .unwrap_or(false);
                    if !valid {
                        return;
                    }
                    if reload {
                        let started = std::time::Instant::now();
                        let recycled = this
                            .update(cx, |this, _| this.live.take_buffer(&id))
                            .unwrap_or_default();
                        let mut recycled = Some(recycled);
                        let retry_backend = backend.clone();
                        let retry_scope = scope.clone();
                        let retry_key = key.clone();
                        // Notices that name every change read only those rows.
                        // A tick, a reconnect or an unnamed notice lists the kind.
                        let named = names.as_ref().map(crate::model::ObservedNames::shared);
                        let rows =
                            crate::read_retry::read(cx.background_executor().clone(), move || {
                                let recycled = recycled.take().unwrap_or_default();
                                match &named {
                                    Some(named) => retry_backend.named_kind_resources(
                                        retry_scope.clone(),
                                        retry_key.clone(),
                                        named.clone(),
                                        recycled,
                                    ),
                                    None => retry_backend.kind_resources(
                                        retry_scope.clone(),
                                        retry_key.clone(),
                                        recycled,
                                    ),
                                }
                            })
                            .await;
                        let Ok(Some(failed)) = this.update_in(cx, |this, _window, cx| {
                            if this.live.owner_epoch != owner {
                                return None;
                            }
                            let failed = match rows {
                                Ok(KindRows {
                                    rows,
                                    resource_version,
                                }) => {
                                    this.queue_live_patch(
                                        super::super::resource_observation::Patch {
                                            scope: scope.clone(),
                                            key: key.clone(),
                                            rows,
                                            resource_version,
                                            names,
                                            watch: Some(id.clone()),
                                            kind_recovery,
                                        },
                                        cx,
                                    );
                                    if let Some(w) = this.live.watches.get_mut(&id) {
                                        w.reload_error = None;
                                        w.catching_up = false;
                                    }
                                    false
                                }
                                Err(error) => {
                                    // A retry for a selected-kind list belongs to its existing error.
                                    // A newer list can retire it without clearing the watch's own errors.
                                    let failed = kind_recovery.is_none();
                                    if let Some(w) = this.live.watches.get_mut(&id) {
                                        if failed {
                                            w.reload_error = Some(error.message);
                                        }
                                        w.catching_up = false;
                                    }
                                    failed
                                }
                            };
                            cx.notify();
                            Some(failed)
                        }) else {
                            return;
                        };
                        read_failed = failed;
                        if !tick {
                            cx.background_executor()
                                .timer(COALESCE.saturating_sub(started.elapsed()))
                                .await;
                        }
                    }
                    if ended {
                        if forbidden {
                            stream = LogStream::replay([]);
                        } else {
                            break;
                        }
                    }
                }
                drop(stream);
                timer.as_mut().await;
                timer = Box::pin(cx.background_executor().timer(RECOVERY));
            }
        })
    }
}
