pub mod builtin_kinds;
pub mod cancellation;
mod cluster_access;
mod contexts;
pub mod diagnostics;
pub mod dynamic_projection;
mod finite_read;
pub mod flux_catalog;
pub mod helpers;
pub mod kubeconfig;
pub mod list_projection;
pub mod live_store;

pub use cancellation::{cancel_backend_requests, BackendCancellationRegistry};
pub use cluster_access::{ClientIntent, ClusterAccess};
#[cfg(feature = "test-support")]
pub use cluster_access::{FixedClusterAccess, FIXED_SOURCE_KEY};
pub use contexts::list_kube_contexts;
pub use diagnostics::{
    clear_backend_diagnostics, diagnostic_field, get_backend_cache_diagnostics,
    get_backend_diagnostics, record_backend_cancelled, record_backend_error,
    record_backend_success, set_backend_diagnostics_enabled,
};
pub use finite_read::FiniteRead;
pub use helpers::forwarded_bearer::enable_forwarded_bearer;
pub use helpers::session_scope::SessionScope;
#[cfg(feature = "desktop")]
pub use kubeconfig::pick_kubeconfig_paths;
pub use kubeconfig::{
    add_kubeconfig_paths, get_kubeconfig_sources, init_kubeconfig_settings_path,
    remove_kubeconfig_path, reorder_kubeconfig_paths, set_kubeconfig_env_var,
    set_show_kubeconfig_source_labels, DEFAULT_KUBECONFIG_ENV_VAR,
};
pub use live_store::ClusterLiveStore;
