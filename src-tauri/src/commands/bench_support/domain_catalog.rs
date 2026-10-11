use serde::Serialize;

#[path = "domain_argo.rs"]
pub mod domain_argo;
#[path = "domain_flux.rs"]
pub mod domain_flux;
#[path = "domain_helm.rs"]
pub mod domain_helm;
#[path = "domain_rbac.rs"]
pub mod domain_rbac;
#[path = "domain_sessions.rs"]
pub mod domain_sessions;
#[path = "domain_streams.rs"]
pub mod domain_streams;
#[path = "domain_usage.rs"]
pub mod domain_usage;

pub fn encode_complete_response<T: Serialize>(response: &T) -> String {
    serde_json::to_string(response).expect("public benchmark response serializes")
}
