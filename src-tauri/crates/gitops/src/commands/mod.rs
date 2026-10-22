pub mod argo;
pub mod flux;
mod gitops_crd;
pub mod helm;

pub(crate) use kubecove_kube::commands::{
    diagnostic_field, helpers, BackendCancellationRegistry, ClusterAccess, FiniteRead,
};
#[cfg(test)]
pub(crate) use kubecove_kube::commands::{FixedClusterAccess, FIXED_SOURCE_KEY};
pub(crate) use kubecove_live::commands::sessions;

#[cfg(not(feature = "desktop"))]
pub use argo::{
    active_credential_store_description, initialize_server_credential_store,
    CredentialStoreDescription,
};
pub use argo::{
    connect_argo_server, detect_argocd, disconnect_argo_server, discover_argo_servers,
    forget_argo_credential, get_argo_application_inspector, get_argo_connection_status,
    get_argo_resource_comparison, get_argocd_application_details, get_argocd_appproject_details,
    get_argocd_appset_details, list_argocd_applications, list_argocd_appprojects,
    list_argocd_appsets, preflight_argo_operation, run_argo_operation, ArgoConnectionStore,
};
pub use flux::{detect_flux, get_flux_resource_details, list_flux_resources};
pub use helm::{get_helm_release_details, get_helm_release_reconciliation, list_helm_releases};
