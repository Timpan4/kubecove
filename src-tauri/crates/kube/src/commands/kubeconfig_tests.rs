use super::*;
use crate::commands::{ClusterAccess, SessionScope};
use crate::models::AppErrorKind;
use std::{
    ffi::OsStr,
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

/// Names the one test a child process runs; only `run_isolated` sets it.
const ISOLATED_TEST_ENV_VAR: &str = "KUBECOVE_ISOLATED_TEST";

/// Runs `body` in a child copy of this test binary that runs only `test`, on one
/// thread. The parallel test harness would otherwise let these environment writes
/// race other threads that read the environment.
fn run_isolated(test: &str, body: impl FnOnce()) {
    if env::var_os(ISOLATED_TEST_ENV_VAR).is_some_and(|name| name == test) {
        body();
        return;
    }
    let module = module_path!().split_once("::").map_or("", |(_, path)| path);
    let output = Command::new(env::current_exe().expect("test binary"))
        .args([&format!("{module}::{test}"), "--exact", "--test-threads=1"])
        .env(ISOLATED_TEST_ENV_VAR, test)
        .output()
        .expect("run isolated test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{test} failed in its isolated process:\n{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Declares a test whose body runs through `run_isolated`.
macro_rules! isolated_test {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            run_isolated(stringify!($name), || $body);
        }
    };
}

fn assert_isolated() {
    assert!(
        env::var_os(ISOLATED_TEST_ENV_VAR).is_some(),
        "environment writes belong in an isolated_test! body"
    );
}

fn set_env(name: &str, value: impl AsRef<OsStr>) {
    assert_isolated();
    // SAFETY: an isolated child process runs this one test on one thread.
    unsafe { env::set_var(name, value) };
}

fn remove_env(name: &str) {
    assert_isolated();
    // SAFETY: an isolated child process runs this one test on one thread.
    unsafe { env::remove_var(name) };
}

#[test]
fn server_sessions_cannot_write_shared_source_settings_without_forwarding() {
    let scope = super::super::helpers::session_scope::SessionScope::new(None);
    let error = scope
        .run_sync(|| save_persisted_sources(&PersistedKubeconfigSources::default()))
        .expect_err("server source settings belong to the operator");
    assert_eq!(error.kind, AppErrorKind::UnsupportedOperation);
}

fn unique_env_var(suffix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    format!("KUBECOVE_TEST_{suffix}_{nanos}")
}

fn write_kubeconfig(context: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = env::temp_dir().join(format!("kubecove-{context}-{nanos}.yaml"));
    let current_context_key = ["current", "-context"].concat();
    let clusters_key = ["cluster", "s"].concat();
    let contexts_key = ["context", "s"].concat();
    let users_key = ["user", "s"].concat();
    let credential_key = ["to", "ken"].concat();
    let credential_value = ["test-", &credential_key].concat();
    let yaml = format!(
        r"apiVersion: v1
kind: Config
{current_context_key}: {context}
{clusters_key}:
- name: cluster-{context}
  cluster:
    server: http://127.0.0.1:1
{contexts_key}:
- name: {context}
  context:
    cluster: cluster-{context}
    user: user-{context}
{users_key}:
- name: user-{context}
  user:
    {credential_key}: {credential_value}
"
    );
    fs::write(&path, yaml).expect("write kubeconfig");
    path
}

isolated_test!(custom_env_var_reads_selected_kubeconfig, {
    let env_var = unique_env_var("CUSTOM");
    let path = write_kubeconfig("custom-env-context");
    set_env(&env_var, &path);

    let access = ClusterAccess::resolve(Some(&env_var)).expect("access");
    let contexts = access.contexts().expect("contexts");

    let _ = fs::remove_file(path);
    assert_eq!(
        access.source_label(),
        Some(format!("{env_var} + 1 path").as_str())
    );
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].name, "custom-env-context");
    assert!(contexts[0].is_current);
});

isolated_test!(
    unset_custom_env_var_uses_default_loader_key_without_path_leak,
    {
        let env_var = unique_env_var("UNSET");
        remove_env(&env_var);

        let source = KubeconfigSource::from_persisted(Some(&env_var)).expect("source");

        assert!(source.key().starts_with("kubeconfigSource="));
        assert!(!source.key().contains(&env_var));
        assert!(!source.key().contains('/'));
        assert!(source.custom_env_value_paths().is_none());
    }
);

