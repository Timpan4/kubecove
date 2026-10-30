//! Desktop credential-process PATH recovery. Never changes the parent environment.

use kube::Config;
use std::{collections::HashMap, env, ffi::OsStr, path::Path, process::Stdio};
use tokio::{process::Command, sync::OnceCell};

const PATH_MARKER: &str = "\u{1e}KUBECOVE_PATH\u{1f}";
const SHELL_COMMAND: &str = "printf '\\036KUBECOVE_PATH\\037'; printenv PATH";

// Called only by the Linux/macOS desktop path; compile helpers on every platform.
#[allow(dead_code)]
pub(super) async fn apply(config: &mut Config) {
    static SHELL_PATH: OnceCell<Option<String>> = OnceCell::const_new();
    recover(config, env::var_os("PATH").as_deref(), async {
        SHELL_PATH
            .get_or_init(|| async {
                let shell = env::var_os("SHELL")?;
                if !Path::new(&shell).is_absolute() {
                    return None;
                }
                let mut command = Command::new(shell);
                command.args(["-ilc", SHELL_COMMAND]);
                if let Some(home) = env::var_os("HOME").filter(|home| Path::new(home).is_absolute())
                {
                    command.current_dir(home);
                }
                read_shell_path(&mut command).await
            })
            .await
            .clone()
    })
    .await;
}

async fn recover(
    config: &mut Config,
    inherited: Option<&OsStr>,
    shell: impl std::future::Future<Output = Option<String>>,
) {
    let Some(exec) = config.auth_info.exec.as_mut() else {
        return;
    };
    let explicit_path = exec
        .env
        .iter()
        .flatten()
        .any(|entry| entry.get("name").is_some_and(|name| name == "PATH"));
    if explicit_path || exec.drop_env.iter().flatten().any(|name| name == "PATH") {
        return;
    }
    let Some(shell) = shell.await else {
        return;
    };
    let mut directories = inherited
        .map(env::split_paths)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    for directory in env::split_paths(&shell) {
        if directory.is_absolute() && !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    let Ok(path) = env::join_paths(directories) else {
        return;
    };
    let Some(path) = path.to_str() else {
        return;
    };
    exec.env.get_or_insert_with(Vec::new).push(HashMap::from([
        ("name".into(), "PATH".into()),
        ("value".into(), path.into()),
    ]));
}

async fn read_shell_path(command: &mut Command) -> Option<String> {
    let output = tokio::time::timeout(
        super::clients::REQUEST_READ_TIMEOUT,
        command.stdin(Stdio::null()).kill_on_drop(true).output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let (_, value) = stdout.rsplit_once(PATH_MARKER)?;
    let path = value.lines().next()?;
    (!path.is_empty()).then(|| path.to_owned())
}

#[cfg(all(test, unix))]
#[path = "plugin_path_tests.rs"]
mod tests;
