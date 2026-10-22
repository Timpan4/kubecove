use kubecove_lib::{
    commands::{
        cancel_workspace_requests, AppUsageMonitor, ArgoConnectionStore,
        BackendCancellationRegistry, ClusterLiveStore, PodExecRegistry, PortForwardRegistry,
        SessionScope, StreamRegistry,
    },
    models::AppError,
    runtime::State,
};
use std::{future::Future, sync::Arc};

pub struct BackendState {
    pub live_store: ClusterLiveStore,
    pub cancellation: BackendCancellationRegistry,
    pub streams: StreamRegistry,
    pub port_forwards: PortForwardRegistry,
    pub pod_exec: PodExecRegistry,
    pub usage: AppUsageMonitor,
    pub argo: ArgoConnectionStore,
    pub scope: Arc<SessionScope>,
}

impl Default for BackendState {
    fn default() -> Self {
        Self::new(SessionScope::new(None))
    }
}

impl BackendState {
    pub fn new(scope: Arc<SessionScope>) -> Self {
        // The Argo CD review store takes its secure store when built, so it
        // is built in the scope that namespaces that store.
        let argo = Arc::clone(&scope).run_sync(ArgoConnectionStore::default);
        Self {
            live_store: ClusterLiveStore::default(),
            cancellation: BackendCancellationRegistry::default(),
            streams: StreamRegistry::default(),
            port_forwards: PortForwardRegistry::default(),
            pod_exec: PodExecRegistry::default(),
            usage: AppUsageMonitor::default(),
            argo,
            scope,
        }
    }

    /// Runs `future` in this session's scope. Everything that reaches the
    /// backend library on behalf of a session goes through here.
    pub async fn scoped<F: Future>(&self, future: F) -> F::Output {
        Arc::clone(&self.scope).run(future).await
    }

    pub async fn cleanup(&self) -> Result<(), AppError> {
        self.streams.stop_all();
        self.pod_exec.stop_all();
        self.port_forwards.stop_all();
        self.scoped(cancel_workspace_requests(
            State::new(&self.cancellation),
            State::new(&self.live_store),
            State::new(&self.argo),
        ))
        .await?;
        Ok(())
    }
}

// Command dispatch reaches session state by the type a command declares.
macro_rules! managed_state {
    ($($field:ident: $state:ty),+ $(,)?) => {
        $(impl AsRef<$state> for BackendState {
            fn as_ref(&self) -> &$state {
                &self.$field
            }
        })+
    };
}

managed_state! {
    live_store: ClusterLiveStore,
    cancellation: BackendCancellationRegistry,
    streams: StreamRegistry,
    port_forwards: PortForwardRegistry,
    pod_exec: PodExecRegistry,
    usage: AppUsageMonitor,
    argo: ArgoConnectionStore,
}
