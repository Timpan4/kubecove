use super::super::credential_store::{active_store, ARGO_CREDENTIAL_SERVICE};
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoConnectionProfile, ArgoServerEndpoint};
use uuid::Uuid;

fn credential_key(profile: &ArgoConnectionProfile) -> String {
    let scope = format!(
        "{}:{}",
        profile.cluster_context.clone().unwrap_or_default(),
        profile.workspace_id.clone().unwrap_or_default()
    );
    match &profile.endpoint {
        // Preserve existing credential lookups for migrated legacy profiles.
        ArgoServerEndpoint::ExternalHttps { .. } => {
            format!("{}:{}:{}", profile.id, profile.url, scope)
        }
        ArgoServerEndpoint::ServiceTunnel {
            namespace,
            service_name,
            service_port,
            scheme,
            root_path,
            tls_server_name,
        } => format!(
            "{}:serviceTunnel:{}:{}:{}:{}:{}:{}:{}:{}",
            profile.id,
            namespace,
            service_name,
            service_port,
            scheme,
            root_path.as_deref().unwrap_or("/"),
            tls_server_name.as_deref().unwrap_or_default(),
            scope,
            profile.kubeconfig_source_key.as_deref().unwrap_or_default(),
        ),
    }
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct StoredCredential {
    pub(super) token: String,
    generation: String,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredCredential")
            .field("token", &"[REDACTED]")
            .field("generation", &self.generation)
            .finish()
    }
}

pub(super) trait CredentialStore {
    fn read(&self, key: &str) -> Result<Option<String>, AppError>;
    fn write(&self, key: &str, value: &str) -> Result<(), AppError>;
    fn delete(&self, key: &str) -> Result<(), AppError>;
}

pub(super) struct ActiveCredentialStore;
impl CredentialStore for ActiveCredentialStore {
    fn read(&self, key: &str) -> Result<Option<String>, AppError> {
        active_store().read(ARGO_CREDENTIAL_SERVICE, key)
    }
    fn write(&self, key: &str, value: &str) -> Result<(), AppError> {
        active_store().write(ARGO_CREDENTIAL_SERVICE, key, value)
    }
    fn delete(&self, key: &str) -> Result<(), AppError> {
        active_store().delete(ARGO_CREDENTIAL_SERVICE, key)
    }
}

#[cfg(test)]
fn credential_error(error: keyring::Error) -> AppError {
    AppError::new(
        "native credential storage unavailable",
        AppErrorKind::CredentialUnavailable,
    )
    .with_source(error)
}

pub(super) fn load_credential(
    store: &dyn CredentialStore,
    profile: &ArgoConnectionProfile,
) -> Result<Option<StoredCredential>, AppError> {
    let value = store.read(&credential_key(profile))?;
    let Some(value) = value else { return Ok(None) };
    let record = match serde_json::from_str::<StoredCredential>(&value) {
        Ok(record) => record,
        Err(_) if !value.is_empty() => {
            let record = StoredCredential {
                token: value,
                generation: Uuid::new_v4().to_string(),
            };
            save_credential(store, profile, &record.token, record.generation.clone())?;
            record
        }
        Err(_) => return Ok(None),
    };
    Ok((!record.token.is_empty()).then_some(record))
}

pub(super) fn save_credential(
    store: &dyn CredentialStore,
    profile: &ArgoConnectionProfile,
    token: &str,
    generation: String,
) -> Result<(), AppError> {
    let value = serde_json::to_string(&StoredCredential {
        token: token.into(),
        generation,
    })
    .map_err(|error| {
        AppError::new(
            "native credential storage unavailable",
            AppErrorKind::CredentialUnavailable,
        )
        .with_source(error)
    })?;
    store.write(&credential_key(profile), &value)
}

pub(super) fn connection_generation(
    stored: Option<&StoredCredential>,
    submitted_token: bool,
    remember_credential: bool,
    insecure_tls: bool,
    custom_ca_pem: Option<&[u8]>,
) -> String {
    if submitted_token || insecure_tls || custom_ca_pem.is_some() {
        return Uuid::new_v4().to_string();
    }
    if remember_credential {
        return stored.map_or_else(
            || Uuid::new_v4().to_string(),
            |record| record.generation.clone(),
        );
    }
    Uuid::new_v4().to_string()
}

pub(super) fn delete_credential(
    store: &dyn CredentialStore,
    profile: &ArgoConnectionProfile,
) -> Result<(), AppError> {
    store.delete(&credential_key(profile))
}

#[cfg(test)]
mod tests;
