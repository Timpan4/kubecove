use super::connected::{ArgoConnectionStore, ConnectedArgo};
use crate::models::AppError;

pub(crate) fn scoped_connection(
    store: &ArgoConnectionStore,
    id: &str,
    cluster_context: &str,
    workspace_id: Option<&str>,
) -> Result<ConnectedArgo, AppError> {
    let workspace_id = workspace_id
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "workspaceId required for connected Argo CD",
                "argoConnection",
            )
        })?;
    let connection = store
        .connections
        .lock()
        .map_err(|_| AppError::new("Argo CD connection state unavailable", "argoConnection"))?
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::new("Argo CD connection not found", "argoConnection"))?;
    if connection.profile.cluster_context.as_deref() != Some(cluster_context)
        || connection.profile.workspace_id.as_deref() != Some(workspace_id)
    {
        return Err(AppError::new(
            "Argo CD connection is outside current workspace scope",
            "argoConnection",
        ));
    }
    Ok(connection)
}
