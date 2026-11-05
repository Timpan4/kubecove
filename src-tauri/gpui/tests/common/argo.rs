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
    connected: ArgoConnectionStatus,
    inspections: Vec<Inspection>,
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
        assert_eq!(
            Some(id.as_str()),
            self.connected.profile.as_ref().map(|p| p.id.as_str())
        );
        *self.active.lock().unwrap() = false;
        ready(())
    }
    pub fn inspect(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
        if request.connection_id.is_some() {
            assert!(*self.active.lock().unwrap());
        }
        let request = serde_json::to_value(request).unwrap();
        let mut result = self
            .inspections
            .iter()
            .find(|r| r.request == request)
            .unwrap_or_else(|| panic!("unexpected inspection: {request}"))
            .result
            .clone();
        if self.complete.load(std::sync::atomic::Ordering::SeqCst) {
            result.operation_state = Some(
                serde_json::json!({"phase":"Succeeded","startedAt":"2026-11-05T00:00:01Z","message":"Fixture operation finished"}),
            );
        }
        ready(result)
    }
    pub fn preflight(&self, request: ArgoOperationRequest) -> Read<ArgoOperationPreflight> {
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
