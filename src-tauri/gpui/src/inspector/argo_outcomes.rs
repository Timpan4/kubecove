//! Accepted submissions remain owned by the workspace after their detail closes.
use super::{Inspector, detail::ResourceDetail};
use crate::{backend::ArgoInspection, ui};
use gpui_kit::{Context, Div, Task, TestSupportExt, div, prelude::*};
use kubecove_lib::models::{ArgoApplicationInspector, ArgoApplicationRef, ArgoOperationRequest};
use std::{rc::Rc, time::Duration};

pub(super) struct Outcome {
    pub busy: bool,
    pub message: String,
    identity: Rc<()>,
    _task: Task<()>,
}
fn key(app: &ArgoApplicationRef) -> String {
    serde_json::to_string(&(&app.context, &app.workspace_id, &app.namespace, &app.name))
        .expect("Application identity")
}
fn text<'a>(v: &'a Option<serde_json::Value>, field: &str) -> Option<&'a str> {
    v.as_ref()?.get(field)?.as_str()
}
impl Inspector {
    pub(super) fn argo_operation_busy(&self, app: &ArgoApplicationRef) -> bool {
        self.argo_outcomes.get(&key(app)).is_some_and(|o| o.busy)
    }
    pub(super) fn argo_outcome_panel(&self, detail: &ResourceDetail) -> Option<Div> {
        let mut panel = None;
        if let Some(snapshot) = detail.argo.as_ref().and_then(|s| s.inspector.as_ref())
            && let Some(outcome) = self.argo_outcomes.get(&key(&snapshot.application))
        {
            panel = Some(
                div().child(
                    ui::xs(outcome.message.clone())
                        .id("argo-outcome")
                        .test_support()
                        .aria_label(outcome.message.clone()),
                ),
            );
        }
        panel
    }
    pub(super) fn observe_argo_operation(
        &mut self,
        request: ArgoOperationRequest,
        baseline: ArgoApplicationInspector,
        cx: &mut Context<Self>,
    ) {
        let id = key(&request.application);
        let identity = Rc::new(());
        let current = identity.clone();
        let job_id = id.clone();
        let backend = self.backend.clone();
        let task = cx.spawn(async move |this, cx| {
            let mut saw_pending = false;
            loop {
                let result = backend
                    .inspect_argo(ArgoInspection {
                        application: request.application.clone(),
                        connection_id: request.connection_id.clone(),
                        source: request.kubeconfig_env_var.clone(),
                    })
                    .await;
                let keep = this.update(cx, |this, cx| {
                    let Some(job) = this.argo_outcomes.get_mut(&job_id)
                        .filter(|j| Rc::ptr_eq(&j.identity, &current))
                    else {
                        return false;
                    };
                    match result {
                        Err(error) => {
                            job.message = format!(
                                "Operation accepted; current status unavailable. Retrying observation. {}",
                                error.message
                            );
                        }
                        Ok(snapshot) => {
                            if baseline.application.uid.is_some()
                                && baseline.application.uid != snapshot.application.uid
                            {
                                job.busy = false;
                                job.message = "Application was replaced. The previous operation's outcome is unknown.".into();
                            } else {
                                let refresh = matches!(request.action.as_str(), "refresh" | "hardRefresh");
                                let phase = text(&snapshot.operation_state, "phase");
                                let active = snapshot.operation_requested
                                    || matches!(phase, Some("Running" | "Terminating"));
                                let pending = if refresh {
                                    snapshot.refresh_requested.is_some()
                                } else {
                                    active
                                };
                                let changed = if refresh {
                                    text(&snapshot.status, "reconciledAt").is_some()
                                        && text(&snapshot.status, "reconciledAt") != text(&baseline.status, "reconciledAt")
                                } else {
                                    text(&snapshot.operation_state, "startedAt") != text(&baseline.operation_state, "startedAt")
                                };
                                if !pending && (saw_pending || changed) {
                                    if refresh || phase == Some("Succeeded") {
                                        job.busy = false;
                                        job.message = "Argo CD confirmed completion".into();
                                    } else if matches!(phase, Some("Failed" | "Error")) {
                                        job.busy = false;
                                        job.message = format!(
                                            "Argo CD reported failure. {}",
                                            text(&snapshot.operation_state, "message").unwrap_or_default()
                                        );
                                    }
                                }
                                saw_pending |= pending;
                            }
                        }
                    }
                    cx.notify();
                    job.busy
                }).unwrap_or(false);
                if !keep {
                    break;
                }
                // Same recovery interval as resource-watch.ts and the web operation tracker.
                cx.background_executor().timer(Duration::from_millis(15_000)).await;
            }
        });
        self.argo_outcomes.insert(
            id,
            Outcome {
                busy: true,
                message: "Accepted; waiting for Argo CD to complete".into(),
                identity,
                _task: task,
            },
        );
    }
}
