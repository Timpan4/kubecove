//! One-time import of Tauri's `tauri-migration.json` (ADR 0036). GPUI imports
//! the first valid snapshot and records that in `live-sessions.json`, together
//! with the merged data. Later snapshots are ignored, and the snapshot file is
//! read-only input: GPUI never renames, moves or deletes it.

use super::{
    LockError, Store, acquire_lock, invalid, io_error, reject_credentials, validate, write_atomic,
};
use crate::preferences::{
    Document,
    tauri_migration::{self, MIGRATION_FILE, MergeReport},
};
use kubecove_lib::models::AppError;
use std::{collections::HashSet, fs, path::Path};

impl Store {
    /// Whether loading has import work: a snapshot GPUI has not imported, or
    /// presets an import left waiting for a kubeconfig source.
    pub(super) fn has_import_work(&self, directory: &Path, document: &Document) -> bool {
        if !document.tauri_migration_pending_presets.is_empty() {
            return true;
        }
        if document.tauri_migration_imported.is_some() {
            return false;
        }
        match directory.join(MIGRATION_FILE).try_exists() {
            Ok(exists) => exists,
            Err(error) => {
                self.set_migration_diagnostic(Some(format!(
                    "Could not inspect {MIGRATION_FILE}: {error}. The next start retries."
                )));
                false
            }
        }
    }

    pub(super) fn load_with_import(&self, directory: &Path) -> Result<Document, AppError> {
        let mut memory = self.memory.lock().map_err(|_| invalid())?;
        fs::create_dir_all(directory).map_err(|_| io_error())?;
        let _lock = match acquire_lock(directory) {
            Ok(lock) => lock,
            Err(error) => {
                let reason = match error {
                    LockError::Busy => "another instance holds the preferences lock".into(),
                    LockError::Io(error) => {
                        format!("live-sessions.lock could not be opened or locked: {error}")
                    }
                };
                self.set_migration_diagnostic(Some(format!(
                    "Skipped the Tauri import because {reason}. The next start retries."
                )));
                return self.load_unlocked(directory).map(|(document, _)| document);
            }
        };
        // Re-read under the lock: another instance may have imported meanwhile.
        let (mut document, existing_preference_keys) = self.load_unlocked(directory)?;
        let source_key = (self.source)();
        if document.tauri_migration_imported.is_none() {
            document =
                self.import_snapshot(directory, document, &existing_preference_keys, &source_key);
        }
        if !document.tauri_migration_pending_presets.is_empty() {
            document = self.file_pending_presets(document, &source_key);
        }
        *memory = document.clone();
        Ok(document)
    }

    fn import_snapshot(
        &self,
        directory: &Path,
        document: Document,
        existing_preference_keys: &HashSet<String>,
        source_key: &Result<String, AppError>,
    ) -> Document {
        let bytes = match fs::read(directory.join(MIGRATION_FILE)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return document,
            Err(error) => {
                self.set_migration_diagnostic(Some(format!(
                    "Could not read {MIGRATION_FILE}: {error}. The next start retries."
                )));
                return document;
            }
        };
        let mut candidate = document.clone();
        let report = match tauri_migration::merge_bytes(
            &bytes,
            &mut candidate,
            existing_preference_keys,
            source_key.as_deref().ok(),
        ) {
            Ok(report) => report,
            Err(reason) => {
                self.set_migration_diagnostic(Some(format!(
                    "Ignored corrupt {MIGRATION_FILE}: {reason}."
                )));
                return document;
            }
        };
        candidate.tauri_migration_imported = Some(report.exported_at.clone());
        match self.commit(candidate) {
            Some(committed) => {
                self.set_migration_diagnostic(import_summary(&report));
                committed
            }
            None => document,
        }
    }

    /// Files presets that waited for a kubeconfig source once it resolves.
    fn file_pending_presets(
        &self,
        document: Document,
        source_key: &Result<String, AppError>,
    ) -> Document {
        let source_key = match source_key {
            Ok(source_key) => source_key,
            Err(error) => {
                let waiting: usize = document
                    .tauri_migration_pending_presets
                    .values()
                    .map(Vec::len)
                    .sum();
                self.set_migration_diagnostic(Some(format!(
                    "{waiting} saved port forward(s) from the Tauri import wait for the active kubeconfig source, which could not be resolved ({}).",
                    error.message
                )));
                return document;
            }
        };
        let mut candidate = document.clone();
        tauri_migration::file_pending_presets(&mut candidate, source_key);
        self.commit(candidate).unwrap_or(document)
    }

    /// Validates and writes an import result with the next revision. On failure
    /// it records a diagnostic and returns `None`; nothing was written.
    fn commit(&self, mut candidate: Document) -> Option<Document> {
        let directory = self.directory.as_deref()?;
        if let Err(error) = validate(&candidate).and_then(|()| {
            let value = serde_json::to_value(&candidate).map_err(|_| invalid())?;
            reject_credentials(&value)
        }) {
            self.set_migration_diagnostic(Some(format!(
                "Did not import {MIGRATION_FILE}: the merged preferences failed validation ({}).",
                error.message
            )));
            return None;
        }
        let Some(revision) = candidate.revision.checked_add(1) else {
            self.set_migration_diagnostic(Some(format!(
                "Did not import {MIGRATION_FILE}: live-sessions.json has no revision left."
            )));
            return None;
        };
        candidate.revision = revision;
        if let Err(error) = write_atomic(directory, &candidate) {
            self.set_migration_diagnostic(Some(format!(
                "Could not write the Tauri import to live-sessions.json ({}). The next start retries.",
                error.message
            )));
            return None;
        }
        Some(candidate)
    }
}

fn import_summary(report: &MergeReport) -> Option<String> {
    let skipped = (report.skipped > 0)
        .then(|| format!("{} invalid or unsafe field(s) skipped", report.skipped));
    let kept = (report.kept_existing > 0).then(|| {
        format!(
            "{} Argo CD entr(ies) kept the existing GPUI version",
            report.kept_existing
        )
    });
    match (skipped, kept) {
        (None, None) => None,
        (Some(one), None) | (None, Some(one)) => {
            Some(format!("Imported {MIGRATION_FILE} with {one}."))
        }
        (Some(skipped), Some(kept)) => Some(format!(
            "Imported {MIGRATION_FILE} with {skipped} and {kept}."
        )),
    }
}
