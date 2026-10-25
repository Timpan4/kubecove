use crate::{
    backend::BackendState,
    channels::ChannelHub,
    security::{equal_secret, random_token},
};
use kubecove_lib::{
    commands::SessionScope,
    models::{AppError, AppErrorKind},
};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::sync::CancellationToken;

pub struct Owner {
    pub resume_token: String,
    subject: Option<String>,
    pub backend: BackendState,
    pub channels: Arc<ChannelHub>,
    pub tunnels: Arc<crate::tunnel::TunnelRegistry>,
    pub ended: CancellationToken,
    pub request_gate: RwLock<()>,
    pub socket_changed: Notify,
    pub socket: Mutex<SocketState>,
}

#[derive(Default)]
pub struct SocketState {
    pub generation: u64,
    pub connected: bool,
    expiry: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub resume_token: String,
    pub session_grace_seconds: u64,
    pub server_version: &'static str,
}

pub struct Sessions {
    owners: Mutex<Vec<Arc<Owner>>>,
    // Expiry, close and shutdown must not race one another's cleanup.
    transition: Mutex<()>,
    pub grace: Duration,
}

impl Sessions {
    pub fn new(grace: Duration) -> Self {
        Self {
            owners: Mutex::new(Vec::new()),
            transition: Mutex::new(()),
            grace,
        }
    }

    /// Resumes the session `resume`, or opens a new one beside the sessions
    /// already open.
    pub async fn authenticate_as(
        self: &Arc<Self>,
        resume: Option<&str>,
        subject: Option<&str>,
    ) -> Result<SessionInfo, AppError> {
        let _transition = self.transition.lock().await;
        if let Some(resume) = resume {
            let owner = self.owner_as(resume, subject).await?;
            return Ok(self.info(&owner));
        }
        let mut owners = self.owners.lock().await;
        let owner = Arc::new(Owner {
            subject: subject.map(str::to_owned),
            resume_token: random_token().map_err(|_| {
                AppError::new(
                    "failed to generate session credential",
                    AppErrorKind::Internal,
                )
            })?,
            backend: BackendState::new(SessionScope::new(subject.map(str::to_owned))),
            channels: Arc::new(ChannelHub::default()),
            tunnels: Arc::new(crate::tunnel::TunnelRegistry::default()),
            ended: CancellationToken::new(),
            request_gate: RwLock::new(()),
            socket_changed: Notify::new(),
            socket: Mutex::new(SocketState::default()),
        });
        let info = self.info(&owner);
        owners.push(Arc::clone(&owner));
        drop(owners);
        self.disconnected(owner, 0).await;
        Ok(info)
    }

    async fn remove(&self, owner: &Arc<Owner>) {
        self.owners
            .lock()
            .await
            .retain(|other| !Arc::ptr_eq(other, owner));
    }

    fn info(&self, owner: &Owner) -> SessionInfo {
        SessionInfo {
            resume_token: owner.resume_token.clone(),
            session_grace_seconds: self.grace.as_secs(),
            server_version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub async fn owner(&self, resume: &str) -> Result<Arc<Owner>, AppError> {
        self.owners
            .lock()
            .await
            .iter()
            .find(|owner| !owner.ended.is_cancelled() && equal_secret(&owner.resume_token, resume))
            .cloned()
            .ok_or_else(|| AppError::new("session closed or expired", AppErrorKind::Session))
    }

    pub async fn owner_as(
        &self,
        resume: &str,
        subject: Option<&str>,
    ) -> Result<Arc<Owner>, AppError> {
        let owner = self.owner(resume).await?;
        if owner.subject.as_deref() != subject {
            return Err(AppError::new(
                "session belongs to another identity",
                AppErrorKind::Session,
            ));
        }
        Ok(owner)
    }

    pub async fn connect(&self, owner: &Owner) -> u64 {
        let mut socket = owner.socket.lock().await;
        if let Some(expiry) = socket.expiry.take() {
            expiry.abort();
        }
        socket.generation += 1;
        socket.connected = true;
        let generation = socket.generation;
        drop(socket);
        owner.socket_changed.notify_waiters();
        generation
    }

    pub async fn disconnected(self: &Arc<Self>, owner: Arc<Owner>, generation: u64) {
        let mut socket = owner.socket.lock().await;
        if socket.generation != generation || owner.ended.is_cancelled() {
            return;
        }
        socket.connected = false;
        if let Some(expiry) = socket.expiry.take() {
            expiry.abort();
        }
        let sessions = Arc::clone(self);
        let expired_owner = Arc::clone(&owner);
        socket.expiry = Some(tokio::spawn(async move {
            let owner = expired_owner;
            tokio::select! {
                () = owner.ended.cancelled() => return,
                () = tokio::time::sleep(sessions.grace) => {}
            }
            {
                let _transition = sessions.transition.lock().await;
                let socket = owner.socket.lock().await;
                if socket.connected || socket.generation != generation || owner.ended.is_cancelled()
                {
                    return;
                }
                owner.ended.cancel();
                owner.channels.close();
                drop(socket);
                sessions.remove(&owner).await;
            }
            if let Err(error) = end(&owner).await {
                eprintln!("headless session cleanup failed: {}", error.kind.as_str());
            }
        }));
    }

    pub async fn shutdown(&self) {
        let owners = {
            let _transition = self.transition.lock().await;
            let owners = std::mem::take(&mut *self.owners.lock().await);
            for owner in &owners {
                owner.ended.cancel();
            }
            owners
        };
        for owner in owners {
            if let Err(error) = end(&owner).await {
                eprintln!("headless shutdown cleanup failed: {}", error.kind.as_str());
            }
        }
    }

    pub async fn close(&self, resume: &str) -> Result<(), AppError> {
        let owner = {
            let _transition = self.transition.lock().await;
            let owner = self.owner(resume).await?;
            owner.ended.cancel();
            self.remove(&owner).await;
            owner
        };
        end(&owner).await
    }
}

/// Stops everything the session runs, after its in-flight requests finish.
/// Callers mark the session ended and remove it under `transition` first, and
/// call this after releasing it: waiting here for one session's slow request
/// must not hold up every other session's login, resume and close.
async fn end(owner: &Owner) -> Result<(), AppError> {
    owner.ended.cancel();
    owner.channels.close();
    let _requests = owner.request_gate.write().await;
    owner.backend.scope.set_bearer(None);
    owner.tunnels.close_all();
    owner.backend.cleanup().await
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
