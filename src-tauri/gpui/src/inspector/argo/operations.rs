//! One Rust-owned, single-use review per selected Application.
use super::super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
    request::InFlight,
};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{Context, Window, div, prelude::*, px};
use kubecove_lib::models::{
    ArgoApplicationInspector, ArgoOperationConfirmation, ArgoOperationRequest,
};
use std::rc::Rc;

/// One review's lifecycle. Each step carries what it needs, and a step that runs a
/// read owns it, so leaving the step cancels the read.
#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    /// Reads the Application and the preflight for `action`.
    Checking {
        action: String,
        _read: InFlight,
    },
    Review(Box<Reviewed>),
    Submitting(Submission),
    /// Retry reviews `retry` again.
    Failed {
        message: String,
        retry: Box<ArgoOperationRequest>,
    },
    /// The submission's result is unknown; only a reload can tell.
    Unconfirmed {
        message: String,
        action: String,
    },
}
impl Phase {
    /// The action whose operation is in flight, which disables the operation buttons
    /// and spins the one that started it.
    pub(super) fn in_flight(&self) -> Option<&str> {
        match self {
            Phase::Checking { action, .. } | Phase::Unconfirmed { action, .. } => Some(action),
            Phase::Submitting(submission) => Some(&submission.action),
            Phase::Idle | Phase::Review(_) | Phase::Failed { .. } => None,
        }
    }
}

/// An allowed preflight, parsed once when it arrives.
pub(super) struct Reviewed {
    session_id: String,
    /// The canonical request the backend reviewed. The panel shows it and the
    /// outcome observer watches it.
    request: ArgoOperationRequest,
    action: String,
    transport: String,
    /// The Application as it was before the operation, which tells the observer
    /// whether Argo CD has acted on it since.
    baseline: ArgoApplicationInspector,
    /// The request the user asked for, which a refused submission retries.
    asked: ArgoOperationRequest,
}

/// A confirmed review whose submission runs. Closing the detail moves it to
/// `ArgoState::submissions`, so an accepted outcome is still observed.
pub(super) struct Submission {
    pub(super) id: Rc<()>,
    action: String,
    _read: InFlight,
}
impl Inspector {
    /// Moves a submission in flight out of the detail that is about to close,
    /// so its outcome still reaches the observer.
    pub(in crate::inspector) fn keep_argo_submission(&mut self) {
        if let Detail::Ready(detail) = &mut self.detail
            && let Some(argo) = detail.argo.as_mut()
            && matches!(argo.operation, Phase::Submitting(_))
            && let Phase::Submitting(submission) = std::mem::take(&mut argo.operation)
        {
            self.argo.submissions.push(submission);
        }
    }

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
        if state.operation.in_flight().is_some() {
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
        let Phase::Failed { retry, .. } = &state.operation else {
            return;
        };
        let request = ArgoOperationRequest::clone(retry);
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
        let action = request.action.clone();
        let retry = request.clone();
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
        // Replacing the step cancels an earlier check, and closing the detail drops it,
        // so a running check always belongs to the open review.
        let check = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(state) = this.argo_tab_state() else {
                    return;
                };
                state.operation = match result {
                    Ok((baseline, review)) => {
                        match (review.allowed, review.session_id, review.reviewed_request) {
                            (true, Some(session_id), Some(request)) => {
                                Phase::Review(Box::new(Reviewed {
                                    session_id,
                                    request,
                                    action: review.action,
                                    transport: review.transport,
                                    baseline,
                                    asked: retry,
                                }))
                            }
                            _ => Phase::Failed {
                                message: review
                                    .reason
                                    .unwrap_or_else(|| "Operation unavailable".into()),
                                retry: Box::new(retry),
                            },
                        }
                    }
                    Err(error) => Phase::Failed {
                        message: error.message,
                        retry: Box::new(retry),
                    },
                };
                cx.notify();
            });
        });
        state.operation = Phase::Checking {
            action,
            _read: check.into(),
        };
        cx.notify();
    }
    pub(super) fn cancel_argo_review(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.argo_tab_state() {
            if matches!(state.operation, Phase::Submitting(_)) {
                return;
            }
            state.operation = Phase::Idle;
            cx.notify();
        }
    }
    pub(super) fn confirm_argo_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        if !matches!(state.operation, Phase::Review(_)) {
            return;
        }
        // Take the review before issuing a request. A failed submission cannot replay it.
        let Phase::Review(review) = std::mem::take(&mut state.operation) else {
            return;
        };
        let Reviewed {
            session_id,
            request,
            baseline,
            asked,
            ..
        } = *review;
        let id = Rc::new(());
        let current = id.clone();
        let action = asked.action.clone();
        let unconfirmed_action = action.clone();
        let read = self.backend.run_argo(ArgoOperationConfirmation {
            confirmation: session_id.clone(),
            session_id,
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let accepted = result.as_ref().is_ok_and(|r| r.accepted);
                // A closed or reopened detail has no matching review to update, but an
                // accepted submission is still observed, as the web tracker does.
                if let Some(state) = this.argo_tab_state()
                    && matches!(&state.operation, Phase::Submitting(s) if Rc::ptr_eq(&s.id, &current))
                {
                    state.operation = match result {
                        Ok(result) if result.accepted => Phase::Idle,
                        Ok(result) => Phase::Failed {
                            message: result.message,
                            retry: Box::new(asked),
                        },
                        Err(error) => Phase::Unconfirmed {
                            message: format!(
                                "Outcome unconfirmed. Reload before retrying. {}",
                                error.message
                            ),
                            action: unconfirmed_action,
                        },
                    };
                }
                // The submission has finished, so a closed detail's entry is done too.
                this.argo.submissions
                    .retain(|submission| !Rc::ptr_eq(&submission.id, &current));
                if accepted {
                    this.observe_argo_operation(request, baseline, cx);
                }
                cx.notify();
            });
        });
        if let Some(state) = self.argo_tab_state() {
            state.operation = Phase::Submitting(Submission {
                id,
                action,
                _read: task.into(),
            });
        }
        cx.notify();
    }
}
pub(super) fn panel(phase: &Phase, cx: &mut Context<Inspector>) -> gpui_kit::AnyElement {
    let body = div().flex().flex_col().gap(px(8.));
    match phase {
        Phase::Idle => body.into_any_element(),
        Phase::Checking { .. } => body
            .child(ui::xs("Checking exact Application and authorization…"))
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-review-cancel")
                    .test_support()
                    .child("Cancel review")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_argo_review(cx))),
            )
            .into_any_element(),
        Phase::Submitting(_) => body
            .child(ui::xs("Submitting reviewed operation…"))
            .into_any_element(),
        Phase::Failed { message, .. } | Phase::Unconfirmed { message, .. } => {
            let unconfirmed = matches!(phase, Phase::Unconfirmed { .. });
            body.child(
                ui::xs(message.clone())
                    .id("argo-operation-result")
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(Tokens::destructive()),
            )
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-operation-retry")
                    .test_support()
                    .child(if unconfirmed {
                        "Retry state refresh"
                    } else {
                        "Retry operation"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this
                            .argo_tab_state()
                            .is_some_and(|s| matches!(s.operation, Phase::Unconfirmed { .. }))
                        {
                            this.reload_argo(window, cx);
                        } else {
                            this.retry_argo_operation(window, cx);
                        }
                    })),
            )
            .into_any_element()
        }
        Phase::Review(review) => {
            let request = &review.request;
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
