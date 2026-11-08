//! Per-scenario LLVM coverage for the parity coverage map (`e2e/parity/README.md`).
//!
//! `scripts/e2e-parity.ts` builds with `-C instrument-coverage --cfg kubecove_coverage` when
//! `PARITY_COVERAGE=1`. Each scenario then starts from zeroed counters and writes them to
//! `<artifacts>/coverage/gpui-<index>.profraw`. Normal builds compile these to no-ops.

use std::ffi::OsStr;

#[cfg(kubecove_coverage)]
mod runtime {
    use std::{
        ffi::{CString, OsStr, c_char, c_int},
        fs,
        path::{Path, PathBuf},
    };

    // The profiler runtime that `-C instrument-coverage` links.
    unsafe extern "C" {
        fn __llvm_profile_reset_counters();
        fn __llvm_profile_set_filename(name: *const c_char);
        fn __llvm_profile_write_file() -> c_int;
    }

    pub(super) fn reset() {
        // SAFETY: the runtime is linked by -C instrument-coverage; the call takes no arguments.
        unsafe { __llvm_profile_reset_counters() }
    }

    pub(super) fn write(artifacts: &OsStr, index: usize) {
        let dir = Path::new(artifacts).join("coverage");
        fs::create_dir_all(&dir).unwrap();
        let c_path = |path: PathBuf| {
            CString::new(path.to_str().expect("artifact path is UTF-8")).expect("no NUL")
        };
        let profile = c_path(dir.join(format!("gpui-{index}.profraw")));
        // The exit-time write goes back to the runner's LLVM_PROFILE_FILE, so it cannot
        // overwrite this scenario's file.
        let exit = c_path(
            std::env::var_os("LLVM_PROFILE_FILE")
                .map_or_else(|| dir.join("gpui-exit-%p.profraw"), PathBuf::from),
        );
        // SAFETY: both strings outlive the calls; the runtime copies the name pattern.
        let status = unsafe {
            __llvm_profile_set_filename(profile.as_ptr());
            let status = __llvm_profile_write_file();
            __llvm_profile_set_filename(exit.as_ptr());
            status
        };
        assert_eq!(status, 0, "cannot write coverage for parity scenario {index}");
    }
}

/// Zeroes the coverage counters before a scenario.
pub(crate) fn reset() {
    #[cfg(kubecove_coverage)]
    runtime::reset();
}

/// Writes the counters the scenario at `index` incremented.
pub(crate) fn write(artifacts: &OsStr, index: usize) {
    #[cfg(kubecove_coverage)]
    runtime::write(artifacts, index);
    #[cfg(not(kubecove_coverage))]
    let _ = (artifacts, index);
}
