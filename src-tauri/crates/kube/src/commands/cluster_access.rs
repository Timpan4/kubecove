//! Cluster access by intent (ADR 0026). A request resolves one `ClusterAccess`
//! from its session's source policy and its kubeconfig selector, then asks it
//! for clients named by what they are for. The intent picks the ADR 0009 client
//! path and whether a source whose configured paths fail to load may fall back
//! to the default kubeconfig.

mod clients;
#[cfg(feature = "test-support")]
mod fixed;

use super::{
    helpers::session_scope::SessionScope,
    kubeconfig::{cluster_contexts_from_kubeconfig, is_source_key, KubeconfigSource},
};
use crate::models::{AppError, ClusterContext};
#[cfg(feature = "test-support")]
pub use fixed::{FixedClusterAccess, FIXED_SOURCE_KEY};
use kube::Client;
use std::{fmt, sync::Arc};

/// What a request wants a Kubernetes client for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIntent {
    /// A finite read: rotating and cancellable, may fall back to the default kubeconfig.
    Read,
    /// A dry run, preflight or state read before a cluster-changing action:
    /// rotating and cancellable, fails closed.
    Preview,
    /// A confirmed write: not cancelled by workspace rotation, fails closed.
    ConfirmedOperation,
    /// A port-forward, Pod exec or Argo CD Service tunnel: long-lived, fails closed.
    LiveSession,
    /// A view-owned watch or log stream: long-lived, may fall back.
    Stream,
}

/// Which kubeconfig sources a session's requests may select.
#[derive(Clone)]
pub(crate) enum SourcePolicy {
    /// The desktop user's own machine: a selector may name any environment
    /// variable.
    Local,
    /// Every selector resolves to one test adapter.
    #[cfg(feature = "test-support")]
    Fixed(Arc<FixedClusterAccess>),
}

/// One request's resolved Kubernetes access: the source it reads and the
/// clients it hands out by intent. Cloning shares the resolved source.
#[derive(Clone)]
pub struct ClusterAccess {
    source_key: String,
    source_label: Option<String>,
    source: Source,
}

// Shows only the key and label, which never contain paths or credentials.
impl fmt::Debug for ClusterAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClusterAccess")
            .field("source_key", &self.source_key)
            .field("source_label", &self.source_label)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
enum Source {
    Kubeconfig(Arc<KubeconfigSource>),
    #[cfg(feature = "test-support")]
    Fixed(Arc<FixedClusterAccess>),
}

impl ClusterAccess {
    /// Resolves the calling session's access for one request. `selector` is
    /// the request's `kubeconfigEnvVar` argument: an environment variable
    /// name, a source key, or nothing for the configured source.
    pub fn resolve(selector: Option<&str>) -> Result<Self, AppError> {
        match SessionScope::current_or_process().source_policy() {
            SourcePolicy::Local => {
                let env_var = selector.filter(|value| !is_source_key(value));
                Ok(Self::from_kubeconfig(KubeconfigSource::from_persisted(
                    env_var,
                )?))
            }
            #[cfg(feature = "test-support")]
            SourcePolicy::Fixed(fixed) => Ok(fixed.access()),
        }
    }

    pub(crate) fn from_kubeconfig(source: KubeconfigSource) -> Self {
        Self {
            source_key: source.key(),
            source_label: source.show_source_labels().then(|| source.label()),
            source: Source::Kubeconfig(Arc::new(source)),
        }
    }

    /// Identifies the resolved source in caches, live-session scope and Argo
    /// CD connection scope. It never contains paths or credentials.
    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    /// The source's display label, or `None` when source labels are hidden.
    pub fn source_label(&self) -> Option<&str> {
        self.source_label.as_deref()
    }

    /// The contexts the resolved source defines.
    pub fn contexts(&self) -> Result<Vec<ClusterContext>, AppError> {
        match &self.source {
            Source::Kubeconfig(source) => {
                Ok(cluster_contexts_from_kubeconfig(&source.read_kubeconfig()?))
            }
            #[cfg(feature = "test-support")]
            Source::Fixed(_) => Ok(Vec::new()),
        }
    }

    pub async fn read_client(&self, cluster_context: &str) -> Result<Client, AppError> {
        self.client(ClientIntent::Read, cluster_context).await
    }

    pub async fn preview_client(&self, cluster_context: &str) -> Result<Client, AppError> {
        self.client(ClientIntent::Preview, cluster_context).await
    }

    pub async fn confirmed_operation_client(
        &self,
        cluster_context: &str,
    ) -> Result<Client, AppError> {
        self.client(ClientIntent::ConfirmedOperation, cluster_context)
            .await
    }

    pub async fn live_session_client(&self, cluster_context: &str) -> Result<Client, AppError> {
        self.client(ClientIntent::LiveSession, cluster_context)
            .await
    }

    pub async fn stream_client(&self, cluster_context: &str) -> Result<Client, AppError> {
        self.client(ClientIntent::Stream, cluster_context).await
    }

    async fn client(
        &self,
        intent: ClientIntent,
        cluster_context: &str,
    ) -> Result<Client, AppError> {
        match &self.source {
            Source::Kubeconfig(source) => {
                clients::client(source, &self.source_key, intent, cluster_context).await
            }
            #[cfg(feature = "test-support")]
            Source::Fixed(fixed) => Ok(fixed.client(intent)),
        }
    }
}

#[cfg(test)]
mod tests;
