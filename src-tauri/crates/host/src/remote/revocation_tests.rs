// Failure modes from the ADR 0031 security review: a token response that
// arrives after Sign out, Cancel or Disconnect stores or uses its credential;
// Sign out that keeps the sign-in while it waits on a silent server; a
// credential store that cannot delete the sign-in while Sign out reports
// success; server error text that quotes the bearer or resume token; a
// session attempt that a sign-out or another server superseded, which then
// publishes its session or acts on its refusal; and a code exchange whose
// caller went away, which leaves an issued credential unrevoked.
use super::session_tests::harness;
use super::store::SIGN_IN_SERVICE;
use super::test_server::{Browser, CREDENTIAL, FakeServer};
use super::*;
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};

#[derive(Clone, Copy, Debug)]
enum Ending {
    SignOut,
    Cancel,
    Disconnect,
}

fn not_deleted(summary: &RemoteConnectionSummary) -> bool {
    serde_json::to_value(summary).unwrap()["signInNotDeleted"] == true
}

#[tokio::test]
async fn a_token_response_after_the_sign_in_ended_is_revoked_not_kept() {
    for ending in [Ending::SignOut, Ending::Cancel, Ending::Disconnect] {
        let harness = harness(Browser::Complete).await;
        let server = &harness.server.state;
        server.stall_tokens.store(true, Ordering::Release);
        harness.select().await.unwrap();
        let store = Arc::clone(&harness.store);
        let signing_in = tokio::spawn(async move { session::sign_in(&store).await });
        server.token_requested.notified().await;
        match ending {
            Ending::SignOut => {
                session::sign_out(&harness.store).await.unwrap();
            }
            Ending::Cancel => session::cancel_sign_in(&harness.store),
            Ending::Disconnect => disconnect(&harness.store).await,
        }
        server.release_tokens.notify_one();
        let _ = signing_in.await.unwrap();
        assert_ne!(
            harness.state().await,
            Some(RemoteSessionState::Connected),
            "{ending:?}"
        );
        assert!(harness.stored().is_none(), "{ending:?}");
        assert!(
            !harness.store.sign_ins.in_memory(&harness.server.url),
            "{ending:?}"
        );
        // The credential nobody holds any more is revoked at the server.
        while !server
            .logouts
            .lock()
            .unwrap()
            .iter()
            .any(|c| c == CREDENTIAL)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!server.valid.lock().unwrap().contains(CREDENTIAL));
    }
}

#[tokio::test]
async fn sign_out_forgets_the_sign_in_before_the_server_answers() {
    for connected in [true, false] {
        let harness = harness(Browser::Complete).await;
        if connected {
            harness.signed_in().await;
        } else {
            // A stored sign-in without a session: the server refuses it at
            // select, and it is stored again before Sign out.
            harness.store_sign_in();
            harness.select().await.unwrap();
            harness.store_sign_in();
        }
        harness
            .server
            .state
            .stall_logouts
            .store(true, Ordering::Release);
        let store = Arc::clone(&harness.store);
        let signing_out = tokio::spawn(async move { session::sign_out(&store).await });
        harness.server.state.logout_requested.notified().await;
        assert!(harness.stored().is_none(), "connected: {connected}");
        assert!(
            !harness.store.sign_ins.in_memory(&harness.server.url),
            "connected: {connected}"
        );
        // Closing the app while the server is silent leaves nothing behind.
        signing_out.abort();
        assert!(harness.stored().is_none());
    }
}

#[tokio::test]
async fn a_sign_in_the_credential_store_cannot_delete_is_reported_and_not_reused() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    harness.secure.refuse_delete.store(true, Ordering::Release);
    // The server cannot revoke it either, so the stored entry still works.
    harness
        .server
        .state
        .refuse_logouts
        .store(true, Ordering::Release);
    let summary = session::sign_out(&harness.store).await.unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInRequired);
    assert!(not_deleted(&summary), "Sign out reports the kept entry");
    assert_eq!(harness.stored().as_deref(), Some(CREDENTIAL));
    // Connecting again neither uses the entry nor hides it.
    disconnect(&harness.store).await;
    let summary = harness.select().await.unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInRequired);
    assert!(not_deleted(&summary));
    // A new sign-in that the store keeps replaces the entry.
    let summary = harness.sign_in().await.unwrap().unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::Connected);
    assert!(!not_deleted(&summary));

    // A refused sign-in that the store cannot delete is reported the same way.
    let harness = self::harness(Browser::Idle).await;
    harness.store_sign_in();
    harness.secure.refuse_delete.store(true, Ordering::Release);
    let summary = harness.select().await.unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInExpired);
    assert!(not_deleted(&summary));
}

