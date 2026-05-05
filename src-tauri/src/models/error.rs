use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("kubeconfig error: {0}")]
    KubeConfig(String),
    #[error("cluster error: {0}")]
    Cluster(String),
    #[error("resource error: {0}")]
    Resource(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::KubeConfig(_) => "kubeConfig",
            AppError::Cluster(_) => "cluster",
            AppError::Resource(_) => "resource",
            AppError::Serialization(_) => "serialization",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}
