use super::*;
use futures_util::poll;
use tokio::sync::oneshot;

// Failure modes: a post-watch read joins an older snapshot; a cancelled caller
// leaves a suspended load behind; cancelling one caller interrupts other callers.
#[tokio::test]
async fn post_watch_read_does_not_join_a_pre_watch_snapshot() {
    let cache = SharedCache::new("test");
    let (release, gate) = oneshot::channel();
    let mut old = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            gate.await.unwrap();
            Ok(vec!["removed-pod"])
        }),
    );
    assert!(poll!(&mut old).is_pending());
    cache.mark_dirty("pods");
    let mut fresh = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            Ok(vec!["replacement-pod"])
        }),
    );
    // Poll before releasing the old snapshot so both requests overlap.
    let first_poll = poll!(&mut fresh);
    let _ = release.send(());
    let result = match first_poll {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => fresh.await,
    };
    assert_eq!(*result.unwrap(), vec!["replacement-pod"]);
    let _ = old.await;
    assert_eq!(
        *cache.peek("pods", CacheMode::GraceOnly).unwrap(),
        vec!["replacement-pod"]
    );
}

#[tokio::test]
async fn leaving_a_view_drops_its_abandoned_snapshot() {
    let cache = SharedCache::new("test");
    let (release, gate) = oneshot::channel();
    let mut old = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            gate.await.unwrap();
            Ok(vec!["removed-pod"])
        }),
    );
    assert!(poll!(&mut old).is_pending());
    drop(old);
    let _ = release.send(());
    let rows = cache
        .get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            Ok(vec!["replacement-pod"])
        })
        .await
        .unwrap();
    assert_eq!(*rows, vec!["replacement-pod"]);
}

#[tokio::test]
async fn leaving_one_view_keeps_a_shared_read_alive_for_another() {
    let cache = SharedCache::new("test");
    let (release, gate) = oneshot::channel();
    let mut first = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            gate.await.unwrap();
            Ok(vec!["shared-pod"])
        }),
    );
    assert!(poll!(&mut first).is_pending());
    let mut second = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            Ok(vec!["unexpected"])
        }),
    );
    assert!(poll!(&mut second).is_pending());
    drop(first);
    release.send(()).unwrap();
    assert_eq!(*second.await.unwrap(), vec!["shared-pod"]);
}

#[tokio::test]
async fn a_failed_replacement_cannot_make_the_pre_watch_fallback_fresh() {
    let cache = SharedCache::new("test");
    let mut previous = SharedCache::ready_value(Arc::new(vec!["removed-pod"]));
    previous.completed_at = Instant::now().checked_sub(COMPLETED_GRACE).unwrap();
    cache
        .entries
        .lock()
        .unwrap()
        .insert("pods".into(), CacheEntry::Ready(previous));
    let (_release, gate) = oneshot::channel::<()>();
    let mut old = Box::pin(
        cache.get_or_load("pods".into(), CacheMode::GraceOnly, || async {
            let _ = gate.await;
            Ok(vec!["removed-pod"])
        }),
    );
    assert!(poll!(&mut old).is_pending());
    cache.mark_dirty("pods");
    assert!(cache
        .get_or_load(
            "pods".into(),
            CacheMode::LiveFor(RESOURCE_FRESHNESS),
            || async { Err(AppError::cancelled()) }
        )
        .await
        .is_err());
    drop(old);
    let fresh = cache
        .get_or_load(
            "pods".into(),
            CacheMode::LiveFor(RESOURCE_FRESHNESS),
            || async { Ok(vec!["replacement-pod"]) },
        )
        .await
        .unwrap();
    assert_eq!(*fresh, vec!["replacement-pod"]);
}
