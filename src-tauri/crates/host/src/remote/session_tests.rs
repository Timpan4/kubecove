// Failure modes (ADR 0031): a sign-in that leaks its credential, code,
// verifier or state into an error or summary; a listener that a forged
// redirect stops; a denial or cancel that strands the client; a refused
// sign-in that stays stored; Disconnect that drops the sign-in or Sign out
// that keeps it; a lost socket that falls back to anything but its own
// session; and a session state that does not follow the server's signals.
use super::sign_in::{ACCESS_UNAVAILABLE, NATIVE_SIGN_IN_UNSUPPORTED, NOT_COMPLETED};
use super::store::SIGN_IN_SERVICE;
use super::test_server::{Browser, CODE, CREDENTIAL, FakeBrowser, FakeServer, MemoryStore};
use super::*;
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};

pub(super) struct Harness {
    pub server: FakeServer,
    pub store: Arc<RemoteConnectionStore>,
    pub secure: Arc<MemoryStore>,
    pub browser: Arc<FakeBrowser>,
}

pub(super) async fn harness(behavior: Browser) -> Harness {
    let server = FakeServer::oidc().await;
    let secure = Arc::new(MemoryStore::default());
    let browser = FakeBrowser::new(&server, behavior);
    let store = Arc::new(RemoteConnectionStore::new(
        Arc::clone(&secure) as _,
        Arc::clone(&browser) as _,
    ));
    Harness {
        server,
        store,
        secure,
        browser,
    }
}

impl Harness {
    pub(super) async fn status(&self) -> Option<RemoteConnectionSummary> {
        self.store.summary().await
    }

    pub(super) async fn state(&self) -> Option<RemoteSessionState> {
        self.status().await.map(|summary| summary.session_state)
    }

