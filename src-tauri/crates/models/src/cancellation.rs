use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelBackendRequestsResult {
    pub cancelled: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelWorkspaceRequestsResult {
    pub cancelled_requests: usize,
    pub cancelled_loads: usize,
    pub client_generation: u64,
}
