use super::*;

#[tokio::test(start_paused = true)]
async fn sessions_run_side_by_side_and_close_alone() {
    let sessions = Arc::new(Sessions::new(Duration::from_mins(5)));
    let alice = sessions
        .authenticate_as(None, Some("alice"))
        .await
        .unwrap();
    let bob = sessions.authenticate_as(None, Some("bob")).await.unwrap();
    let alice_owner = sessions.owner(&alice.resume_token).await.unwrap();
    let bob_owner = sessions.owner(&bob.resume_token).await.unwrap();
    assert!(!alice_owner.ended.is_cancelled());
    assert!(!Arc::ptr_eq(&alice_owner.backend.scope, &bob_owner.backend.scope));

    assert!(sessions
        .owner_as(&alice.resume_token, Some("bob"))
        .await
        .is_err());

    sessions.close(&alice.resume_token).await.unwrap();
    assert!(alice_owner.ended.is_cancelled());
    assert!(sessions.owner(&alice.resume_token).await.is_err());
    assert!(!bob_owner.ended.is_cancelled());
    assert!(sessions
        .owner_as(&bob.resume_token, Some("bob"))
        .await
        .is_ok());
}

#[tokio::test(start_paused = true)]
async fn a_close_waiting_on_a_request_holds_up_no_other_login() {
    let sessions = Arc::new(Sessions::new(Duration::from_mins(5)));
    let alice = sessions.authenticate(None).await.unwrap();
    let alice_owner = sessions.owner(&alice.resume_token).await.unwrap();
    let in_flight = alice_owner.request_gate.read().await;

    let closing = {
        let sessions = Arc::clone(&sessions);
        let resume = alice.resume_token.clone();
        tokio::spawn(async move { sessions.close(&resume).await })
    };
    tokio::task::yield_now().await;
    assert!(alice_owner.ended.is_cancelled());

    let bob = tokio::time::timeout(Duration::from_secs(1), sessions.authenticate(None)).await;
    assert!(bob.is_ok_and(|bob| bob.is_ok()), "login waited for another session's cleanup");
    assert!(!closing.is_finished());

    drop(in_flight);
    closing.await.unwrap().unwrap();
}

#[tokio::test]
async fn diagnostics_belong_to_their_session() {
    use kubecove_lib::commands::{
        get_backend_diagnostics, record_backend_error, set_backend_diagnostics_enabled,
    };
    let alice = BackendState::default();
    let bob = BackendState::default();
    bob.scoped(async { set_backend_diagnostics_enabled(true) })
        .await
        .unwrap();
    alice
        .scoped(async {
            set_backend_diagnostics_enabled(true).unwrap();
            record_backend_error("list_namespaces", std::time::Instant::now(), "forbidden");
        })
        .await;
    assert_eq!(
        alice
            .scoped(async { get_backend_diagnostics() })
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        bob.scoped(async { get_backend_diagnostics() })
            .await
            .unwrap()
            .len(),
        0
    );
}

// Watches, exec and port-forwards build their clients inside tasks that the
// library starts with runtime::spawn. Without the session's scope there, a
// forwarding server refuses to build them.
#[tokio::test]
async fn library_tasks_keep_the_session_scope() {
    use kubecove_lib::commands::SessionScope;
    let backend = BackendState::default();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    backend
        .scoped(async {
            kubecove_lib::runtime::spawn(async move {
                let _ = sender.send(SessionScope::current());
            });
        })
        .await;
    let seen = receiver.await.unwrap().expect("a scope in the task");
    assert!(Arc::ptr_eq(&seen, &backend.scope));
}

#[tokio::test(start_paused = true)]
async fn reconnect_invalidates_the_old_disconnect_timer_and_expiry_revokes_access() {
    let grace = Duration::from_mins(5);
    let sessions = Arc::new(Sessions::new(grace));
    let info = sessions.authenticate(None).await.unwrap();
    let owner = sessions.owner(&info.resume_token).await.unwrap();
    let first_socket = sessions.connect(&owner).await;
    sessions
        .disconnected(Arc::clone(&owner), first_socket)
        .await;
    tokio::task::yield_now().await;
    let second_socket = sessions.connect(&owner).await;
    tokio::time::advance(grace).await;
    tokio::task::yield_now().await;
    assert!(!owner.ended.is_cancelled());
    assert!(sessions.owner(&info.resume_token).await.is_ok());
    sessions
        .disconnected(Arc::clone(&owner), second_socket)
        .await;
    tokio::task::yield_now().await;
    tokio::time::advance(grace).await;
    owner.ended.cancelled().await;
    assert!(sessions.owner(&info.resume_token).await.is_err());
}

#[tokio::test(start_paused = true)]
async fn zero_grace_stops_on_disconnect_and_login_without_socket_eventually_expires() {
    let sessions = Arc::new(Sessions::new(Duration::ZERO));
    let info = sessions.authenticate(None).await.unwrap();
    let owner = sessions.owner(&info.resume_token).await.unwrap();
    let socket = sessions.connect(&owner).await;
    sessions.disconnected(Arc::clone(&owner), socket).await;
    owner.ended.cancelled().await;
    assert!(sessions.owner(&info.resume_token).await.is_err());
}
