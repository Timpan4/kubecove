use crate::{
    backend::BackendState,
    channels::ChannelHub,
    metrics::{CacheTotals, RetiredCaches},
    observe::{self, AuthMode, CloseReason, SessionId},
    security::{equal_secret, random_token},
};
use kubecove_lib::{
    commands::SessionScope,
    models::{AppError, AppErrorKind},
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::sync::CancellationToken;

pub struct Owner {
    pub resume_token: String,
    pub session_id: SessionId,
    subject: Option<String>,
    pub backend: BackendState,
    pub channels: Arc<ChannelHub>,
    pub tunnels: Arc<crate::tunnel::TunnelRegistry>,
    pub ended: CancellationToken,
    pub request_gate: RwLock<()>,
    pub socket_changed: Notify,
    pub socket: Mutex<SocketState>,
    /// Declared last: fields drop in order, so this runs after the session's caches are freed.
    _release_heap: ReleaseHeapOnDrop,
}

/// The session's browser socket. Each connect bumps the generation, so a
/// replaced socket's disconnect and a stale expiry timer are both ignored.
#[derive(Default)]
pub enum SocketState {
    /// Opened, before its first grace timer starts.
    #[default]
    New,
    Connected {
        generation: u64,
    },
    /// No socket; the session expires when the timer fires.
    Grace {
        generation: u64,
        expiry: tokio::task::JoinHandle<()>,
    },
}

impl SocketState {
    pub fn generation(&self) -> u64 {
        match self {
            Self::New => 0,
            Self::Connected { generation } | Self::Grace { generation, .. } => *generation,
        }
    }

    fn replace(&mut self, next: Self) {
        if let Self::Grace { expiry, .. } = std::mem::replace(self, next) {
            expiry.abort();
        }
    }
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
    auth_mode: AuthMode,
    retired_caches: RetiredCaches,
}

impl Sessions {
    pub fn new(grace: Duration, auth_mode: AuthMode) -> Self {
        Self {
            owners: Mutex::new(Vec::new()),
            transition: Mutex::new(()),
            grace,
            auth_mode,
            retired_caches: RetiredCaches::default(),
        }
    }

    /// Open sessions and cache totals by cache label. A session is removed
    /// from the live list before `end` retires its counters, so it is never
    /// counted twice.
    pub async fn cache_totals(&self) -> (usize, BTreeMap<String, CacheTotals>) {
        let mut totals = self.retired_caches.totals();
        let owners = self.owners.lock().await.clone();
        for owner in &owners {
            CacheTotals::add_live(&mut totals, &owner.backend.live_store);
        }
        (owners.len(), totals)
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
            observe::session_resumed(owner.session_id, self.auth_mode, subject);
            return Ok(self.info(&owner));
        }
        let mut owners = self.owners.lock().await;
        let credential_failed = |_| {
            AppError::new(
                "failed to generate session credential",
                AppErrorKind::Internal,
            )
        };
        let owner = Arc::new(Owner {
            subject: subject.map(str::to_owned),
            resume_token: random_token().map_err(credential_failed)?,
            session_id: SessionId::random().map_err(credential_failed)?,
            backend: BackendState::new(SessionScope::new(subject.map(str::to_owned))),
            channels: Arc::new(ChannelHub::default()),
            tunnels: Arc::new(crate::tunnel::TunnelRegistry::default()),
            ended: CancellationToken::new(),
            request_gate: RwLock::new(()),
            socket_changed: Notify::new(),
            socket: Mutex::new(SocketState::default()),
            _release_heap: ReleaseHeapOnDrop,
        });
        let info = self.info(&owner);
        owners.push(Arc::clone(&owner));
        drop(owners);
        observe::session_opened(owner.session_id, self.auth_mode, subject);
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
        let generation = socket.generation() + 1;
        socket.replace(SocketState::Connected { generation });
        drop(socket);
        owner.socket_changed.notify_waiters();
        generation
    }

    pub async fn disconnected(self: &Arc<Self>, owner: Arc<Owner>, generation: u64) {
        let mut socket = owner.socket.lock().await;
        if socket.generation() != generation || owner.ended.is_cancelled() {
            return;
        }
        let sessions = Arc::clone(self);
        let expired_owner = Arc::clone(&owner);
        let expiry = tokio::spawn(async move {
            let owner = expired_owner;
            tokio::select! {
                () = owner.ended.cancelled() => return,
                () = tokio::time::sleep(sessions.grace) => {}
            }
            {
                let _transition = sessions.transition.lock().await;
                let socket = owner.socket.lock().await;
                let still_waiting = matches!(
                    *socket,
                    SocketState::Grace { generation: current, .. } if current == generation
                );
                if !still_waiting || owner.ended.is_cancelled() {
                    return;
                }
                owner.ended.cancel();
                owner.channels.close();
                drop(socket);
                sessions.remove(&owner).await;
            }
            observe::session_expired(owner.session_id, sessions.grace);
            let _ = sessions.end(&owner).await;
        });
        socket.replace(SocketState::Grace { generation, expiry });
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
            observe::session_closed(owner.session_id, CloseReason::Shutdown);
            let _ = self.end(&owner).await;
        }
    }

    pub async fn close(&self, resume: &str, reason: CloseReason) -> Result<(), AppError> {
        let owner = {
            let _transition = self.transition.lock().await;
            let owner = self.owner(resume).await?;
            owner.ended.cancel();
            self.remove(&owner).await;
            owner
        };
        observe::session_closed(owner.session_id, reason);
        self.end(&owner).await
    }

    /// Stops everything the session runs, after its in-flight requests finish,
    /// then retires its cache counters. Callers mark the session ended and
    /// remove it under `transition` first, and call this after releasing it:
    /// waiting here for one session's slow request must not hold up every
    /// other session's login, resume and close.
    async fn end(&self, owner: &Owner) -> Result<(), AppError> {
        owner.ended.cancel();
        owner.channels.close();
        let _requests = owner.request_gate.write().await;
        owner.backend.scope.set_bearer(None);
        owner.tunnels.close_all();
        let result = owner.backend.cleanup().await;
        if let Err(error) = &result {
            observe::session_cleanup_failed(owner.session_id, error.kind);
        }
        self.retired_caches.retire(&owner.backend.live_store);
        result
    }
}

/// An idle server stayed at its peak (1 GiB on one deployment) because the
/// pages a session's caches freed were never returned. The workspace-close
/// trim in `cleanup` runs while the caches are still alive, so this one runs
/// after they are gone.
struct ReleaseHeapOnDrop;

impl Drop for ReleaseHeapOnDrop {
    fn drop(&mut self) {
        // The trim took 1.5 to 2.9 ms after a large session, and the last owner
        // reference usually drops on an async worker.
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => drop(runtime.spawn_blocking(kubecove_lib::commands::release_freed_heap)),
            Err(_) => kubecove_lib::commands::release_freed_heap(),
        }
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
