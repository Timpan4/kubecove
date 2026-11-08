// Failure modes from the ADR 0031 security review: a token response that
// arrives after Sign out, Cancel or Disconnect stores or uses its credential;
// Sign out that keeps the sign-in while it waits on a silent server; a
// credential store that cannot delete the sign-in while Sign out reports
// success; and server error text that quotes the bearer or resume token.
use super::session_tests::harness;
use super::test_server::{Browser, CREDENTIAL};
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
