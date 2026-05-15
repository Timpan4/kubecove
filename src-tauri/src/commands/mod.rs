mod argo;
mod contexts;
mod discovery;
mod events;
mod helpers;
mod namespaces;
mod resources;

pub use argo::{
    detect_argocd, get_argocd_application_details, get_argocd_appproject_details,
    get_argocd_appset_details, list_argocd_applications, list_argocd_appprojects,
    list_argocd_appsets,
};
pub use contexts::{get_cluster_contexts, list_kube_contexts};
pub use discovery::{list_resource_kinds, resource_kinds_from};
pub use events::{list_resource_events, resource_events_from};
pub use namespaces::{list_namespaces, namespaces_summary_from};
pub use resources::{
    get_resource_details, get_resource_yaml, list_resources, resource_details_from,
    resource_yaml_from, resources_summary_from,
};
