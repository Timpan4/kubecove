use super::{COMPLETED_GRACE, MAX_FRESHNESS, cache_diagnostics::CacheCounters, list::RowSlots};
use crate::models::AppError;
use futures_util::future::{BoxFuture, FutureExt, Shared};
use std::{
    collections::HashMap,
    future::Future,
    ops::Deref,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

pub(super) const MAX_CACHE_ENTRIES: usize = 128;

pub(super) type SharedLoad<T> = Shared<BoxFuture<'static, Result<Arc<T>, AppError>>>;

#[derive(Clone, Copy)]
pub(super) enum CacheMode {
    GraceOnly,
    LiveFor(Duration),
}

pub(super) struct ReadyValue<T> {
    pub(super) value: Arc<T>,
    pub(super) completed_at: Instant,
    pub(super) dirty: bool,
    pub(super) slots: Option<Box<RowSlots>>,
}

pub(super) enum CacheEntry<T> {
    Ready(ReadyValue<T>),
    Loading {
        load_id: u64,
        future: SharedLoad<T>,
        cancellation: CancellationToken,
        previous: Option<ReadyValue<T>>,
        dirty: bool,
        waiters: usize,
    },
}

pub(super) struct SharedCache<T> {
    pub(super) label: &'static str,
    next_load_id: AtomicU64,
    pub(super) entries: Mutex<CacheEntries<T>>,
    pub(super) counters: CacheCounters,
    /// Counter values at the last diagnostics clear. The counters themselves
    /// never reset, so server metrics stay monotonic.
    pub(super) cleared: CacheCounters,
}

pub(super) struct CacheEntries<T> {
    by_key: HashMap<String, CacheEntry<T>>,
    ready_count: usize,
}

impl<T> Default for CacheEntries<T> {
    fn default() -> Self {
        Self {
            by_key: HashMap::new(),
            ready_count: 0,
        }
    }
}

impl<T> Deref for CacheEntries<T> {
    type Target = HashMap<String, CacheEntry<T>>;

    fn deref(&self) -> &Self::Target {
        &self.by_key
    }
}

impl<T> CacheEntries<T> {
    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut CacheEntry<T>> {
        self.by_key.get_mut(key)
    }

    pub(super) fn insert(&mut self, key: String, entry: CacheEntry<T>) {
        self.ready_count += usize::from(matches!(&entry, CacheEntry::Ready(_)));
        let replaced = self.by_key.insert(key, entry);
        self.ready_count -= usize::from(matches!(replaced, Some(CacheEntry::Ready(_))));
    }

    fn remove(&mut self, key: &str) -> Option<CacheEntry<T>> {
        let removed = self.by_key.remove(key);
        self.ready_count -= usize::from(matches!(&removed, Some(CacheEntry::Ready(_))));
        removed
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&str, &CacheEntry<T>) -> bool) {
        self.by_key.retain(|key, entry| {
            let retain = keep(key, entry);
            self.ready_count -= usize::from(!retain && matches!(entry, CacheEntry::Ready(_)));
            retain
        });
    }
}

// The cache retains a lazy shared future. Without a caller, that future must be
// removed so a later view cannot resume an abandoned Kubernetes snapshot.
struct LoadWaiter<'a, T> {
    cache: &'a SharedCache<T>,
    key: &'a str,
    load_id: u64,
}

impl<T> Drop for LoadWaiter<'_, T> {
    fn drop(&mut self) {
        let mut entries = self.cache.entries.lock().expect("live store cache lock");
        let Some(CacheEntry::Loading {
            load_id, waiters, ..
        }) = entries.get_mut(self.key)
        else {
            return;
        };
        if *load_id != self.load_id {
            return;
        }
        *waiters -= 1;
        if *waiters != 0 {
            return;
        }
        let Some(CacheEntry::Loading {
            future,
            cancellation,
            previous,
            ..
        }) = entries.remove(self.key)
        else {
            return;
        };
        cancellation.cancel();
        if let Some(mut previous) = previous {
            previous.dirty = true;
            entries.insert(self.key.to_string(), CacheEntry::Ready(previous));
        }
        // The last shared handle owns the abandoned load future; drop it after
        // unlocking so its teardown never runs under the cache lock.
        drop(entries);
        drop(future);
    }
}

