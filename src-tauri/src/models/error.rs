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
        Self::new(message, "cluster")
    }

    pub fn cancelled() -> Self {
        Self::new("request cancelled", "cancelled")
    }
}

impl From<kube::Error> for AppError {
    fn from(e: kube::Error) -> Self {
        Self::kube(e.to_string())
    }
}
