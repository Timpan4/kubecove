//! A test's own directory under `.t/` in the checkout, removed when the test ends on any path,
//! including a failed assertion.
use std::{
    io::ErrorKind,
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
        let stem = format!(
            "{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        );
        let mut path = parent.join(&stem);
        let mut suffix = 0_u64;
        loop {
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    suffix += 1;
                    path = parent.join(format!("{stem}-{suffix}"));
                }
                Err(error) => panic!("creating {}: {error}", path.display()),
            }
        }
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
    fn scratch_uses_a_fresh_directory_when_a_previous_run_left_one_behind() {
        // Keep a guard for the simulated stale directory so this test can clean it up.
        let stale = Scratch::new("scratch-tests", "collision");
        let sentinel = stale.join("fixture");
        fs::write(&sentinel, "previous run").unwrap();

        let fresh = Scratch::new("scratch-tests", "collision");
        assert_ne!(&*fresh, &*stale);
        let fresh_path = fresh.to_path_buf();
        fs::write(fresh.join("fixture"), "new run").unwrap();
        drop(fresh);

        assert!(!fresh_path.try_exists().unwrap());
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "previous run");
    }
}
