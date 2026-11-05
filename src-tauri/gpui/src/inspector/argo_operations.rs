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
    Result(String),
    Failed(String),
    Unconfirmed(String),
}
#[derive(Default)]
pub(super) struct Operation {
    pub phase: Phase,
    request: Rc<()>,
    task: Option<Task<()>>,
}
impl Inspector {
    pub(super) fn review_argo_operation(
        &mut self,
        label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection_id = self.argo_connection_id();
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let Some(state) = detail.argo.as_mut() else {
            return;
        };
        if matches!(
            state.operation.phase,
            Phase::Checking | Phase::Submitting | Phase::Result(_) | Phase::Unconfirmed(_)
        ) {
            return;
        }
        let Some(snapshot) = &state.inspector else {
            return;
        };
        let action = match label {
            "Refresh" => "refresh",
            "Sync" => "sync",
            "Rollback" => "rollback",
            _ => return,
        };
        let request = ArgoOperationRequest {
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
                snapshot.history.iter().skip(1).find_map(|h| h.id)
            } else {
                None
            },
            prune: if action == "sync" {
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
            dry_run: (action == "sync").then_some(false),
            force: (action == "sync").then_some(false),
            ..Default::default()
        };
        let read = self.backend.preflight_argo(request);
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
                    Ok(review)
                        if review.allowed
                            && review.session_id.is_some()
                            && review.reviewed_request.is_some() =>
                    {
                        Phase::Review(Box::new(review))
                    }
                    Ok(review) => Phase::Failed(
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
                state.operation.phase = match result {
                    Ok(result) if result.accepted => Phase::Result(format!(
                        "{} Accepted; waiting for Argo CD to complete.",
                        result.message
                    )),
                    Ok(result) => Phase::Failed(result.message),
                    Err(error) => Phase::Unconfirmed(format!(
                        "Outcome unconfirmed. Reload before retrying. {}",
                        error.message
                    )),
                };
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
        Phase::Result(message) | Phase::Failed(message) | Phase::Unconfirmed(message) => body
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
            body.id("argo-review")
                .test_support()
                .aria_label(target.clone())
                .p(px(12.))
                .border_1()
                .border_color(Tokens::border())
                .child(ui::sm("Review Argo CD operation"))
                .child(ui::xs(target))
                .child(ui::xs(format!(
                    "Revision: {}. History: {}. Prune: {}. Dry run: {}. Force: {}.",
                    request.revision.as_deref().unwrap_or("Application default"),
                    request
                        .history_id
                        .map_or_else(|| "none".into(), |id| id.to_string()),
                    flag(request.prune),
                    flag(request.dry_run),
                    flag(request.force)
                )))
                .child(
                    button(Variant::Default, 24., false)
                        .id("argo-review-confirm")
                        .test_support()
                        .child("Confirm Argo CD operation")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_argo_review(window, cx)),
                        ),
                )
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-review-cancel")
                        .test_support()
                        .child("Cancel review")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_argo_review(cx))),
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