isolated_test!(
    unset_custom_env_uses_standard_kubeconfig_for_key_and_warnings,
    {
        let selected_env_var = unique_env_var("UNSET_WITH_STANDARD");
        let missing_standard = env::temp_dir().join(unique_env_var("MISSING_STANDARD"));
        let standard_path = write_kubeconfig("standard-context");
        let app_path = write_kubeconfig("app-context");
        remove_env(&selected_env_var);
        set_env(DEFAULT_KUBECONFIG_ENV_VAR, &missing_standard);
        let source = KubeconfigSource::from_settings(
            Some(&selected_env_var),
            vec![app_path.to_string_lossy().into_owned()],
            true,
        )
        .expect("source");

        let missing_key = source.key();
        let (_, warnings) = source
            .read_configured_kubeconfig(Fallback::Allow)
            .expect("app fallback");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].source, "env");
        assert!(warnings[0].message.contains(DEFAULT_KUBECONFIG_ENV_VAR));

        set_env(DEFAULT_KUBECONFIG_ENV_VAR, &standard_path);
        assert_ne!(source.key(), missing_key);
        let contexts =
            cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));
        assert_eq!(
            contexts
                .iter()
                .map(|context| context.name.as_str())
                .collect::<Vec<_>>(),
            vec!["standard-context", "app-context"]
        );

        let _ = fs::remove_file(standard_path);
        let _ = fs::remove_file(app_path);
    }
);

isolated_test!(env_and_app_paths_merge_in_order_without_key_path_leak, {
    let env_var = unique_env_var("MERGE");
    let env_path = write_kubeconfig("env-context");
    let app_path = write_kubeconfig("app-context");
    set_env(&env_var, &env_path);

    let source = KubeconfigSource::from_settings(
        Some(&env_var),
        vec![app_path.to_string_lossy().into_owned()],
        true,
    )
    .expect("source");
    let contexts = cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));
    let label = source.label();

    let _ = fs::remove_file(&env_path);
    let _ = fs::remove_file(&app_path);

    assert_eq!(label, format!("{env_var} + 2 paths"));
    assert!(
        !source
            .key()
            .contains(&env_path.to_string_lossy().into_owned())
    );
    assert!(
        !source
            .key()
            .contains(&app_path.to_string_lossy().into_owned())
    );
    assert_eq!(
        contexts
            .iter()
            .map(|context| context.name.as_str())
            .collect::<Vec<_>>(),
        vec!["env-context", "app-context"]
    );
});

isolated_test!(unset_env_and_app_paths_merge_platform_default_in_order, {
    let selected_env_var = unique_env_var("UNSET_WITH_APP");
    let home = env::temp_dir().join(unique_env_var("HOME"));
    let default_path = home.join(".kube").join("config");
    fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
        .expect("create default kubeconfig directory");
    let default_source = write_kubeconfig("default-context");
    fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
    let app_path = write_kubeconfig("app-context");
    remove_env(&selected_env_var);
    remove_env(DEFAULT_KUBECONFIG_ENV_VAR);
    set_env("HOME", &home);

    let source = KubeconfigSource::from_settings(
        Some(&selected_env_var),
        vec![app_path.to_string_lossy().into_owned()],
        true,
    )
    .expect("source");
    let contexts = cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));

    assert_eq!(
        contexts
            .iter()
            .map(|context| context.name.as_str())
            .collect::<Vec<_>>(),
        vec!["default-context", "app-context"]
    );
    assert_eq!(
        source.effective_kubeconfig_paths(),
        vec![default_path.clone(), app_path.clone()]
    );

    fs::write(&default_path, "not: [valid").expect("write malformed default kubeconfig");
    let (_, warnings) = source
        .read_configured_kubeconfig(Fallback::Allow)
        .expect("app fallback");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].source, "default");
    assert!(warnings[0].path.is_none());

    fs::remove_file(&default_path).expect("remove default kubeconfig");
    let (_, warnings) = source
        .read_configured_kubeconfig(Fallback::Allow)
        .expect("app fallback");
    assert!(warnings.is_empty());

    let _ = fs::remove_dir_all(home);
    let _ = fs::remove_file(default_source);
    let _ = fs::remove_file(app_path);
});

isolated_test!(missing_app_path_warns_and_falls_back_when_default_exists, {
    let home = env::temp_dir().join(unique_env_var("MISSING_APP_HOME"));
    let default_path = home.join(".kube").join("config");
    fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
        .expect("create default kubeconfig directory");
    let default_source = write_kubeconfig("default-context");
    fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
    let missing = env::temp_dir().join(unique_env_var("MISSING_APP"));
    remove_env(DEFAULT_KUBECONFIG_ENV_VAR);
    set_env("HOME", &home);
    let source = KubeconfigSource::from_settings(
        Some(DEFAULT_KUBECONFIG_ENV_VAR),
        vec![missing.to_string_lossy().into_owned()],
        true,
    )
    .expect("source");

    let (kubeconfig, warnings) = source
        .read_configured_kubeconfig(Fallback::Allow)
        .expect("default fallback");

    assert_eq!(
        kubeconfig.current_context.as_deref(),
        Some("default-context")
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.path.as_deref() == Some(missing.to_string_lossy().as_ref()))
    );

    let _ = fs::remove_dir_all(home);
    let _ = fs::remove_file(default_source);
});

