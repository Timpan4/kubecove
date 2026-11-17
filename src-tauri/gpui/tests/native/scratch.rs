//! A test's own directory under `.t/` in the checkout, removed when the test ends on any path,
//! including a failed assertion.
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

pub struct Scratch(PathBuf);

impl Scratch {
    /// `.t/<area>/<name>-<process>-<thread>`, emptied of anything an earlier run with the same
    /// process id left. The shared `.t/<area>` parent stays, because parallel tests create
    /// their directories in it.
    pub fn new(area: &str, name: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.t")
            .join(area)
            .join(format!(
                "{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let removed = std::fs::remove_dir_all(&self.0);
        // A second panic while a failed test unwinds would abort the whole test binary and
        // hide the first failure, so only a passing test fails on a cleanup error.
        if !std::thread::panicking() {
            removed.unwrap_or_else(|error| panic!("removing {}: {error}", self.0.display()));
        }
    }
}
