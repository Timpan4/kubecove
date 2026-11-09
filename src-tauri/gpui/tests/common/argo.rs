//! Fixture requests must match in full, including scope and confirmation.
use kubecove_gpui::backend::{ArgoConnect, ArgoInspection, Read};
use kubecove_lib::models::*;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Deserialize)]
pub struct Argo {
    connections: Vec<Connection>,
    discovery: Vec<ArgoServerCapability>,
    target: Value,
    previews: Vec<Preview>,
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
    active: Arc<Mutex<bool>>,
    /// Commands that "connect-pending" or "disconnect-pending" hold until the next script.
    #[serde(skip)]
    held: Mutex<Vec<oneshot::Sender<()>>>,
    #[serde(skip)]
    reviewed: Mutex<bool>,
    #[serde(skip)]
    pub complete: std::sync::atomic::AtomicBool,
    /// Requests no fixture entry matches. They fail as Argo errors instead of panicking:
    /// the client often calls the backend inside a GPUI update, and a panic there stops
    /// effect flushing for every later scenario in the process.
    #[serde(skip)]
    unexpected: Mutex<Vec<String>>,
}
#[derive(Deserialize)]
struct Preview {
    request: Value,
    result: ArgoTunnelTargetPreview,
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
    /// `configureArgoMock`: sets the script and releases held commands.
    // Only the parity harness configures Argo scripts.
    #[allow(dead_code)]
    pub fn configure(&self, script: &str) {
        *self.script.lock().unwrap() = script.into();
        for release in self.held.lock().unwrap().drain(..) {
            let _ = release.send(());
        }
    }
    /// Requests that matched no fixture entry; the parity harness fails the scenario on any.
    // Only the parity harness checks them.
    #[allow(dead_code)]
    pub fn unexpected_requests(&self) -> Vec<String> {
        self.unexpected.lock().unwrap().clone()
    }
    fn unexpected<T: Send + 'static>(&self, kind: &str, request: &Value) -> Read<T> {
        let message = format!("unexpected {kind}: {request}");
        eprintln!("{message}");
        self.unexpected.lock().unwrap().push(message.clone());
        failed(&message)
    }
    /// Waits for the next script when `script` is the current one.
    fn hold(&self, script: &str) -> Option<oneshot::Receiver<()>> {
        if *self.script.lock().unwrap() != script {
            return None;
        }
        let (sender, receiver) = oneshot::channel();
        self.held.lock().unwrap().push(sender);
        Some(receiver)
    }
    pub fn connect(&self, request: ArgoConnect) -> Read<ArgoConnectionStatus> {
        if *self.script.lock().unwrap() == "target-mismatch" {
            return failed("Fixture tunnel target changed");
        }
        let request = serde_json::to_value(request).unwrap();
        let Some(connection) = self.connections.iter().find(|r| r.request == request) else {
            return self.unexpected("connection", &request);
        };
        let status = connection.result.clone();
        let held = self.hold("connect-pending");
        let active = self.active.clone();
        Box::pin(async move {
            if let Some(held) = held {
                let _ = held.await;
            }
            *active.lock().unwrap() = true;
            Ok(status)
        })
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
        if *self.script.lock().unwrap() == "preview-error" {
            return failed("Fixture tunnel preview unavailable");
        }
        let request = serde_json::to_value(request).unwrap();
        match self.previews.iter().find(|p| p.request == request) {
            Some(preview) => ready(preview.result.clone()),
            None => self.unexpected("preview", &request),
        }
    }
    pub fn disconnect(&self, id: String) -> Read<()> {
        assert!(
            self.connections
                .iter()
                .any(|c| c.result.profile.as_ref().is_some_and(|p| p.id == id))
        );
        let held = self.hold("disconnect-pending");
        let active = self.active.clone();
        Box::pin(async move {
            if let Some(held) = held {
                let _ = held.await;
            }
            *active.lock().unwrap() = false;
            Ok(())
        })
    }
    pub fn inspect(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
        if *self.script.lock().unwrap() == "snapshot-error" {
            return failed("Fixture snapshot unavailable");
        }
        if request.connection_id.is_some() {
            assert!(*self.active.lock().unwrap());
        }
        let request = serde_json::to_value(request).unwrap();
        let Some(inspection) = self.inspections.iter().find(|r| r.request == request) else {
            return self.unexpected("inspection", &request);
        };
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
        let Some(operation) = self.operations.iter().find(|r| r.request == request) else {
            return self.unexpected("operation", &request);
        };
        let review = operation.review.clone();
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
        match self.comparisons.iter().find(|c| c.request == request) {
            Some(comparison) => ready(comparison.result.clone()),
            None => self.unexpected("comparison", &request),
        }
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
