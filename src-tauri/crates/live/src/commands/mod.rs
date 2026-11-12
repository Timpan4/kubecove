mod live_sessions;
pub mod pod_exec;
pub mod sessions;
pub mod streams;

#[cfg(test)]
pub(crate) use kubecove_kube::commands::{ClientIntent, FixedClusterAccess};
pub(crate) use kubecove_kube::commands::{
    ClusterAccess, ClusterLiveStore, SessionScope, WatchOrigin, builtin_kinds, helpers,
};

pub use live_sessions::{LiveSessionScope, stop_live_sessions_outside_scope};
pub use pod_exec::{
    PodExecRegistry, list_pod_exec_sessions, resize_pod_exec_terminal, start_pod_exec_session,
    stop_pod_exec_session, write_pod_exec_stdin,
};
pub use sessions::{
    PortForwardRegistry, list_port_forwards, start_pod_port_forward, stop_port_forward,
};
pub use streams::{
    StreamRegistry, read_pod_logs, refresh_resource_cache, resource_watch_start_version,
    start_aggregated_log_stream, start_pod_log_stream, start_resource_event_watch,
    start_resource_watch, stop_stream,
};