    /// Polls the status until it reports `state`.
    async fn wait_for(&self, state: RemoteSessionState) -> RemoteConnectionSummary {
        loop {
            if let Some(summary) = self.status().await
                && summary.session_state == state
            {
                return summary;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub(super) async fn select(&self) -> Result<RemoteConnectionSummary, AppError> {
        connect(self.server.url.clone(), String::new(), &self.store).await
    }

    pub(super) async fn sign_in(&self) -> Result<Option<RemoteConnectionSummary>, AppError> {
        session::sign_in(&self.store).await
    }

    pub(super) fn stored(&self) -> Option<String> {
        self.secure
            .values
            .lock()
            .unwrap()
            .get(&(SIGN_IN_SERVICE.to_owned(), self.server.url.clone()))
            .cloned()
    }

    pub(super) fn store_sign_in(&self) {
        self.secure.values.lock().unwrap().insert(
            (SIGN_IN_SERVICE.to_owned(), self.server.url.clone()),
            CREDENTIAL.to_owned(),
        );
    }

    pub(super) async fn signed_in(&self) {
        assert_eq!(
            self.select().await.unwrap().session_state,
            RemoteSessionState::SignInRequired
        );
        let summary = self.sign_in().await.unwrap().unwrap();
        assert_eq!(summary.session_state, RemoteSessionState::Connected);
        assert!(summary.connected);
    }

    /// Every secret this sign-in used: the credential, code, verifier and state.
    fn secrets(&self) -> Vec<String> {
        let mut secrets = vec![
            CREDENTIAL.to_owned(),
            CODE.to_owned(),
            "resume-secret".to_owned(),
        ];
        secrets.extend(self.server.state.verifiers.lock().unwrap().iter().cloned());
        for link in self.browser.opened.lock().unwrap().iter() {
            let url = reqwest::Url::parse(link).unwrap();
            secrets.extend(
                url.query_pairs()
                    .filter(|(key, _)| key == "state")
                    .map(|(_, value)| value.into_owned()),
            );
        }
        secrets
    }

    pub(super) fn assert_redacted(&self, text: &str) {
        for secret in self.secrets() {
            assert!(!text.contains(&secret), "leaked a secret in {text}");
        }
    }
}

#[tokio::test]
async fn sign_in_opens_a_session_and_disconnect_keeps_the_sign_in() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    assert_eq!(harness.stored().as_deref(), Some(CREDENTIAL));
    let summary = harness.status().await.unwrap();
    assert_eq!(summary.auth_mode, RemoteAuthMode::Oidc);
    assert!(!summary.sign_in_memory_only);
    harness.assert_redacted(&format!("{summary:?}"));
    harness.assert_redacted(&serde_json::to_string(&summary).unwrap());
    let link = harness.browser.opened.lock().unwrap()[0].clone();
    assert!(!link.contains(CREDENTIAL), "the link carries no secret");
    invoke("list_kube_contexts".into(), json!({}), &harness.store)
        .await
        .unwrap();

    disconnect(&harness.store).await;
    assert!(harness.status().await.is_none());
    assert!(harness.store.sign_ins.in_memory(&harness.server.url));
    assert_eq!(harness.stored().as_deref(), Some(CREDENTIAL));
    // Reconnecting to the same server reuses the sign-in without a browser.
    assert_eq!(
        harness.select().await.unwrap().session_state,
        RemoteSessionState::Connected
    );
    assert_eq!(harness.browser.opened.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_stored_sign_in_survives_a_restart() {
    let harness = harness(Browser::Idle).await;
    harness.store_sign_in();
    harness
        .server
        .state
        .valid
        .lock()
        .unwrap()
        .insert(CREDENTIAL.to_owned());
    assert_eq!(
        harness.select().await.unwrap().session_state,
        RemoteSessionState::Connected
    );
    assert!(harness.browser.opened.lock().unwrap().is_empty());
}

#[tokio::test]
async fn forged_redirects_and_other_paths_do_not_stop_the_listener() {
    let harness = harness(Browser::ForgeThenComplete).await;
    harness.signed_in().await;
}

#[tokio::test]
async fn provider_denial_reports_an_incomplete_sign_in() {
    let harness = harness(Browser::Deny).await;
    harness.select().await.unwrap();
    let error = harness.sign_in().await.unwrap_err();
    assert_eq!(error.message, NOT_COMPLETED);
    harness.assert_redacted(&format!("{error:?}"));
    assert_eq!(
        harness.state().await,
        Some(RemoteSessionState::SignInRequired)
    );
    assert!(harness.stored().is_none());
}

#[tokio::test]
async fn a_refused_code_exchange_reveals_no_secret() {
    let harness = harness(Browser::LostTransaction).await;
    harness.select().await.unwrap();
    let error = harness.sign_in().await.unwrap_err();
    harness.assert_redacted(&format!("{error:?}"));
    assert_eq!(
        harness.state().await,
        Some(RemoteSessionState::SignInRequired)
    );
}

#[tokio::test]
async fn cancel_and_disconnect_end_a_waiting_sign_in() {
    let harness = harness(Browser::Idle).await;
    harness.select().await.unwrap();
    let store = Arc::clone(&harness.store);
    let waiting = tokio::spawn(async move { session::sign_in(&store).await });
    harness
        .wait_for(RemoteSessionState::WaitingForBrowser)
        .await;
    let link = session::sign_in_link(&harness.store).unwrap();
    assert_eq!(link, harness.browser.opened.lock().unwrap()[0]);
    session::cancel_sign_in(&harness.store);
    let summary = waiting.await.unwrap().unwrap().unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInRequired);
    assert!(session::sign_in_link(&harness.store).is_none());

    let store = Arc::clone(&harness.store);
    let waiting = tokio::spawn(async move { session::sign_in(&store).await });
    harness
        .wait_for(RemoteSessionState::WaitingForBrowser)
        .await;
    disconnect(&harness.store).await;
    assert!(waiting.await.unwrap().unwrap().is_none());
    assert!(harness.status().await.is_none());
}

#[tokio::test]
async fn a_sign_in_dropped_while_it_waits_for_the_browser_can_start_again() {
    let harness = harness(Browser::Idle).await;
    harness.select().await.unwrap();
    let store = Arc::clone(&harness.store);
    let waiting = tokio::spawn(async move { session::sign_in(&store).await });
    harness
        .wait_for(RemoteSessionState::WaitingForBrowser)
        .await;
    // The GPUI client drops the future when it replaces the sign-in task or closes.
    waiting.abort();
    assert!(waiting.await.unwrap_err().is_cancelled());
    assert_eq!(harness.state().await, Some(RemoteSessionState::SignInRequired));
    assert!(session::sign_in_link(&harness.store).is_none());

    *harness.browser.behavior.lock().unwrap() = Browser::Complete;
    let summary = harness.sign_in().await.unwrap().unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::Connected);
}

#[tokio::test]
async fn the_session_is_connecting_while_the_server_opens_it() {
    let harness = harness(Browser::Idle).await;
    harness.store_sign_in();
    harness
        .server
        .state
        .valid
        .lock()
        .unwrap()
        .insert(CREDENTIAL.to_owned());
    harness
        .server
        .state
        .stall_sessions
        .store(true, Ordering::Release);
    let store = Arc::clone(&harness.store);
    let url = harness.server.url.clone();
    let connecting = tokio::spawn(async move { connect(url, String::new(), &store).await });
    harness.server.state.session_requested.notified().await;
    assert_eq!(harness.state().await, Some(RemoteSessionState::Connecting));
    harness.server.state.release_sessions.notify_one();
    let summary = connecting.await.unwrap().unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::Connected);
}

#[tokio::test]
async fn a_moved_session_ends_and_a_new_session_recovers() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    harness.server.send(r#"{"type":"session_moved"}"#);
    let summary = harness.wait_for(RemoteSessionState::SessionEnded).await;
    assert!(!summary.connected);
    let error = invoke("list_kube_contexts".into(), json!({}), &harness.store)
        .await
        .unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Session, "no local fallback");
    let summary = session::start_new_session(&harness.store).await.unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::Connected);
}

