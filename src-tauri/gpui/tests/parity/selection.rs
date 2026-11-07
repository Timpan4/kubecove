//! Which scenarios this run executes, and the shard-specific artifact names it writes.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use super::string;

/// `KUBECOVE_PARITY_SHARD=<index>/<count>`, set by `scripts/e2e-parity.ts` when it splits
/// the native run across processes.
pub(crate) fn shard() -> Option<(usize, usize)> {
    let value = std::env::var("KUBECOVE_PARITY_SHARD").ok()?;
    let parsed = value
        .split_once('/')
        .and_then(|(index, count)| Some((index.parse().ok()?, count.parse().ok()?)))
        .filter(|(index, count)| index < count);
    Some(parsed.unwrap_or_else(|| panic!("KUBECOVE_PARITY_SHARD must be <index>/<count>: {value}")))
}

/// Report and progress files carry the shard so parallel processes do not overwrite each other.
pub(crate) fn artifact_name(stem: &str, extension: &str) -> String {
    match shard() {
        Some((index, _)) => format!("{stem}-{index}.{extension}"),
        None => format!("{stem}.{extension}"),
    }
}

/// Every scenario file in name order, as `loadContract` in `e2e/parity/contract.ts` reads them.
/// `KUBECOVE_PARITY_SCENARIO_IDS` names a file with one scenario ID per line to run instead.
pub(crate) fn scenarios() -> Vec<Value> {
    let ids: Option<std::collections::HashSet<String>> =
        std::env::var_os("KUBECOVE_PARITY_SCENARIO_IDS").map(|path| {
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} unreadable: {error}", path.display()))
                .lines()
                .map(str::to_owned)
                .collect()
        });
    let prefix = std::env::var("KUBECOVE_PARITY_SCENARIO_PREFIX").unwrap_or_default();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/parity/scenarios");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} unreadable: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|file| {
            let contract: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            assert_eq!(contract["fixture"], "browser-mocks", "{}", file.display());
            assert_eq!(contract["context"], "mock-dev", "{}", file.display());
            contract["scenarios"].as_array().unwrap().clone()
        })
        .filter(|scenario| string(scenario, "id").starts_with(&prefix))
        .filter(|scenario| {
            ids.as_ref()
                .is_none_or(|ids| ids.contains(string(scenario, "id")))
        })
        .enumerate()
        .filter(|(index, _)| shard().is_none_or(|(shard, count)| index % count == shard))
        .map(|(_, scenario)| scenario)
        .collect()
}
