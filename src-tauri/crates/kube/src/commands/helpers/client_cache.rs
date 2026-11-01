use super::session_scope::SessionScope;
use kube::Client;
use std::{
    collections::hash_map::DefaultHasher,
    collections::{HashMap, VecDeque},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock, Weak},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

// Building a kube Client re-reads kubeconfig files, redoes TLS setup, and
// starts a cold connection pool, so commands reuse one client per
// (source key, cluster context) while the backing kubeconfig files are
// unchanged. Fingerprints track file identity, length, and mtime so external
// kubeconfig rotation (for example cloud CLI credential refresh) invalidates
// the entry without re-reading file contents on every command.
struct Cache<T> {
    entries: HashMap<(String, String), (u64, T)>,
    recency: VecDeque<(String, String)>,
}

// Entries are keyed by hashed source state, so source changes strand old
// entries; the cap bounds that growth without an LRU dependency.
const MAX_ENTRIES: usize = 32;

impl<T: Clone> Cache<T> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            recency: VecDeque::new(),
        }
    }

    fn lookup(&mut self, source_key: &str, cluster_context: &str, fingerprint: u64) -> Option<T> {
        let key = (source_key.to_string(), cluster_context.to_string());
        let (cached_fingerprint, value) = self.entries.get(&key)?;
        if *cached_fingerprint == fingerprint {
            let value = value.clone();
            self.recency.retain(|entry| entry != &key);
            self.recency.push_back(key);
            Some(value)
        } else {
            None
        }
    }

    fn store(&mut self, source_key: String, cluster_context: String, fingerprint: u64, value: T) {
        let key = (source_key, cluster_context);
        if self.entries.contains_key(&key) {
            self.recency.retain(|entry| entry != &key);
        } else if self.entries.len() >= MAX_ENTRIES {
            if let Some(oldest) = self.recency.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.recency.push_back(key.clone());
        self.entries.insert(key, (fingerprint, value));
    }
}

impl Cache<Client> {
    fn current_client(
        &mut self,
        source_key: &str,
        context: &str,
        fingerprint: u64,
    ) -> Option<Client> {
        self.lookup(source_key, context, fingerprint)
            .filter(|client| {
                client
                    .valid_until()
                    .as_ref()
                    .is_none_or(|expires| SystemTime::now() < SystemTime::from(*expires))
            })
    }
}

#[derive(Clone)]
pub(crate) struct ClientGeneration {
    pub(crate) id: u64,
    pub(crate) token: CancellationToken,
}

struct FiniteClientState {
    generation: ClientGeneration,
    clients: Cache<Client>,
}

impl FiniteClientState {
    fn new() -> Self {
        Self {
            generation: ClientGeneration {
                id: 0,
                token: CancellationToken::new(),
            },
            clients: Cache::new(),
        }
    }
}

/// The finite, operation and live client caches of one session scope, or of
/// the whole process outside any scope.
type BuildGates = HashMap<(String, String), Weak<tokio::sync::Mutex<()>>>;

pub(crate) struct ClientCaches {
    finite: RwLock<FiniteClientState>,
    operation: RwLock<Cache<Client>>,
    live: RwLock<Cache<Client>>,
    builds: Mutex<BuildGates>,
}

impl Default for ClientCaches {
    fn default() -> Self {
        Self {
            finite: RwLock::new(FiniteClientState::new()),
            operation: RwLock::new(Cache::new()),
            live: RwLock::new(Cache::new()),
            builds: Mutex::default(),
        }
    }
}

fn with_caches<R>(f: impl FnOnce(&ClientCaches) -> R) -> R {
    f(&SessionScope::current_or_process().caches)
}

pub(crate) fn client_build_gate(source_key: &str, context: &str) -> Arc<tokio::sync::Mutex<()>> {
    with_caches(|caches| {
        let mut builds = caches.builds.lock().expect("client build gates lock");
        builds.retain(|_, gate| gate.strong_count() > 0);
        let entry = builds
            .entry((source_key.to_owned(), context.to_owned()))
            .or_default();
        if let Some(gate) = entry.upgrade() {
            return gate;
        }
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        *entry = Arc::downgrade(&gate);
        gate
    })
}

pub(crate) fn finite_client_generation() -> ClientGeneration {
    with_caches(|caches| {
        caches
            .finite
            .read()
            .expect("finite client cache lock")
            .generation
            .clone()
    })
}

pub(crate) fn lookup_client(
    source_key: &str,
    cluster_context: &str,
    fingerprint: u64,
    generation: u64,
) -> Option<Client> {
    with_caches(|caches| {
        let mut state = caches.finite.write().expect("finite client cache lock");
        (state.generation.id == generation)
            .then(|| {
                state
                    .clients
                    .current_client(source_key, cluster_context, fingerprint)
            })
            .flatten()
    })
}

pub(crate) fn store_client(
    source_key: String,
    cluster_context: String,
    fingerprint: u64,
    generation: u64,
    client: Client,
) -> bool {
    with_caches(|caches| {
        let mut state = caches.finite.write().expect("finite client cache lock");
        if state.generation.id != generation {
            return false;
        }
        state
            .clients
            .store(source_key, cluster_context, fingerprint, client);
        true
    })
}

