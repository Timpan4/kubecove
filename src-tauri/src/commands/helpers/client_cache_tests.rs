use super::*;
use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_file(content: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = env::temp_dir().join(format!("kubecove-client-cache-{nanos}.yaml"));
    fs::write(&path, content).expect("write temp file");
    path
}

#[test]
fn lookup_returns_stored_value_for_matching_fingerprint() {
    let mut cache = Cache::new();
    cache.store("source".to_string(), "context".to_string(), 7, 42u32);

    assert_eq!(cache.lookup("source", "context", 7), Some(42));
    assert_eq!(cache.lookup("source", "other-context", 7), None);
    assert_eq!(cache.lookup("other-source", "context", 7), None);
}

#[test]
fn lookup_misses_when_fingerprint_changes() {
    let mut cache = Cache::new();
    cache.store("source".to_string(), "context".to_string(), 7, 42u32);

    assert_eq!(cache.lookup("source", "context", 8), None);

    cache.store("source".to_string(), "context".to_string(), 8, 43u32);
    assert_eq!(cache.lookup("source", "context", 8), Some(43));
}

#[test]
fn store_clears_cache_at_capacity_instead_of_growing() {
    let mut cache = Cache::new();
    for index in 0..MAX_ENTRIES {
        cache.store(format!("source-{index}"), "context".to_string(), 1, index);
    }
    assert_eq!(cache.entries.len(), MAX_ENTRIES);

    cache.store("overflow".to_string(), "context".to_string(), 1, 99);
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.lookup("overflow", "context", 1), Some(99));
}

#[test]
fn rotation_cancels_previous_generation_only() {
    let previous = finite_client_generation();
    let next_id = rotate_client_generation();
    let replacement = finite_client_generation();

    assert!(previous.token.is_cancelled());
    assert_eq!(replacement.id, next_id);
    assert!(!replacement.token.is_cancelled());
}

#[test]
fn fingerprint_changes_when_file_content_changes() {
    let path = temp_file("first");
    let before = fingerprint_files(std::slice::from_ref(&path));

    fs::write(&path, "second-longer-content").expect("rewrite temp file");
    let after = fingerprint_files(std::slice::from_ref(&path));

    let _ = fs::remove_file(&path);
    assert_ne!(before, after);
}

#[test]
fn fingerprint_distinguishes_missing_from_existing_file() {
    let path = temp_file("exists");
    let existing = fingerprint_files(std::slice::from_ref(&path));

    let _ = fs::remove_file(&path);
    let missing = fingerprint_files(std::slice::from_ref(&path));

    assert_ne!(existing, missing);
}
