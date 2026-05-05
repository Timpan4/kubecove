use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterContext {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceSummary {
    pub name: String,
    pub age: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSummary {
    pub kind: String,
    pub cluster: String,
    pub name: String,
    pub namespace: Option<String>,
    pub age: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDetails {
    pub kind: String,
    pub cluster: String,
    pub name: String,
    pub namespace: Option<String>,
    pub yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDetailsFull {
    pub summary: ResourceSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
    pub status: Option<serde_json::Value>,
}

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
        Self::new(message, "cluster")
    }
}

impl From<kube::Error> for AppError {
    fn from(e: kube::Error) -> Self {
        Self::kube(e.to_string())
    }
}