pub fn rotate_client_generation() -> u64 {
    with_caches(rotate_generation)
}

fn rotate_generation(caches: &ClientCaches) -> u64 {
    let (previous, next_id) = {
        let mut state = caches.finite.write().expect("finite client cache lock");
        let previous = state.generation.token.clone();
        let next_id = state.generation.id.saturating_add(1);
        state.generation = ClientGeneration {
            id: next_id,
            token: CancellationToken::new(),
        };
        state.clients.entries.clear();
        state.clients.recency.clear();
        (previous, next_id)
    };
    previous.cancel();
    next_id
}

pub(crate) fn lookup_operation_client(
    source_key: &str,
    cluster_context: &str,
    fingerprint: u64,
) -> Option<Client> {
    with_caches(|caches| {
        caches
            .operation
            .write()
            .expect("operation client cache lock")
            .current_client(source_key, cluster_context, fingerprint)
    })
}

pub(crate) fn store_operation_client(
    source_key: String,
    cluster_context: String,
    fingerprint: u64,
    client: Client,
) {
    with_caches(|caches| {
        caches
            .operation
            .write()
            .expect("operation client cache lock")
            .store(source_key, cluster_context, fingerprint, client);
    });
}

pub(crate) fn lookup_live_client(
    source_key: &str,
    cluster_context: &str,
    fingerprint: u64,
) -> Option<Client> {
    with_caches(|caches| {
        caches
            .live
            .write()
            .expect("live client cache lock")
            .current_client(source_key, cluster_context, fingerprint)
    })
}

pub(crate) fn store_live_client(
    source_key: String,
    cluster_context: String,
    fingerprint: u64,
    client: Client,
) {
    with_caches(|caches| {
        caches.live.write().expect("live client cache lock").store(
            source_key,
            cluster_context,
            fingerprint,
            client,
        );
    });
}

pub(crate) fn fingerprint_files(paths: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for path in paths {
        path.hash(&mut hasher);
        match fs::metadata(path) {
            Ok(metadata) => {
                metadata.len().hash(&mut hasher);
                let modified_nanos = metadata
                    .modified()
                    .ok()
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_nanos());
                modified_nanos.hash(&mut hasher);
            }
            Err(_) => false.hash(&mut hasher),
        }
    }
    hasher.finish()
}

#[cfg(test)]
#[path = "client_cache_tests.rs"]
mod client_cache_tests;

#[cfg(test)]
mod lru_tests {
    use super::*;

    // A plugin-issued client certificate must not be reused after expiry.
    #[tokio::test]
    async fn expired_clients_miss_in_every_cache_path() {
        SessionScope::new(None)
            .run(async {
                for expiry in [
                    None,
                    Some("2000-01-01T00:00:00Z"),
                    Some("2099-01-01T00:00:00Z"),
                ] {
                    let (service, _handle) = tower_test::mock::pair::<
                        http::Request<kube::client::Body>,
                        http::Response<kube::client::Body>,
                    >();
                    let client = Client::new(service, "default")
                        .with_valid_until(expiry.map(|value| value.parse().unwrap()));
                    let generation = finite_client_generation();
                    assert!(store_client(
                        "source".into(),
                        "context".into(),
                        1,
                        generation.id,
                        client.clone()
                    ));
                    store_operation_client("source".into(), "context".into(), 1, client.clone());
                    store_live_client("source".into(), "context".into(), 1, client);
                    let expected = expiry != Some("2000-01-01T00:00:00Z");
                    assert_eq!(
                        lookup_client("source", "context", 1, generation.id).is_some(),
                        expected
                    );
                    assert_eq!(
                        lookup_operation_client("source", "context", 1).is_some(),
                        expected
                    );
                    assert_eq!(
                        lookup_live_client("source", "context", 1).is_some(),
                        expected
                    );
                }
            })
            .await;
    }

    #[test]
    fn hit_refreshes_recency_but_fingerprint_miss_does_not() {
        let mut cache = Cache::new();
        for index in 0..MAX_ENTRIES {
            cache.store(format!("source-{index}"), "context".into(), 1, index);
        }
        assert_eq!(cache.lookup("source-0", "context", 1), Some(0));
        assert_eq!(cache.lookup("source-1", "context", 2), None);
        cache.store("overflow".into(), "context".into(), 1, MAX_ENTRIES);
        assert_eq!(cache.lookup("source-0", "context", 1), Some(0));
        assert_eq!(cache.lookup("source-1", "context", 1), None);
        assert_eq!(cache.lookup("source-2", "context", 1), Some(2));
    }

    #[tokio::test]
    async fn generation_rotation_clears_entries_and_recency_together() {
        let caches = ClientCaches::default();
        {
            let mut state = caches.finite.write().unwrap();
            let (service, _) = tower_test::mock::pair::<
                http::Request<kube::client::Body>,
                http::Response<kube::client::Body>,
            >();
            state.clients.store(
                "source".into(),
                "context".into(),
                1,
                Client::new(service, "default"),
            );
        }
        rotate_generation(&caches);
        let state = caches.finite.read().unwrap();
        assert!(state.clients.entries.is_empty());
        assert!(state.clients.recency.is_empty());
    }
}
