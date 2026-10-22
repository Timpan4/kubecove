pub mod discovery;
mod event_projection;
pub mod events;
pub mod incidents;
pub mod metrics;
pub mod namespaces;
mod operations;
pub mod rbac;
mod rbac_review;
mod resource_actions;
pub mod resources;

// Reuse the topology list fan-out budget across inspection commands.
pub(crate) const MAX_LIST_CONCURRENCY: usize = 16;

pub(crate) use kubecove_kube::commands::{
    builtin_kinds, diagnostic_field, diagnostics, helpers, kubeconfig, record_backend_error,
    record_backend_success, BackendCancellationRegistry, ClusterLiveStore,
};

pub use discovery::{
    list_present_custom_resource_kinds, list_resource_kinds, present_custom_resource_kinds_from,
    resource_kinds_from,
};
pub use events::{list_resource_events, resource_events_from};
pub use incidents::{incident_cockpit_from, list_incident_cockpit};
pub use metrics::{list_resource_metrics, resource_metrics_from};
pub use namespaces::{list_namespaces, namespaces_summary_from};
pub use operations::{
    delete_resource, preview_delete_resource, preview_rollout_restart, preview_scale_workload,
    rollout_restart, scale_workload,
};
pub use rbac::{list_rbac_inspection, rbac_inspection_from};
pub use rbac_review::review_rbac_access;
pub use resource_actions::{
    execute_resource_action, get_resource_action_state, preview_resource_action,
};
pub use resources::{
    apply_yaml, dynamic_resource_details_from, dynamic_resources_summary_from,
    get_dynamic_resource_details, get_resource_details, get_resource_yaml, lint_kubernetes_yaml,
    list_deployment_revisions, list_dynamic_resources, list_resource_scope, list_resource_topology,
    list_resources, prepare_yaml_apply, resource_details_from, resource_scope_from,
    resource_topology_from, resource_yaml_from, resources_summary_from, reveal_secret_data_value,
};
