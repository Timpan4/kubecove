//! Failure modes (ADR 0035): a release build starts under `KUBECOVE_E2E=1`; the E2E build
//! starts without it, with a kubeconfig that also reaches a remote cluster, or without an
//! automation port. Each case must exit before storage or a window. The child gets no
//! display and a worktree-local home, so a broken check cannot reach the desktop or the
//! user's settings.
use std::{path::PathBuf, process::Command};

const E2E_VARIABLES: [&str; 5] = [
    "KUBECOVE_E2E",
    "KUBECOVE_KUBECONFIG",
    "KUBECOVE_DATA_DIR",
    "KUBECOVE_E2E_CLUSTER",
    "KUBECOVE_AUTOMATION_PORT",
];

fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Runs the client binary and returns whether it succeeded and its standard error.
fn launch(variables: &[(&str, &str)]) -> (bool, String) {
    let home = scratch("e2e-interlock-home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_kubecove-gpui"));
    command
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &home);
    for variable in E2E_VARIABLES {
        command.env_remove(variable);
    }
    let output = command.envs(variables.iter().copied()).output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[cfg(not(feature = "e2e"))]
#[test]
fn a_build_without_the_e2e_feature_refuses_the_e2e_environment() {
    let (succeeded, stderr) = launch(&[("KUBECOVE_E2E", "1")]);
    assert!(!succeeded);
    assert!(
        stderr.contains("KUBECOVE_E2E=1 requires the e2e Cargo feature"),
        "{stderr}"
    );
}

#[cfg(feature = "e2e")]
fn kubeconfig(name: &str, servers: &[&str]) -> String {
    let clusters: String = servers
        .iter()
        .enumerate()
        .map(|(index, server)| format!("- name: c{index}\n  cluster: {{ server: {server} }}\n"))
        .collect();
    let path = scratch("e2e-interlock").join(name);
    std::fs::write(
        &path,
        format!("clusters:\n{clusters}contexts:\n- name: dev\n  context: {{ cluster: c0, user: dev }}\nusers:\n- name: dev\n  user: {{ token: dev }}\n"),
    )
    .unwrap();
    path.to_str().unwrap().to_owned()
}

#[cfg(feature = "e2e")]
#[test]
fn the_e2e_build_refuses_to_start_outside_its_environment() {
    let data = scratch("e2e-interlock-data");
    let data = data.to_str().unwrap();
    let local = kubeconfig("local", &["https://127.0.0.1:41234"]);
    let mixed = kubeconfig(
        "mixed",
        &["https://127.0.0.1:41234", "https://prod.example.com"],
    );

    let (succeeded, stderr) = launch(&[]);
    assert!(!succeeded);
    assert!(stderr.contains("requires KUBECOVE_E2E=1"), "{stderr}");

    let (succeeded, stderr) = launch(&[
        ("KUBECOVE_E2E", "1"),
        ("KUBECOVE_KUBECONFIG", &mixed),
        ("KUBECOVE_DATA_DIR", data),
        ("KUBECOVE_AUTOMATION_PORT", "0"),
    ]);
    assert!(!succeeded);
    assert!(stderr.contains("must use loopback"), "{stderr}");

    let (succeeded, stderr) = launch(&[
        ("KUBECOVE_E2E", "1"),
        ("KUBECOVE_KUBECONFIG", &local),
        ("KUBECOVE_DATA_DIR", data),
    ]);
    assert!(!succeeded);
    assert!(stderr.contains("KUBECOVE_AUTOMATION_PORT"), "{stderr}");
    assert!(
        !std::path::Path::new(data).join("automation-token").exists(),
        "a refused start wrote the token"
    );
}
