use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

pub fn write_startup_token(directory: &Path, token: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
        if !directory.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)?;
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "server data directory must be a private directory with mode 0700",
            ));
        }
        let temporary = directory.join(format!(
            ".server-token-{}",
            crate::security::random_token()?
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        if file.metadata()?.uid() != metadata.uid() {
            fs::remove_file(&temporary)?;
            return Err(io::Error::other(
                "server data directory must be owned by the current user",
            ));
        }
        let result = (|| {
            file.write_all(token.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, directory.join("server-token"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
    #[cfg(not(unix))]
    {
        let _ = (directory, token);
        // The server release targets are Unix. Windows desktop remote mode uses native storage.
        Err(io::Error::other(
            "headless server token files require Unix file permissions",
        ))
    }
}
