use super::*;
use crate::oidc::OidcConfig;
use futures_util::FutureExt;
use std::time::Duration;

fn bind_expired_token_without_refresh(oidc: &Oidc) -> String {
    let key = oidc.insert_identity_for_test(
        "https://id.example",
        "user",
        unix_now() + REFRESH_MARGIN_SECONDS,
    );
    oidc.credentials
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .tokens = Some(Tokens {
        id_token: "never-forwarded-expired-token".into(),
        id_expires_unix: 0,
        refresh_token: None,
        nonce: String::new(),
        subject: "user".into(),
    });
    oidc.bind(
        &OidcCredential(key.clone()),
        &key,
        SessionScope::new(None),
        &CancellationToken::new(),
    )
    .unwrap();
    key
}

#[tokio::test(start_paused = true)]
async fn stalled_renewal_does_not_block_other_sign_ins_or_later_ticks() {
    let config = OidcConfig::new(
        "https://id.example".into(),
        "client".into(),
        None,
        Duration::from_secs(REFRESH_MARGIN_SECONDS),
    )
    .unwrap();
    let oidc = Oidc::new_for_test(config, "https://app.example").unwrap();
    let mut keys = [
        bind_expired_token_without_refresh(&oidc),
        bind_expired_token_without_refresh(&oidc),
    ];
    keys.sort_unstable();
    let gate = Arc::clone(&oidc.credentials.lock().unwrap()[&keys[0]].refreshing);
    let _stalled = gate.lock().await;
    let revoked = oidc.credentials.lock().unwrap()[&keys[1]].revoked.clone();
    let mut renewals = Box::pin(oidc.refresh_forever());
    assert!(renewals.as_mut().now_or_never().is_none());
    tokio::time::advance(REFRESH_INTERVAL).await;
    assert!(renewals.as_mut().now_or_never().is_none());
    // An explicit missing-refresh refusal must be processed despite the first
    // credential's stalled renewal. No network or wall-clock timing is involved.
    assert!(
        revoked.is_cancelled(),
        "the first tick did not reach the other sign-in"
    );

    let later = bind_expired_token_without_refresh(&oidc);
    let revoked = oidc.credentials.lock().unwrap()[&later].revoked.clone();
    tokio::time::advance(REFRESH_INTERVAL).await;
    assert!(renewals.as_mut().now_or_never().is_none());
    assert!(
        revoked.is_cancelled(),
        "a pending renewal blocked subsequent ticks"
    );
}