#[tokio::test]
async fn a_moved_session_signals_the_views_to_read_it_again() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    let mut changes = subscribe_session_changes();
    changes.borrow_and_update();
    harness.server.send(r#"{"type":"session_moved"}"#);
    // The signal follows the stored state, so waiting for the signal is
    // enough. A reader woken by it sees SessionEnded.
    changes.changed().await.expect("session signal");
    let summary = harness.wait_for(RemoteSessionState::SessionEnded).await;
    assert!(!summary.connected);
}

#[tokio::test]
async fn a_dropped_socket_reconnects_within_the_grace_window_or_expires() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    *harness.server.state.refuse_upgrades.lock().unwrap() = Some(503);
    harness.server.drop_sockets();
    let summary = harness.wait_for(RemoteSessionState::Reconnecting).await;
    assert!(summary.connected, "the session is still this app's");
    *harness.server.state.refuse_upgrades.lock().unwrap() = None;
    harness.wait_for(RemoteSessionState::Connected).await;

    // A one-second grace window elapses while the server refuses upgrades.
    *harness.server.state.grace.lock().unwrap() = 1;
    session::start_new_session(&harness.store).await.unwrap();
    *harness.server.state.refuse_upgrades.lock().unwrap() = Some(503);
    harness.server.drop_sockets();
    let summary = harness.wait_for(RemoteSessionState::SessionExpired).await;
    assert!(!summary.connected);
    assert_eq!(harness.stored().as_deref(), Some(CREDENTIAL));
}

#[tokio::test]
async fn a_revoked_sign_in_is_forgotten_wherever_the_server_refuses_it() {
    // Refused on an invoke.
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    harness.server.revoke(CREDENTIAL);
    let error = invoke("list_kube_contexts".into(), json!({}), &harness.store)
        .await
        .unwrap_err();
    harness.assert_redacted(&format!("{error:?}"));
    assert_eq!(
        harness.state().await,
        Some(RemoteSessionState::SignInExpired)
    );
    assert!(harness.stored().is_none());
    assert!(!harness.store.sign_ins.in_memory(&harness.server.url));

    // Refused on the reconnect upgrade.
    let harness = self::harness(Browser::Complete).await;
    harness.signed_in().await;
    harness.server.revoke(CREDENTIAL);
    harness.server.drop_sockets();
    harness.wait_for(RemoteSessionState::SignInExpired).await;
    assert!(harness.stored().is_none());

    // Refused when a stored sign-in opens a session after a server restart.
    let harness = self::harness(Browser::Idle).await;
    harness.store_sign_in();
    assert_eq!(
        harness.select().await.unwrap().session_state,
        RemoteSessionState::SignInExpired
    );
    assert!(harness.stored().is_none());
    // "Start a new session" without a sign-in stays expired.
    assert_eq!(
        session::start_new_session(&harness.store)
            .await
            .unwrap()
            .session_state,
        RemoteSessionState::SignInExpired
    );
}

#[tokio::test]
async fn sign_out_revokes_at_the_server_and_forgets_the_sign_in() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    let summary = session::sign_out(&harness.store).await.unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInRequired);
    assert_eq!(*harness.server.state.logouts.lock().unwrap(), [CREDENTIAL]);
    assert!(harness.stored().is_none());
    assert!(!harness.store.sign_ins.in_memory(&harness.server.url));

    // Sign out also forgets a sign-in the server cannot be reached for.
    let harness = self::harness(Browser::Idle).await;
    harness.store_sign_in();
    harness.select().await.unwrap();
    harness.store_sign_in();
    session::sign_out(&harness.store).await.unwrap();
    assert!(harness.stored().is_none());
}

#[tokio::test]
async fn an_unavailable_credential_store_keeps_the_sign_in_in_memory() {
    let harness = harness(Browser::Complete).await;
    harness.secure.refuse.store(true, Ordering::Release);
    harness.signed_in().await;
    let summary = harness.status().await.unwrap();
    assert!(summary.sign_in_memory_only);
    assert!(harness.stored().is_none());
    assert!(harness.store.sign_ins.in_memory(&harness.server.url));
}

#[tokio::test]
async fn servers_without_desktop_sign_in_are_refused_with_their_reason() {
    for (mode, message) in [
        (json!({"mode": "cloudflareAccess"}), ACCESS_UNAVAILABLE),
        (
            json!({"mode": "oidc", "authenticated": false}),
            NATIVE_SIGN_IN_UNSUPPORTED,
        ),
        (
            json!({"mode": "token"}),
            "This server uses a server token. Enter the token to connect.",
        ),
    ] {
        let server = FakeServer::start(mode).await;
        let store = RemoteConnectionStore::new(
            Arc::new(MemoryStore::default()),
            FakeBrowser::new(&server, Browser::Idle),
        );
        let error = connect(server.url.clone(), String::new(), &store)
            .await
            .unwrap_err();
        assert_eq!(error.message, message);
        assert!(store.summary().await.is_none());
    }
}
