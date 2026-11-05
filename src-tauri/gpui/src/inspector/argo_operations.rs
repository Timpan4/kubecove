//! One Rust-owned, single-use review per selected Application.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
};
use crate::{theme::Tokens, ui};
use gpui_kit::{Context, Task, TestSupportExt, Window, div, prelude::*, px};
use kubecove_lib::models::{
    ArgoOperationConfirmation, ArgoOperationPreflight, ArgoOperationRequest,
};
use std::rc::Rc;

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    Checking,
    Review(Box<ArgoOperationPreflight>),
    Submitting,
    Failed(String),
    Unconfirmed(String),
}
#[derive(Default)]
pub(super) struct Operation {
    pub phase: Phase,
    request: Rc<()>,
    task: Option<Task<()>>,
    baseline: Option<kubecove_lib::models::ArgoApplicationInspector>,
    pub(super) last_request: Option<ArgoOperationRequest>,
}
impl Inspector {
    pub(super) fn review_argo_operation(
        &mut self,
        label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection_id = self.argo_connection_id();
        let source = self.argo_source_key().map(str::to_owned);
        if let Detail::Ready(d) = &self.detail
            && let Some(snapshot) = d.argo.as_ref().and_then(|s| s.inspector.as_ref())
            && self.argo_operation_busy(&snapshot.application)
        {
            return;
        }
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let Some(state) = detail.argo.as_mut() else {
            return;
        };
        if matches!(
            state.operation.phase,
            Phase::Checking | Phase::Submitting | Phase::Unconfirmed(_)
        ) {
            return;
        }
        let Some(snapshot) = &state.inspector else {
            return;
        };
        let action = match label {
            "Refresh" => "refresh",
            "Hard refresh" => "hardRefresh",
            "Sync" | "Advanced Sync" => "sync",
            "Rollback" => "rollback",
            _ => return,
        };
        let advanced = label == "Advanced Sync";
        if advanced && state.options.name.read(cx).value().as_str() != snapshot.application.name {
            return;
        }
        let request = ArgoOperationRequest {
            kubeconfig_env_var: source,
            connection_id: if snapshot.transport == "connected" {
                connection_id
            } else {
                None
            },
            transport: snapshot.transport.clone(),
            application: snapshot.application.clone(),
            action: action.into(),
            resource_version: snapshot.application.resource_version.clone(),
            cluster_context: snapshot.application.context.clone(),
            history_id: if action == "rollback" {
                state
                    .options
                    .history
                    .or_else(|| snapshot.history.iter().skip(1).find_map(|h| h.id))
            } else {
                None
            },
            revision: advanced
                .then(|| state.options.revision.read(cx).value().trim().to_owned())
                .filter(|s| !s.is_empty()),
            prune: if advanced {
                Some(state.options.prune.unwrap_or(false))
            } else if action == "sync" {
                Some(
                    serde_yaml::from_str::<serde_json::Value>(&detail.data.yaml)
                        .ok()
                        .and_then(|v| {
                            v.pointer("/spec/syncPolicy/automated/prune")
                                .and_then(|v| v.as_bool())
                        })
                        .unwrap_or(false),
                )
            } else {
                None
            },
            dry_run: (action == "sync").then_some(advanced && state.options.dry_run),
            force: (action == "sync").then_some(advanced && state.options.force),
            ..Default::default()
        };
        state.refresh_options = false;
        state.options.open = false;
        state.options.confirming = false;
        self.start_argo_review(request, window, cx);
    }
    pub(super) fn retry_argo_operation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        if !matches!(state.operation.phase, Phase::Failed(_)) {
            return;
        }
        let Some(request) = state.operation.last_request.clone() else {
            return;
        };
        self.start_argo_review(request, window, cx);
    }
    fn start_argo_review(
        &mut self,
        request: ArgoOperationRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backend = self.backend.clone();
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        if state.inspector.as_ref().is_none_or(|s| {
            let app = &s.application;
            app.context != request.application.context
                || app.workspace_id != request.application.workspace_id
                || app.namespace != request.application.namespace
                || app.name != request.application.name
                || app.uid != request.application.uid
        }) {
            return;
        }
        state.operation.last_request = Some(request.clone());
        let read = async move {
            let before = backend
                .inspect_argo(crate::backend::ArgoInspection {
                    application: request.application.clone(),
                    connection_id: request.connection_id.clone(),
                    source: request.kubeconfig_env_var.clone(),
                })
                .await?;
            if request.application.uid.is_some()
                && request.application.uid != before.application.uid
            {
                return Err(kubecove_lib::models::AppError::new(
                    "Application was replaced. Reload before reviewing.",
                    kubecove_lib::models::AppErrorKind::ArgoConnection,
                ));
            }
            let review = backend.preflight_argo(request).await?;
            Ok((before, review))
        };
        state.operation.request = Rc::new(());
        let current = state.operation.request.clone();
        state.operation.phase = Phase::Checking;
        state.operation.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(state) = this.argo_tab_state() else {
                    return;
                };
                if !Rc::ptr_eq(&current, &state.operation.request) {
                    return;
                }
                state.operation.phase = match result {
                    Ok((before, review))
                        if review.allowed
                            && review.session_id.is_some()
                            && review.reviewed_request.is_some() =>
                    {
                        state.operation.baseline = Some(before);
                        Phase::Review(Box::new(review))
                    }
                    Ok((_, review)) => Phase::Failed(
                        review
                            .reason
                            .unwrap_or_else(|| "Operation unavailable".into()),
                    ),
                    Err(error) => Phase::Failed(error.message),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn cancel_argo_review(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.argo_tab_state() {
            if matches!(state.operation.phase, Phase::Submitting) {
                return;
            }
            state.operation = Operation::default();
            cx.notify();
        }
    }
    pub(super) fn confirm_argo_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        let Phase::Review(review) = &state.operation.phase else {
            return;
        };
        let Some(session_id) = review.session_id.clone() else {
            return;
        };
        let observed_request = review.reviewed_request.clone().expect("canonical review");
        let baseline = state
            .operation
            .baseline
            .clone()
            .expect("fresh review baseline");
        // Take the review before issuing a request. A failed submission cannot replay it.
        state.operation.phase = Phase::Submitting;
        let current = state.operation.request.clone();
        let read = self.backend.run_argo(ArgoOperationConfirmation {
            confirmation: session_id.clone(),
            session_id,
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(state) = this.argo_tab_state() else {
                    return;
                };
                if !Rc::ptr_eq(&current, &state.operation.request) {
                    return;
                }
                let accepted = result.as_ref().is_ok_and(|r| r.accepted);
                state.operation.phase = match result {
                    Ok(result) if result.accepted => Phase::Idle,
                    Ok(result) => Phase::Failed(result.message),
                    Err(error) => Phase::Unconfirmed(format!(
                        "Outcome unconfirmed. Reload before retrying. {}",
                        error.message
                    )),
                };
                if accepted {
                    this.observe_argo_operation(observed_request, baseline, cx);
                }
                cx.notify();
            });
        });
        if let Some(state) = self.argo_tab_state() {
            state.operation.task = Some(task);
        }
        cx.notify();
    }
}
pub(super) fn panel(state: &Operation, cx: &mut Context<Inspector>) -> gpui_kit::AnyElement {
    let body = div().flex().flex_col().gap(px(8.));
    match &state.phase {
        Phase::Idle => body.into_any_element(),
        Phase::Checking => body
            .child(ui::xs("Checking exact Application and authorization…"))
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-review-cancel")
                    .test_support()
                    .child("Cancel review")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_argo_review(cx))),
            )
            .into_any_element(),
        Phase::Submitting => body
            .child(ui::xs("Submitting reviewed operation…"))
            .into_any_element(),
        Phase::Failed(message) | Phase::Unconfirmed(message) => body
            .child(
                ui::xs(message.clone())
                    .id("argo-operation-result")
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(
                        if matches!(state.phase, Phase::Failed(_) | Phase::Unconfirmed(_)) {
                            Tokens::destructive()
                        } else {
                            Tokens::foreground()
                        },
                    ),
            )
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-operation-retry")
                    .test_support()
                    .child(if matches!(state.phase, Phase::Unconfirmed(_)) {
                        "Retry state refresh"
                    } else {
                        "Retry operation"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this
                            .argo_tab_state()
                            .is_some_and(|s| matches!(s.operation.phase, Phase::Unconfirmed(_)))
                        {
                            this.reload_argo(window, cx);
                        } else {
                            this.retry_argo_operation(window, cx);
                        }
                    })),
            )
            .into_any_element(),
        Phase::Review(review) => {
            let request = review
                .reviewed_request
                .as_ref()
                .expect("review contains canonical request");
            let target = format!(
                "{} {} {} Application {} {}",
                review.action,
                review.transport,
                request.application.context.as_deref().unwrap_or_default(),
                request.application.namespace.as_deref().unwrap_or_default(),
                request.application.name
            );
            let summary = format!(
                "Revision: {}. History: {}. Prune: {}. Dry run: {}. Force: {}.",
                request.revision.as_deref().unwrap_or("Application default"),
                request
                    .history_id
                    .map_or_else(|| "none".into(), |id| id.to_string()),
                flag(request.prune),
                flag(request.dry_run),
                flag(request.force)
            );
            body.id("argo-review")
                .test_support()
                .aria_label(target.clone())
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(Tokens::border())
                .child(ui::sm("Review Argo CD operation"))
                .child(ui::xs(target))
                .child(
                    ui::xs(summary.clone())
                        .id("argo-review-options")
                        .test_support()
                        .aria_label(summary),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .child(
                            button(Variant::Default, 24., false)
                                .id("argo-review-confirm")
                                .test_support()
                                .child("Confirm Argo CD operation")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_argo_review(window, cx)
                                })),
                        )
                        .child(
                            button(Variant::Outline, 24., false)
                                .id("argo-review-cancel")
                                .test_support()
                                .child("Cancel review")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_argo_review(cx)),
                                ),
                        ),
                )
                .into_any_element()
        }
    }
}
fn flag(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "not applicable",
    }
}