#[tokio::test]
async fn server_error_text_never_carries_the_bearer_or_resume_token() {
    let harness = harness(Browser::Complete).await;
    harness.signed_in().await;
    let error = invoke("echo_credentials".into(), json!({}), &harness.store)
        .await
        .unwrap_err();
    assert!(error.message.starts_with("refused bearer"), "{error:?}");
    harness.assert_redacted(&format!("{error:?}"));
}

/// Starts a select that reuses the stored sign-in and waits until the server
/// holds its session request.
async fn stalled_select(
    harness: &super::session_tests::Harness,
) -> tokio::task::JoinHandle<Result<RemoteConnectionSummary, AppError>> {
    harness.store_sign_in();
    let server = &harness.server.state;
    server.valid.lock().unwrap().insert(CREDENTIAL.to_owned());
    server.stall_sessions.store(true, Ordering::Release);
    let store = Arc::clone(&harness.store);
    let url = harness.server.url.clone();
    let connecting = tokio::spawn(async move { connect(url, String::new(), &store).await });
    server.session_requested.notified().await;
    connecting
}

#[tokio::test]
async fn a_session_that_answers_after_sign_out_stays_signed_out() {
    let harness = harness(Browser::Idle).await;
    let connecting = stalled_select(&harness).await;
    session::sign_out(&harness.store).await.unwrap();
    harness.server.state.release_sessions.notify_one();
    let summary = connecting.await.unwrap().unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::SignInRequired);
    assert!(!summary.connected);
    assert!(harness.store.current.read().await.is_none());
}

#[tokio::test]
async fn a_late_refusal_from_a_replaced_server_keeps_the_new_session() {
    let harness = harness(Browser::Idle).await;
    let connecting = stalled_select(&harness).await;
    // The first server refuses the sign-in when the stalled attempt upgrades.
    harness.server.revoke(CREDENTIAL);
    let second = FakeServer::oidc().await;
    second
        .state
        .valid
        .lock()
        .unwrap()
        .insert(CREDENTIAL.to_owned());
    harness.secure.values.lock().unwrap().insert(
        (SIGN_IN_SERVICE.to_owned(), second.url.clone()),
        CREDENTIAL.to_owned(),
    );
    let summary = connect(second.url.clone(), String::new(), &harness.store)
        .await
        .unwrap();
    assert_eq!(summary.session_state, RemoteSessionState::Connected);

    harness.server.state.release_sessions.notify_one();
    let _ = connecting.await.unwrap();
    let summary = harness.status().await.unwrap();
    assert_eq!(summary.server_url, second.url);
    assert_eq!(summary.session_state, RemoteSessionState::Connected);
    invoke("list_kube_contexts".into(), json!({}), &harness.store)
        .await
        .unwrap();
    assert!(harness.store.sign_ins.in_memory(&second.url));
}

#[tokio::test]
async fn a_sign_in_dropped_during_the_code_exchange_revokes_its_credential() {
    let harness = harness(Browser::Complete).await;
    let server = &harness.server.state;
    server.stall_tokens.store(true, Ordering::Release);
    harness.select().await.unwrap();
    let store = Arc::clone(&harness.store);
    let signing_in = tokio::spawn(async move { session::sign_in(&store).await });
    server.token_requested.notified().await;
    // The GPUI client drops the future when it replaces the sign-in task or closes.
    signing_in.abort();
    assert!(signing_in.await.unwrap_err().is_cancelled());
    server.release_tokens.notify_one();
    while !server
        .logouts
        .lock()
        .unwrap()
        .iter()
        .any(|c| c == CREDENTIAL)
    {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(harness.stored().is_none());
    assert_ne!(harness.state().await, Some(RemoteSessionState::Connected));
}
