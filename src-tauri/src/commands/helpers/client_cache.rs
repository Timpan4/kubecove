use kube::Client;
use std::{
    collections::hash_map::DefaultHasher,
    collections::HashMap,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{LazyLock, RwLock},
    time::UNIX_EPOCH,
};

// Building a kube Client re-reads kubeconfig files, redoes TLS setup, and
// starts a cold connection pool, so commands reuse one client per
// (source key, cluster context) while the backing kubeconfig files are
// unchanged. Fingerprints track file identity, length, and mtime so external
// kubeconfig rotation (for example cloud CLI credential refresh) invalidates
// the entry without re-reading file contents on every command.
struct Cache<T> {
    entries: HashMap<(String, String), (u64, T)>,
}

// Entries are keyed by hashed source state, so source changes strand old
// entries; the cap bounds that growth without an LRU dependency.
const MAX_ENTRIES: usize = 32;

impl<T: Clone> Cache<T> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn lookup(&self, source_key: &str, cluster_context: &str, fingerprint: u64) -> Option<T> {
        let (cached_fingerprint, value) = self
            .entries
            .get(&(source_key.to_string(), cluster_context.to_string()))?;
        if *cached_fingerprint == fingerprint {
            Some(value.clone())
        } else {
            None
        }
    }

    fn store(&mut self, source_key: String, cluster_context: String, fingerprint: u64, value: T) {
        let key = (source_key, cluster_context);
        if self.entries.len() >= MAX_ENTRIES && !self.entries.contains_key(&key) {
            self.entries.clear();
        }
        self.entries.insert(key, (fingerprint, value));
    }
}

type ClientEntry = (Client, String);

static CLIENTS: LazyLock<RwLock<Cache<ClientEntry>>> =
    LazyLock::new(|| RwLock::new(Cache::new()));

pub(crate) fn lookup_client(
    source_key: &str,
    cluster_context: &str,
    fingerprint: u64,
) -> Option<ClientEntry> {
    CLIENTS
        .read()
        .expect("client cache lock")
        .lookup(source_key, cluster_context, fingerprint)
}

pub(crate) fn store_client(
    source_key: String,
    cluster_context: String,
    fingerprint: u64,
    client: Client,
    default_namespace: String,
) {
    CLIENTS.write().expect("client cache lock").store(
        source_key,
        cluster_context,
        fingerprint,
        (client, default_namespace),
    );
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
