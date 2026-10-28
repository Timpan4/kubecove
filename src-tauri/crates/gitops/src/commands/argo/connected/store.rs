use super::super::session::SessionStore;
use super::super::tunnel::ArgoServiceTunnel;
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoConnectionProfile};
use reqwest::Client as HttpClient;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Default)]
pub struct ArgoConnectionStore {
    pub(crate) connections: Mutex<HashMap<String, Arc<ConnectedArgo>>>,
    pub(crate) sessions: SessionStore,
    pub(crate) cleanup_epoch: AtomicU64,
}

pub(crate) struct ConnectedArgo {
    pub(crate) profile: ArgoConnectionProfile,
    pub(crate) token: String,
    pub(crate) username: Option<String>,
    pub(crate) client: HttpClient,
    pub(crate) tunnel: Option<ArgoServiceTunnel>,
    pub(crate) generation: String,
    pub(crate) instance_id: String,
    pub(crate) gate: Arc<AsyncMutex<()>>,
}

impl ConnectedArgo {
    fn close_tunnel(&self) {
        if let Some(tunnel) = &self.tunnel {
            tunnel.close();
        }
    }
}

fn cleanup_error() -> AppError {
    AppError::new(
        "Argo CD connection was cancelled by workspace cleanup",
        AppErrorKind::ArgoConnection,
    )
}

impl ArgoConnectionStore {
    pub(crate) fn lock_connections(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, Arc<ConnectedArgo>>>, AppError> {
        self.connections.lock().map_err(|_| {
            AppError::new(
                "Argo CD connection state unavailable",
                AppErrorKind::ArgoConnection,
            )
        })
    }

    pub(crate) fn connection_epoch(&self) -> u64 {
        self.cleanup_epoch.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) async fn replace_connection(
        &self,
        id: String,
        connection: Arc<ConnectedArgo>,
    ) -> Result<(), AppError> {
        self.replace_connection_at(id, connection, self.connection_epoch())
            .await
    }

    pub(crate) async fn replace_connection_at(
        &self,
        id: String,
        connection: Arc<ConnectedArgo>,
        expected_epoch: u64,
    ) -> Result<(), AppError> {
        loop {
            let old = self.lock_connections()?.get(&id).cloned();
            let Some(old) = old else {
                let mut connections = self.lock_connections()?;
                if self.connection_epoch() != expected_epoch {
                    return Err(cleanup_error());
                }
                if connections.contains_key(&id) {
                    continue;
                }
                connections.insert(id.clone(), connection);
                return Ok(());
            };
            let _lease = old.gate.clone().lock_owned().await;
            let mut connections = self.lock_connections()?;
            if self.connection_epoch() != expected_epoch {
                return Err(cleanup_error());
            }
            if connections
                .get(&id)
                .is_some_and(|current| Arc::ptr_eq(current, &old))
            {
                connections.insert(id.clone(), connection);
                drop(connections);
                old.close_tunnel();
                return Ok(());
            }
        }
    }

    pub async fn close_all(&self) -> Result<(), AppError> {
        self.cleanup_epoch.fetch_add(1, Ordering::AcqRel);
        let ids = self.lock_connections()?.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            self.remove_connection(&id).await?;
        }
        Ok(())
    }

    pub(crate) async fn remove_connection(&self, id: &str) -> Result<(), AppError> {
        loop {
            let old = self.lock_connections()?.get(id).cloned();
            let Some(old) = old else {
                return Ok(());
            };
            let _lease = old.gate.clone().lock_owned().await;
            let removed = {
                let mut connections = self.lock_connections()?;
                if connections
                    .get(id)
                    .is_some_and(|current| Arc::ptr_eq(current, &old))
                {
                    connections.remove(id)
                } else {
                    None
                }
            };
            if removed.is_some() {
                old.close_tunnel();
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests;
