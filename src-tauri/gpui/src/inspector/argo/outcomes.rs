//! Accepted submissions remain owned by the workspace after their detail closes.
use super::super::{Inspector, controls, detail::ResourceDetail, request::InFlight};
use crate::targets::TestSupportExt;
use crate::{backend::ArgoInspection, ui};
use gpui_kit::{Context, Div, div, prelude::*};
use kubecove_lib::models::{
    AppError, ArgoApplicationInspector, ArgoApplicationRef, ArgoOperationRequest,
};
use std::time::Duration;

/// One accepted operation, observed until Argo CD reports its end. Replacing or
/// dropping the outcome cancels its observation.
pub(super) struct Outcome {
    observation: Observation,
    _observer: InFlight,
    request: ArgoOperationRequest,
    baseline: ArgoApplicationInspector,
}
/// What the observer knows about an accepted operation.
enum Observation {
    /// Argo CD has not finished it. `read_error` holds the last failed read, which
    /// offers a manual retry beside the automatic one.
    Waiting {
        saw_pending: bool,
        read_error: Option<String>,
    },
    Finished(String),
}
impl Observation {
    fn message(&self) -> String {
        match self {
            Observation::Waiting {
                read_error: Some(error),
                ..
            } => format!(
                "Operation accepted; current status unavailable. Retrying observation. {error}"
            ),
            Observation::Waiting { .. } => "Accepted; waiting for Argo CD to complete".into(),
            Observation::Finished(message) => message.clone(),
        }
    }
    fn waiting(&self) -> bool {
        matches!(self, Observation::Waiting { .. })
    }
    fn read_failed(&self) -> bool {
        matches!(
            self,
            Observation::Waiting {
                read_error: Some(_),
                ..
            }
        )
    }
}
/// One observation read. `saw_pending` records whether Argo CD has reported the
/// operation as pending since it was accepted.
fn observe(
    saw_pending: bool,
    request: &ArgoOperationRequest,
    baseline: &ArgoApplicationInspector,
    read: Result<ArgoApplicationInspector, AppError>,
) -> Observation {
    let snapshot = match read {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Observation::Waiting {
                saw_pending,
                read_error: Some(error.message),
            };
        }
    };
    if baseline.application.uid.is_some() && baseline.application.uid != snapshot.application.uid {
        return Observation::Finished(
            "Application was replaced. The previous operation's outcome is unknown.".into(),
        );
    }
    let refresh = matches!(request.action.as_str(), "refresh" | "hardRefresh");
    let phase = text(&snapshot.operation_state, "phase");
    let active = snapshot.operation_requested || matches!(phase, Some("Running" | "Terminating"));
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
            return Observation::Finished("Argo CD confirmed completion".into());
        }
        if matches!(phase, Some("Failed" | "Error")) {
            return Observation::Finished(format!(
                "Argo CD reported failure. {}",
                text(&snapshot.operation_state, "message").unwrap_or_default()
            ));
        }
    }
    // A successful read replaces an earlier read error, as the web tracker does on each read.
    Observation::Waiting {
        saw_pending: saw_pending || pending,
        read_error: None,
    }
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
        self.argo
            .outcomes
            .get(&key(app))
            .is_some_and(|o| o.observation.waiting())
    }
    pub(super) fn argo_outcome_panel(
        &self,
        detail: &ResourceDetail,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let snapshot = detail.argo.as_ref()?.inspector.as_ref()?;
        let outcome = self.argo.outcomes.get(&key(&snapshot.application))?;
        let message = outcome.observation.message();
        Some(
            div()
                .child(
                    ui::xs(message.clone())
                        .id("argo-outcome")
                        .test_support()
                        .aria_label(message),
                )
                .when(outcome.observation.read_failed(), |panel| {
                    let id = key(&snapshot.application);
                    panel.child(
                        controls::button(controls::Variant::Outline, 24., false)
                            .id("argo-state-retry")
                            .test_support()
                            .child("Retry state refresh")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let Some(job) = this
                                    .argo
                                    .outcomes
                                    .get(&id)
                                    .filter(|j| j.observation.read_failed())
                                else {
                                    return;
                                };
                                this.observe_argo_operation(
                                    job.request.clone(),
                                    job.baseline.clone(),
                                    cx,
                                );
                            })),
                    )
                }),
        )
    }
    pub(super) fn observe_argo_operation(
        &mut self,
        request: ArgoOperationRequest,
        baseline: ArgoApplicationInspector,
        cx: &mut Context<Self>,
    ) {
        let id = key(&request.application);
        let job_id = id.clone();
        let backend = self.backend.clone();
        let saved_request = request.clone();
        let saved_baseline = baseline.clone();
        // A retry keeps what the replaced observation already saw.
        let saw_pending = matches!(
            self.argo.outcomes.get(&id).map(|job| &job.observation),
            Some(Observation::Waiting {
                saw_pending: true,
                ..
            })
        );
        // Inserting this outcome drops the one it replaces, which cancels that
        // observer, so the entry under `job_id` is always this observer's own.
        let observer = cx.spawn(async move |this, cx| {
            loop {
                let read = backend
                    .inspect_argo(ArgoInspection {
                        application: request.application.clone(),
                        connection_id: request.connection_id.clone(),
                        source: request.kubeconfig_env_var.clone(),
                    })
                    .await;
                let keep = this
                    .update(cx, |this, cx| {
                        let Some(job) = this.argo.outcomes.get_mut(&job_id) else {
                            return false;
                        };
                        let Observation::Waiting { saw_pending, .. } = job.observation else {
                            return false;
                        };
                        job.observation = observe(saw_pending, &request, &baseline, read);
                        cx.notify();
                        job.observation.waiting()
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
                // Same recovery interval as resource-watch.ts and the web operation tracker.
                cx.background_executor()
                    .timer(Duration::from_millis(15_000))
                    .await;
            }
        });
        self.argo.outcomes.insert(
            id,
            Outcome {
                observation: Observation::Waiting {
                    saw_pending,
                    read_error: None,
                },
                _observer: observer.into(),
                request: saved_request,
                baseline: saved_baseline,
            },
        );
    }
}
