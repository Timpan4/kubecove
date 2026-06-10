pub(crate) mod client_cache;
mod metadata;
mod serialization;
mod time;

pub(crate) use metadata::{
    base_resource_summary, extract_argo_app, extract_git_ops_owner, extract_helm_release,
    extract_owner_ref, extract_owner_ref_summary, fmt_ready,
};
pub(crate) use serialization::{
    fetch_and_serialize, fetch_and_serialize_cluster, fetch_and_serialize_cluster_with_encoding,
    fetch_and_serialize_with_encoding, redact_secret, serialize_resource_document,
};
pub(crate) use time::{k8s_creation_timestamp_to_rfc3339, k8s_timestamp_to_datetime, resource_age};

use kube::api::ListParams;

pub(crate) fn list_params() -> ListParams {
    ListParams::default()
}