impl<T> SharedCache<T>
where
    T: Send + Sync + 'static,
{
    pub(super) fn new(label: &'static str) -> Self {
        Self {
            label,
            next_load_id: AtomicU64::new(0),
            entries: Mutex::new(CacheEntries::default()),
            counters: CacheCounters::default(),
            cleared: CacheCounters::default(),
        }
    }

    pub(super) fn ready_value(value: Arc<T>) -> ReadyValue<T> {
        ReadyValue {
            value,
            completed_at: Instant::now(),
            dirty: false,
            slots: None,
        }
    }

    fn can_reuse(ready: &ReadyValue<T>, mode: CacheMode) -> bool {
        let freshness = match mode {
            CacheMode::GraceOnly => COMPLETED_GRACE,
            CacheMode::LiveFor(freshness) => freshness,
        };
        debug_assert!(freshness <= MAX_FRESHNESS);
        if ready.dirty {
            return false;
        }

        ready.completed_at.elapsed() <= freshness
    }

    pub(super) fn peek(&self, key: &str, mode: CacheMode) -> Option<Arc<T>> {
        let entries = self.entries.lock().expect("live store cache lock");
        match entries.get(key) {
            Some(CacheEntry::Ready(ready)) if Self::can_reuse(ready, mode) => {
                self.counters.hits.fetch_add(1, Ordering::Relaxed);
                Some(Arc::clone(&ready.value))
            }
            _ => None,
        }
    }

    pub(super) async fn get_or_load<F, Fut>(
        &self,
        key: String,
        mode: CacheMode,
        loader: F,
    ) -> Result<Arc<T>, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        let (future, load_id) = {
            let mut entries = self.entries.lock().expect("live store cache lock");
            match entries.get_mut(&key) {
                Some(CacheEntry::Ready(ready)) if Self::can_reuse(ready, mode) => {
                    self.counters.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(Arc::clone(&ready.value));
                }
                Some(CacheEntry::Loading {
                    load_id,
                    future,
                    waiters,
                    dirty: false,
                    ..
                }) => {
                    self.counters.joins.fetch_add(1, Ordering::Relaxed);
                    *waiters += 1;
                    (future.clone(), *load_id)
                }
                _ => {
                    self.counters.misses.fetch_add(1, Ordering::Relaxed);
                    self.prune_expired(&mut entries, &key);
                    // A dirty load is detached, not cancelled: its waiters still
                    // get their read, and dropping the last of them ends it.
                    // Cancelling it made every waiter retry after a second.
                    let previous = entries.remove(&key).and_then(|entry| match entry {
                        CacheEntry::Ready(ready) => Some(ready),
                        CacheEntry::Loading {
                            previous, dirty, ..
                        } => previous.map(|mut ready| {
                            ready.dirty |= dirty;
                            ready
                        }),
                    });
                    let load_id = self.next_load_id.fetch_add(1, Ordering::Relaxed) + 1;
                    let cancellation = CancellationToken::new();
                    let loader_cancellation = cancellation.clone();
                    let future = async move {
                        tokio::select! {
                            result = loader() => result.map(Arc::new),
                            () = loader_cancellation.cancelled() => Err(AppError::cancelled()),
                        }
                    }
                    .boxed()
                    .shared();
                    entries.insert(
                        key.clone(),
                        CacheEntry::Loading {
                            load_id,
                            future: future.clone(),
                            cancellation,
                            previous,
                            dirty: false,
                            waiters: 1,
                        },
                    );
                    (future, load_id)
                }
            }
        };

        let _waiter = LoadWaiter {
            cache: self,
            key: &key,
            load_id,
        };
        let result = future.await;
        let mut entries = self.entries.lock().expect("live store cache lock");
        let Some(CacheEntry::Loading {
            load_id: active_load_id,
            dirty,
            ..
        }) = entries.get(&key)
        else {
            return result;
        };
        if *active_load_id != load_id {
            return result;
        }
        let dirty_while_loading = *dirty;
        if let Ok(value) = &result {
            let mut ready = Self::ready_value(value.clone());
            ready.dirty = dirty_while_loading;
            entries.insert(key.clone(), CacheEntry::Ready(ready));
            self.trim_to_budget(&mut entries);
        } else {
            let previous = entries.remove(&key).and_then(|entry| match entry {
                CacheEntry::Loading {
                    previous, dirty, ..
                } => previous.map(|mut ready| {
                    ready.dirty |= dirty;
                    ready
                }),
                CacheEntry::Ready(ready) => Some(ready),
            });
            if let Some(previous) = previous {
                self.counters
                    .restored_reload_failures
                    .fetch_add(1, Ordering::Relaxed);
                entries.insert(key.clone(), CacheEntry::Ready(previous));
                self.trim_to_budget(&mut entries);
            }
        }
        result
    }

    pub(super) fn mark_dirty(&self, key: &str) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        if let Some(entry) = entries.get_mut(key) {
            match entry {
                CacheEntry::Ready(ready) => ready.dirty = true,
                CacheEntry::Loading { dirty, .. } => *dirty = true,
            }
        }
    }

    pub(super) fn mark_dirty_where(&self, mut matches: impl FnMut(&str) -> bool) {
        let mut entries = self.entries.lock().expect("live store cache lock");
        for (key, entry) in &mut entries.by_key {
            if !matches(key) {
                continue;
            }
            match entry {
                CacheEntry::Ready(ready) => ready.dirty = true,
                CacheEntry::Loading { dirty, .. } => *dirty = true,
            }
        }
    }

    pub(super) fn cancel_loading(&self) -> usize {
        let mut entries = self.entries.lock().expect("live store cache lock");
        let loading_keys = entries
            .iter()
            .filter(|(_, entry)| matches!(entry, CacheEntry::Loading { .. }))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in &loading_keys {
            let previous = entries.remove(key).and_then(|entry| match entry {
                CacheEntry::Loading {
                    cancellation,
                    previous,
                    ..
                } => {
                    cancellation.cancel();
                    previous
                }
                CacheEntry::Ready(_) => None,
            });
            if let Some(mut previous) = previous {
                previous.dirty = true;
                entries.insert(key.clone(), CacheEntry::Ready(previous));
            }
        }
        loading_keys.len()
    }

    fn prune_expired(&self, entries: &mut CacheEntries<T>, requested_key: &str) {
        // Use the longest existing freshness window, without a new retention
        // multiplier. Keep the requested value available for failed reloads.
        entries.retain(|key, entry| {
            let evict = key != requested_key
                && matches!(entry,
                CacheEntry::Ready(ready) if ready.completed_at.elapsed() > MAX_FRESHNESS
                    && Arc::strong_count(&ready.value) == 1);
            if evict {
                self.counters.evictions.fetch_add(1, Ordering::Relaxed);
            }
            !evict
        });
    }

    fn trim_to_budget(&self, entries: &mut CacheEntries<T>) {
        while entries.ready_count > MAX_CACHE_ENTRIES {
            let victim = entries
                .iter()
                .filter_map(|(key, entry)| match entry {
                    CacheEntry::Ready(ready) => Some((key, ready.dirty, ready.completed_at)),
                    CacheEntry::Loading { .. } => None,
                })
                .min_by_key(|&(_, dirty, completed_at)| (dirty, completed_at))
                .map(|(key, _, _)| key.clone())
                .expect("an over-budget cache has a ready victim");
            entries.remove(&victim);
            self.counters.evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.lock().expect("live store cache lock").len()
    }

    /// Counts `Ready` entries directly, so budget tests can check the bookkeeping below.
    #[cfg(test)]
    pub(super) fn ready_len(&self) -> usize {
        self.entries
            .lock()
            .expect("live store cache lock")
            .values()
            .filter(|entry| matches!(entry, CacheEntry::Ready(_)))
            .count()
    }

    #[cfg(test)]
    pub(super) fn ready_count_bookkeeping(&self) -> usize {
        self.entries
            .lock()
            .expect("live store cache lock")
            .ready_count
    }

    #[cfg(test)]
    pub(super) fn has_key(&self, key: &str) -> bool {
        self.entries
            .lock()
            .expect("live store cache lock")
            .contains_key(key)
    }
}
