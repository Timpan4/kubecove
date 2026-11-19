//! A test's own directory under `.t/` in the checkout, removed when the test ends on any path,
//! including a failed assertion.
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

pub struct Scratch(PathBuf);

impl Scratch {
    /// Create a private directory under the shared `.t/<area>` parent.
    pub fn new(area: &str, name: &str) -> Self {
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.t")
            .join(area);
        std::fs::create_dir_all(&parent)
            .unwrap_or_else(|error| panic!("creating {}: {error}", parent.display()));
        let path = parent.join(format!(
            "{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir(&path)
            .unwrap_or_else(|error| panic!("creating {}: {error}", path.display()));
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

#[cfg(test)]
mod tests {
    use super::Scratch;
    use std::{
        fs,
        panic::{catch_unwind, panic_any},
        path::PathBuf,
    };

    #[test]
    fn scratch_removes_its_directory_on_return_and_unwind() {
        let populated_scratch = || {
            let scratch = Scratch::new("scratch-tests", "cleanup");
            fs::write(scratch.join("fixture"), "test data").unwrap();
            let path = scratch.to_path_buf();
            (scratch, path)
        };

        let (scratch, path) = populated_scratch();
        drop(scratch);
        assert!(!path.try_exists().unwrap());

        let failure = catch_unwind(|| {
            let (_scratch, path) = populated_scratch();
            panic_any(path);
        })
        .unwrap_err();
        let path = failure.downcast::<PathBuf>().unwrap();
        assert!(!path.try_exists().unwrap());
    }

    #[test]
    fn scratch_keeps_the_original_panic_when_removal_fails() {
        let failure = catch_unwind(|| {
            let scratch = Scratch::new("scratch-tests", "missing");
            fs::remove_dir(&*scratch).unwrap();
            panic!("injected fixture failure");
        })
        .unwrap_err();
        assert_eq!(
            failure.downcast_ref::<&str>(),
            Some(&"injected fixture failure")
        );
    }

    #[test]
    fn scratch_rejects_an_existing_directory_without_touching_its_contents() {
        let scratch = Scratch::new("scratch-tests", "collision");
        let sentinel = scratch.join("fixture");
        fs::write(&sentinel, "existing data").unwrap();

        let collision = catch_unwind(|| Scratch::new("scratch-tests", "collision"));

        assert!(collision.is_err());
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "existing data");
    }
}
