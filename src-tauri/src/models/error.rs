use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub message: String,
    pub kind: String,
}

impl AppError {
    pub fn new(message: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: kind.into(),
        }
    }

    pub fn kube(message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = kube_message_kind(&message).unwrap_or("cluster");
        Self::new(message, kind)
    }

    pub fn cancelled() -> Self {
        Self::new("request cancelled", "cancelled")
    }
}

impl From<kube::Error> for AppError {
    fn from(e: kube::Error) -> Self {
        let message = e.to_string();
        let kind = kube_error_kind(&e)
            .or_else(|| kube_message_kind(&message))
            .unwrap_or("cluster");
        Self::new(message, kind)
    }
}

fn kube_error_kind(error: &kube::Error) -> Option<&'static str> {
    match error {
        kube::Error::Api(status) => Some(status_kind(status)),
        _ => None,
    }
}

fn status_kind(status: &kube::core::Status) -> &'static str {
    if status.code == 403 || status.reason == "Forbidden" {
        return "forbidden";
    }
    if status.code == 404 || status.reason == "NotFound" {
        return "notFound";
    }
    if status.code == 422 || status.reason == "Invalid" {
        if is_admission_status(status) {
            return "admissionDenied";
        }
        return "invalidResource";
    }
    "cluster"
}

fn is_admission_status(status: &kube::core::Status) -> bool {
    let message = status.message.to_ascii_lowercase();
    message.contains("admission webhook")
        || message.contains("denied the request")
        || message.contains("violates")
        || message.contains("podsecurity")
}

fn kube_message_kind(message: &str) -> Option<&'static str> {
    let message = message.to_ascii_lowercase();
    if message.contains("fieldmanagerconflict")
        || message.contains("field manager conflict")
        || message.contains("apply failed with conflicts")
    {
        return Some("fieldManagerConflict");
    }
    if message.contains("pod updates may not change fields")
        || message.contains("field is immutable")
        || message.contains("fieldvalueforbidden")
    {
        return Some("immutableField");
    }
    if message.contains("admission webhook")
        || message.contains("denied the request")
        || message.contains("podsecurity")
        || message.contains("violates")
    {
        return Some("admissionDenied");
    }
    if message.contains("forbidden") || message.contains("status 403") {
        return Some("forbidden");
    }
    if message.contains("kubeconfig")
        || message.contains("failed to infer config")
        || message.contains("no configuration has been provided")
        || message.contains("context ") && message.contains(" not found")
    {
        return Some("kubeconfig");
    }
    if message.contains("no ready pod")
        || message.contains("no matching pod")
        || message.contains("selector") && message.contains("matched no pod")
        || message.contains("container") && message.contains("not found")
        || message.contains("address already in use")
        || message.contains("port-forward")
    {
        return Some("liveSessionTargetUnavailable");
    }
    if message.contains("discovery")
        || message.contains("customresourcedefinition")
        || message.contains("metrics.k8s.io")
        || message.contains("metrics api")
    {
        return Some("providerDiscoveryUnavailable");
    }
    if message.contains("serialize")
        || message.contains("serialization")
        || message.contains("deserialize")
        || message.contains("json")
        || message.contains("yaml")
    {
        return Some("serialization");
    }
    if message.contains("not found") || message.contains("status 404") {
        return Some("notFound");
    }
    if message.contains("connection refused")
        || message.contains("connection reset")
        || message.contains("timed out")
        || message.contains("timeout")
        || message.contains("unreachable")
        || message.contains("i/o timeout")
        || message.contains("dns")
    {
        return Some("network");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::core::Status;

    fn status(code: u16, reason: &str, message: &str) -> kube::Error {
        kube::Error::Api(Box::new(Status {
            code,
            reason: reason.to_string(),
            message: message.to_string(),
            ..Default::default()
        }))
    }

    #[test]
    fn classifies_forbidden_status_errors() {
        let err = AppError::from(status(403, "Forbidden", "pods is forbidden"));

        assert_eq!(err.kind, "forbidden");
    }

    #[test]
    fn classifies_not_found_status_errors() {
        let err = AppError::from(status(404, "NotFound", "pods \"gone\" not found"));

        assert_eq!(err.kind, "notFound");
    }

    #[test]
    fn classifies_invalid_status_errors() {
        let err = AppError::from(status(422, "Invalid", "Pod \"api\" is invalid"));

        assert_eq!(err.kind, "invalidResource");
    }

    #[test]
    fn classifies_admission_status_errors() {
        let err = AppError::from(status(
            422,
            "Invalid",
            "admission webhook \"policy\" denied the request",
        ));

        assert_eq!(err.kind, "admissionDenied");
    }

    #[test]
    fn classifies_kubeconfig_message_errors() {
        let err = AppError::kube("failed to infer config: kubeconfig missing");

        assert_eq!(err.kind, "kubeconfig");

        let err = AppError::kube("context admin@kind not found");

        assert_eq!(err.kind, "kubeconfig");
    }

    #[test]
    fn classifies_live_session_target_message_errors() {
        let err = AppError::kube("no ready Pods matched this Service selector");

        assert_eq!(err.kind, "liveSessionTargetUnavailable");
    }

    #[test]
    fn classifies_provider_discovery_message_errors() {
        let err = AppError::kube("metrics.k8s.io discovery unavailable");

        assert_eq!(err.kind, "providerDiscoveryUnavailable");
    }

    #[test]
    fn classifies_serialization_message_errors() {
        let err = AppError::kube("failed to serialize resource yaml");

        assert_eq!(err.kind, "serialization");
    }
}
