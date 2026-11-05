//! Fixture requests must match in full, including scope and confirmation.
use kubecove_gpui::backend::{ArgoConnect, ArgoInspection, Read};
use kubecove_lib::models::*;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;

#[derive(Deserialize)]
pub struct Argo {
    connections: Vec<Connection>,
    discovery: Vec<ArgoServerCapability>,
    target: Value,
    preview: ArgoTunnelTargetPreview,
    inspections: Vec<Inspection>,
    comparisons: Vec<Comparison>,
    #[serde(skip)]
    pub script: Mutex<String>,
    #[serde(skip)]
    pub inventory_mode: Mutex<String>,
    operations: Vec<Operation>,
    confirmation: Value,
    result: ArgoOperationResult,
    #[serde(skip)]
    active: Mutex<bool>,
    #[serde(skip)]
    reviewed: Mutex<bool>,
    #[serde(skip)]
    pub complete: std::sync::atomic::AtomicBool,
}
#[derive(Deserialize)]
struct Connection {
    request: Value,
    result: ArgoConnectionStatus,
}
#[derive(Deserialize)]
struct Inspection {
    request: Value,
    result: ArgoApplicationInspector,
    full: ArgoApplicationInspector,
}
#[derive(Deserialize)]
struct Comparison {
    request: Value,
    result: ArgoResourceComparison,
}
fn failed<T: Send + 'static>(message: &str) -> Read<T> {
    let error = AppError::new(message, AppErrorKind::ArgoConnection);
    Box::pin(async move { Err(error) })
}
#[derive(Deserialize)]
struct Operation {
    request: Value,
    review: ArgoOperationPreflight,
}
fn ready<T: Send + 'static>(value: T) -> Read<T> {
    Box::pin(async move { Ok(value) })
}
impl Argo {
    pub fn connect(&self, request: ArgoConnect) -> Read<ArgoConnectionStatus> {
        if *self.script.lock().unwrap() == "target-mismatch" {
            return failed("Fixture tunnel target changed");
        }
        let request = serde_json::to_value(request).unwrap();
        let status = self
            .connections
            .iter()
            .find(|r| r.request == request)
            .unwrap_or_else(|| panic!("unexpected connection: {request}"));
        *self.active.lock().unwrap() = true;
        ready(status.result.clone())
    }
    pub fn discover(
        &self,
        request: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<Vec<ArgoServerCapability>> {
        if *self.script.lock().unwrap() == "discovery-error" {
            return failed("Fixture discovery unavailable");
        }
        assert_eq!(request.context, "mock-dev");
        assert_eq!(
            serde_json::to_value(request.source).unwrap(),
            self.target["source"]
        );
        assert!(request.endpoint.is_none());
        ready(self.discovery.clone())
    }
    pub fn preview(
        &self,
        request: kubecove_gpui::backend::ArgoTarget,
    ) -> Read<ArgoTunnelTargetPreview> {
        assert_eq!(serde_json::to_value(request).unwrap(), self.target);
        ready(self.preview.clone())
    }
    pub fn disconnect(&self, id: String) -> Read<()> {
        assert!(
            self.connections
                .iter()
                .any(|c| c.result.profile.as_ref().is_some_and(|p| p.id == id))
        );
        *self.active.lock().unwrap() = false;
        ready(())
    }
    pub fn inspect(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
        if *self.script.lock().unwrap() == "snapshot-error" {
            return failed("Fixture snapshot unavailable");
        }
        if request.connection_id.is_some() {
            assert!(*self.active.lock().unwrap());
        }
        let request = serde_json::to_value(request).unwrap();
        let inspection = self
            .inspections
            .iter()
            .find(|r| r.request == request)
            .unwrap_or_else(|| panic!("unexpected inspection: {request}"));
        let mut result = if *self.script.lock().unwrap() == "full-diff" {
            inspection.full.clone()
        } else {
            inspection.result.clone()
        };
        if *self.script.lock().unwrap() == "lazy" {
            result.comparisons.clear();
            for resource in &mut result.resources {
                resource.target_state = None;
                resource.live_state = None;
            }
        }
        if self.complete.load(std::sync::atomic::Ordering::SeqCst) {
            result.operation_state = Some(
                serde_json::json!({"phase":"Succeeded","startedAt":"2026-11-05T00:00:01Z","message":"Fixture operation finished"}),
            );
        }
        ready(result)
    }
    pub fn preflight(&self, request: ArgoOperationRequest) -> Read<ArgoOperationPreflight> {
        if *self.script.lock().unwrap() == "operation-error" {
            return failed("Fixture preflight unavailable");
        }
        let request = serde_json::to_value(request).unwrap();
        let review = self
            .operations
            .iter()
            .find(|r| r.request == request)
            .unwrap_or_else(|| panic!("unexpected operation: {request}"))
            .review
            .clone();
        *self.reviewed.lock().unwrap() = review.allowed;
        ready(review)
    }
    pub fn forget(&self, profile: ArgoConnectionProfile) -> Read<()> {
        assert!(
            self.connections
                .iter()
                .any(|c| c
                    .result
                    .profile
                    .as_ref()
                    .is_some_and(|p| serde_json::to_value(p).unwrap()
                        == serde_json::to_value(&profile).unwrap()))
        );
        assert!(
            !*self.active.lock().unwrap(),
            "disconnect before forgetting"
        );
        ready(())
    }
    pub fn compare(
        &self,
        inspection: ArgoInspection,
        resource: ArgoManagedResource,
    ) -> Read<ArgoResourceComparison> {
        let request = serde_json::json!({"inspection": inspection, "resource": resource});
        let result = self
            .comparisons
            .iter()
            .find(|c| c.request == request)
            .unwrap_or_else(|| panic!("unexpected comparison: {request}"));
        ready(result.result.clone())
    }
    pub fn run(&self, confirmation: ArgoOperationConfirmation) -> Read<ArgoOperationResult> {
        assert!(
            std::mem::take(&mut *self.reviewed.lock().unwrap()),
            "no unconsumed review"
        );
        assert_eq!(
            serde_json::to_value(confirmation).unwrap(),
            self.confirmation
        );
        ready(self.result.clone())
    }
}
