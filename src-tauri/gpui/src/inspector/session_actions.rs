//! `portForwardLifecycle.ts`: reconnect stops before starting, and always reloads.
use super::{
    Inspector,
    controls::{Variant, button},
    port_forwards::{ActionFeedback, SessionAction},
};
use crate::targets::TestSupportExt;
use crate::{
    backend::Read,
    live_sessions::{forward_status, forward_title},
    theme::Tokens,
    ui,
};
use gpui_kit::{Context, SharedString, Window, div, prelude::*, px};
use kubecove_lib::models::{AppError, AppErrorKind, PortForwardRequest, PortForwardSessionSummary};

impl Inspector {
    fn run_forward_action(
        &mut self,
        read: Read<Vec<Result<PortForwardSessionSummary, AppError>>>,
        clear_local: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions.action.running() {
            return;
        }
        self.sessions.action = SessionAction::Running;
        let context = self.scope.context.clone();
        let source = self
            .sessions
            .load
            .data()
            .and_then(|s| s.source_key.clone());
        let backend = self.backend.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            if let Ok(sessions) = &result {
                let belongs = this
                    .update(cx, |this, _| {
                        let current = this
                            .sessions
                            .load
                            .data()
                            .and_then(|s| s.source_key.clone());
                        current == source
                            && (this
                                .persisted
                                .preferences
                                .keep_live_sessions_on_workspace_switch
                                || this.scope.context == context)
                    })
                    .unwrap_or(false);
                if !belongs {
                    for session in sessions.iter().filter_map(|s| s.as_ref().ok()) {
                        let _ = backend.stop_forward(session.id.clone()).await;
                    }
                }
            }
            let _ = this.update_in(cx, |this, window, cx| {
                if clear_local
                    && let Ok(results) = &result
                    && let Some(session) = results.iter().find_map(|r| r.as_ref().ok())
                    && let super::detail::Detail::Ready(detail) = &mut this.detail
                    && detail.data.summary.name == session.target_name
                    && detail.data.summary.kind == session.target_kind
                    && detail.data.summary.namespace.as_ref() == Some(&session.namespace)
                {
                    detail
                        .forward
                        .local
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                let errors = match result {
                    Err(error) => vec![error.message],
                    Ok(results) => results
                        .into_iter()
                        .filter_map(Result::err)
                        .map(|e| e.message)
                        .collect(),
                };
                this.sessions.action = SessionAction::finished(errors);
                this.restore_prompt
                    .finish_start(this.sessions.action.failure().map(str::to_owned));
                this.load_sessions(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn session_env_var(&self) -> Option<String> {
        self.sessions
            .load
            .data()
            .and_then(|s| s.source_key.as_deref())
            .and_then(|s| s.strip_prefix("kubeconfigEnv="))
            .filter(|s| *s != "KUBECONFIG")
            .map(str::to_owned)
    }
    pub(super) fn run_session_action(&mut self, read: Read<()>, cx: &mut Context<Self>) {
        if self.sessions.action.running() {
            return;
        }
        self.sessions.action = SessionAction::Running;
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                this.sessions.action =
                    SessionAction::finished(result.err().map(|e| e.message).into_iter().collect());
                this.load_sessions(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn start_forward_request(
        &mut self,
        request: PortForwardRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions.action.running() {
            return;
        }
        let read = self.backend.start_forward(request);
        let read = self.timings.measure("start_forward", read);
        self.run_forward_action(
            Box::pin(async move { read.await.map(|s| vec![Ok(s)]) }),
            true,
            window,
            cx,
        );
    }
    pub(super) fn stop_forward_id(&mut self, id: String, cx: &mut Context<Self>) {
        if self.sessions.action.running() {
            return;
        }
        let read = self.backend.stop_forward(id);
        let read = self.timings.measure("stop_forward", read);
        self.run_session_action(
            Box::pin(async move {
                if read.await? {
                    Ok(())
                } else {
                    Err(AppError::new(
                        "Session was not stopped",
                        AppErrorKind::Cluster,
                    ))
                }
            }),
            cx,
        );
    }
    pub(super) fn stop_exec_id(&mut self, id: String, cx: &mut Context<Self>) {
        if self.sessions.action.running() {
            return;
        }
        let read = self.backend.stop_exec(id);
        self.run_session_action(
            Box::pin(async move {
                if read.await? {
                    Ok(())
                } else {
                    Err(AppError::new(
                        "Session was not stopped",
                        AppErrorKind::Cluster,
                    ))
                }
            }),
            cx,
        );
    }
    pub(super) fn reconnect_forward(
        &mut self,
        session: PortForwardSessionSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions.action.running() {
            return;
        }
        let backend = self.backend.clone();
        self.run_forward_action(
            Box::pin(async move {
                if !backend.stop_forward(session.id).await? {
                    return Err(AppError::new(
                        "Session was not stopped",
                        AppErrorKind::Cluster,
                    ));
                }
                backend
                    .start_forward(PortForwardRequest {
                        cluster_context: session.cluster_context,
                        kubeconfig_env_var: session.kubeconfig_env_var,
                        namespace: session.namespace,
                        target_kind: Some(session.target_kind),
                        target_name: Some(session.target_name),
                        pod_name: None,
                        remote_port: session.remote_port.into(),
                        local_port: Some(session.local_port.into()),
                    })
                    .await
                    .map(|s| vec![Ok(s)])
            }),
            false,
            window,
            cx,
        );
    }
    pub(super) fn forward_card(
        &self,
        session: &PortForwardSessionSummary,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = session.id.clone();
        div()
            .id(SharedString::from(format!("forward-card:{}", session.id)))
            .test_support()
            .aria_label(forward_title(session))
            .aria_description(format!(
                "{} {} {}:{}",
                session.local_port,
                forward_status(session.status).0,
                session.resolved_pod_name,
                session.resolved_pod_port
            ))
            .p(px(12.))
            .rounded(px(6.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::background())
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .min_w_0()
                    .child(ui::medium(ui::xs(format!(
                        "127.0.0.1:{} -> {}",
                        session.local_port,
                        forward_title(session)
                    ))))
                    .child(
                        ui::muted(ui::xs(crate::live_sessions::local_url(session)))
                            .font_family(crate::theme::MONO),
                    )
                    .child(ui::muted(ui::xs(format!(
                        "Pod/{}:{}",
                        session.resolved_pod_name, session.resolved_pod_port
                    ))))
                    .when_some(session.last_error.clone(), |body, error| {
                        let message = format!("Port-forward failed: {error}");
                        body.child(
                            ui::xs(message.clone())
                                .id(SharedString::from(format!(
                                    "forward-card-error:{}",
                                    session.id
                                )))
                                .test_support()
                                .aria_label(message)
                                .text_color(Tokens::destructive())
                                .mt(px(4.)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::outline_badge(forward_status(session.status).0))
                    .child({
                        let url = crate::live_sessions::local_url(session);
                        button(Variant::Outline, 24., false)
                            .id(SharedString::from(format!("copy-forward:{}", session.id)))
                            .test_support()
                            .aria_label("Copy")
                            .child(ui::icon("copy", 14., Tokens::foreground()))
                            .child(ui::xs("Copy"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                    url.clone(),
                                ))
                            }))
                    })
                    .child(
                        button(Variant::Outline, 24., self.sessions.action.running())
                            .id(SharedString::from(format!(
                                "stop-forward:{}",
                                forward_title(session)
                            )))
                            .test_support()
                            .child(ui::icon("square", 14., Tokens::foreground()))
                            .child(ui::xs("Stop"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.stop_forward_id(id.clone(), cx)
                            })),
                    ),
            )
    }
    pub(super) fn save_forward_preset(&mut self, cx: &mut Context<Self>) {
        let preset_key = self.preset_key();
        match self.forward_request(cx) {
            Ok(request) => {
                let presets = self
                    .persisted
                    .saved_forward_presets
                    .entry(preset_key)
                    .or_default();
                let duplicate = presets.iter().any(|p| {
                    serde_json::to_value(&p.request).ok() == serde_json::to_value(&request).ok()
                });
                // Wording from `saveServicePreset` in `PortForwardTab.svelte`.
                let message = if duplicate {
                    "Preset already saved for this Service."
                } else {
                    presets.push(crate::preferences::SavedForward {
                        request,
                        label: String::new(),
                    });
                    self.save_preferences(cx);
                    "Saved Service forward to this workspace."
                };
                self.sessions.action = SessionAction::Done(ActionFeedback::Info(message.into()));
            }
            Err(error) => self.sessions.action = SessionAction::Done(ActionFeedback::Failed(error)),
        }
        cx.notify();
    }
    pub(super) fn restore_forwards(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sessions.action.running() {
            return;
        }
        let requests = self
            .persisted
            .saved_forward_presets
            .get(&self.preset_key())
            .map(|presets| presets.iter().map(|p| p.request.clone()).collect())
            .unwrap_or_default();
        self.restore_forward_requests(requests, window, cx);
    }
    pub(super) fn start_saved_forward(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions.action.running() {
            return;
        }
        let Some(saved) = self
            .persisted
            .saved_forward_presets
            .get(&self.preset_key())
            .and_then(|p| p.get(index))
        else {
            return;
        };
        self.restore_forward_requests(vec![saved.request.clone()], window, cx);
    }
    fn restore_forward_requests(
        &mut self,
        requests: Vec<PortForwardRequest>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backend = self.backend.clone();
        self.run_forward_action(
            Box::pin(async move {
                let mut started = Vec::new();
                for request in requests {
                    let sessions = match backend.live_sessions().await {
                        Ok(sessions) => sessions,
                        Err(error) => {
                            started.push(Err(error));
                            continue;
                        }
                    };
                    let previous = sessions
                        .for_context(&request.cluster_context)
                        .0
                        .into_iter()
                        .find(|s| {
                            Some(&s.target_kind) == request.target_kind.as_ref()
                                && Some(&s.target_name) == request.target_name.as_ref()
                                && s.namespace == request.namespace
                                && i64::from(s.remote_port) == request.remote_port
                                && request
                                    .local_port
                                    .is_none_or(|p| p == i64::from(s.local_port))
                        });
                    if let Some(previous) = previous {
                        if !matches!(
                            previous.status,
                            kubecove_lib::models::PortForwardSessionStatus::Stopped
                                | kubecove_lib::models::PortForwardSessionStatus::Error
                        ) {
                            continue;
                        }
                        match backend.stop_forward(previous.id.clone()).await {
                            Ok(true) => {}
                            Ok(false) => {
                                started.push(Err(AppError::new(
                                    "Session was not stopped",
                                    AppErrorKind::Cluster,
                                )));
                                continue;
                            }
                            Err(error) => {
                                started.push(Err(error));
                                continue;
                            }
                        }
                    }
                    started.push(backend.start_forward(request).await);
                }
                Ok(started)
            }),
            false,
            window,
            cx,
        );
    }
}
