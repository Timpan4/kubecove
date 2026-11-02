//! The MCP bearer token file (ADR 0028). The token persists across restarts so agent
//! configuration stays valid, and is readable only by its owner on Unix.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{
    fs, io,
    io::Write as _,
    path::{Path, PathBuf},
};

pub const TOKEN_FILE_NAME: &str = "mcp-token";

/// Returns the token stored in `dir`, creating one when the file is missing or empty.
pub fn load_or_create_token(dir: &Path) -> io::Result<String> {
    let path = dir.join(TOKEN_FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(token) if !token.trim().is_empty() => {
            restrict(&path)?;
            Ok(token.trim().to_owned())
        }
        Ok(_) => write_new(&path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => write_new(&path),
        Err(error) => Err(error),
    }
}

/// Replaces the stored token. The previous token stops working once the host reloads it.
pub fn rotate_token(dir: &Path) -> io::Result<String> {
    write_new(&dir.join(TOKEN_FILE_NAME))
}

fn write_new(path: &Path) -> io::Result<String> {
    // ADR 0020 requires 32 bytes from the OS CSPRNG for server tokens; ADR 0028 reuses it.
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    // Write beside the target and rename, so a reader never sees a partial token.
    let staged = staged_path(path);
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&staged)?;
    restrict(&staged)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&staged, path)?;
    Ok(token)
}

fn staged_path(path: &Path) -> PathBuf {
    let mut staged = path.as_os_str().to_owned();
    staged.push(".tmp");
    PathBuf::from(staged)
}

/// Removes group and other access from an existing file, including one created before
/// the mode was applied or edited by hand.
#[cfg(unix)]
fn restrict(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o700))?;
    }
    Ok(())
}

// Windows keeps the file in the user's profile, whose ACL already excludes other users.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn restrict(_path: &Path) -> io::Result<()> {
    Ok(())
}