#[test]
fn invalid_env_var_returns_validation_error() {
    let Err(err) = ClusterAccess::resolve(Some("bad-name")) else {
        panic!("an invalid variable name must be rejected");
    };

    assert_eq!(err.kind, AppErrorKind::Validation);
    assert!(err.message.contains("kubeconfig env var name"));
}

isolated_test!(e2e_source_ignores_inherited_override, {
    let expected = write_kubeconfig("expected-e2e-context");
    let override_path = write_kubeconfig("inherited-context");
    let standard_path = write_kubeconfig("standard-context");
    set_env(E2E_KUBECONFIG_SOURCE_ENV_VAR, &override_path);
    set_env(DEFAULT_KUBECONFIG_ENV_VAR, &standard_path);

    let source = KubeconfigSource::from_e2e_path(expected.clone());
    let contexts = cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].name, "expected-e2e-context");
    assert_eq!(source.effective_kubeconfig_paths(), vec![expected.clone()]);

    let _ = fs::remove_file(expected);
    let _ = fs::remove_file(override_path);
    let _ = fs::remove_file(standard_path);
});

// Failure modes: a cluster-changing intent that falls back to the default
// kubeconfig, a fail-closed intent served a client cached by a read that fell
// back, and a read or stream that stops falling back. The environment is set
// before the runtime starts its threads.
isolated_test!(
    cluster_changing_intents_fail_closed_when_configured_sources_do_not_load,
    {
        let home = env::temp_dir().join(unique_env_var("FAIL_CLOSED_HOME"));
        let default_path = home.join(".kube").join("config");
        fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
            .expect("create default kubeconfig directory");
        let default_source = write_kubeconfig("default-context");
        fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
        let missing = env::temp_dir().join(unique_env_var("FAIL_CLOSED_MISSING"));
        let env_var = unique_env_var("FAIL_CLOSED_ENV");
        set_env(&env_var, &missing);
        set_env("HOME", &home);
        let access = ClusterAccess::from_kubeconfig(
            KubeconfigSource::from_settings(Some(&env_var), Vec::new(), true).expect("source"),
        );

        crate::runtime::block_on(SessionScope::new(None).run(async {
            access
                .read_client("default-context")
                .await
                .expect("reads fall back to the default kubeconfig");
            access
                .stream_client("default-context")
                .await
                .expect("streams fall back to the default kubeconfig");
            for (intent, result) in [
                ("preview", access.preview_client("default-context").await),
                (
                    "confirmed operation",
                    access.confirmed_operation_client("default-context").await,
                ),
                (
                    "live session",
                    access.live_session_client("default-context").await,
                ),
            ] {
                let Err(error) = result else {
                    panic!("{intent} must not fall back to the default kubeconfig");
                };
                assert_eq!(error.kind, AppErrorKind::Kubeconfig, "{intent}");
                assert!(error.message.contains("refusing to fall back"), "{intent}");
            }
        }));

        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_file(default_source);
    }
);

fn context_names(access: &ClusterAccess) -> Vec<String> {
    access
        .contexts()
        .expect("contexts")
        .into_iter()
        .map(|context| context.name)
        .collect()
}

// Failure modes (ADR 0026): a server session that reads an environment
// variable a client named, or the kubeconfig files it lists; a server session
// that rejects the operator's own variable name or source key; and a desktop
// that loses its local environment variable selector.
isolated_test!(
    server_sessions_select_only_the_operator_source_while_the_desktop_selects_any,
    {
        let client_var = unique_env_var("CLIENT_SELECTED");
        let client_path = write_kubeconfig("client-selected-context");
        set_env(&client_var, &client_path);
        let operator_path = write_kubeconfig("operator-context");
        set_env(DEFAULT_KUBECONFIG_ENV_VAR, &operator_path);

        let desktop = ClusterAccess::resolve(Some(&client_var)).expect("desktop selector");
        assert_eq!(context_names(&desktop), vec!["client-selected-context"]);

        let server = SessionScope::new(None);
        let resolve = |selector: Option<&str>| {
            std::sync::Arc::clone(&server).run_sync(|| ClusterAccess::resolve(selector))
        };
        let configured = resolve(None).expect("operator source");
        assert_eq!(context_names(&configured), vec!["operator-context"]);
        for selector in [DEFAULT_KUBECONFIG_ENV_VAR, configured.source_key()] {
            let access = resolve(Some(selector)).expect(selector);
            assert_eq!(access.source_key(), configured.source_key(), "{selector}");
        }
        // The client variable lists a readable kubeconfig, so reading it would
        // resolve its context as the desktop did.
        for selector in [
            client_var.as_str(),
            "bad-name",
            "kubeconfigSource=0000000000000000",
        ] {
            let error = resolve(Some(selector)).expect_err(selector);
            assert_eq!(error.kind, AppErrorKind::Kubeconfig, "{selector}");
            assert!(error.message.contains("server operator"), "{selector}");
        }

        let _ = fs::remove_file(client_path);
        let _ = fs::remove_file(operator_path);
    }
);
