use super::*;
use crate::commands::{ClusterAccess, SessionScope};
use crate::models::AppErrorKind;
use std::{
    ffi::OsStr,
    fs,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn server_sessions_cannot_write_shared_source_settings_without_forwarding() {
    let scope = super::super::helpers::session_scope::SessionScope::new(None);
    let error = scope
        .run_sync(|| save_persisted_sources(&PersistedKubeconfigSources::default()))
        .expect_err("server source settings belong to the operator");
    assert_eq!(error.kind, AppErrorKind::UnsupportedOperation);
}

struct EnvVarGuard {
    name: String,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(name: impl Into<String>, value: impl AsRef<OsStr>) -> Self {
        let name = name.into();
        let previous = env::var_os(&name);
        env::set_var(&name, value);
        Self { name, previous }
    }

    fn unset(name: impl Into<String>) -> Self {
        let name = name.into();
        let previous = env::var_os(&name);
        env::remove_var(&name);
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.previous {
            env::set_var(&self.name, value);
        } else {
            env::remove_var(&self.name);
        }
    }
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

#[test]
fn custom_env_var_reads_selected_kubeconfig() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let env_var = unique_env_var("CUSTOM");
    let path = write_kubeconfig("custom-env-context");
    let _selected_env = EnvVarGuard::set(env_var.clone(), &path);

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
}

#[test]
fn unset_custom_env_var_uses_default_loader_key_without_path_leak() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let env_var = unique_env_var("UNSET");
    let _selected_env = EnvVarGuard::unset(env_var.clone());

    let source = KubeconfigSource::from_persisted(Some(&env_var)).expect("source");

    assert!(source.key().starts_with("kubeconfigSource="));
    assert!(!source.key().contains(&env_var));
    assert!(!source.key().contains('/'));
    assert!(source.custom_env_value_paths().is_none());
}

#[test]
fn unset_custom_env_uses_standard_kubeconfig_for_key_and_warnings() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let selected_env_var = unique_env_var("UNSET_WITH_STANDARD");
    let missing_standard = env::temp_dir().join(unique_env_var("MISSING_STANDARD"));
    let standard_path = write_kubeconfig("standard-context");
    let app_path = write_kubeconfig("app-context");
    let _selected_env = EnvVarGuard::unset(selected_env_var.clone());
    let _standard = EnvVarGuard::set(DEFAULT_KUBECONFIG_ENV_VAR, &missing_standard);
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

    env::set_var(DEFAULT_KUBECONFIG_ENV_VAR, &standard_path);
    assert_ne!(source.key(), missing_key);
    let contexts = cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));
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

#[test]
fn env_and_app_paths_merge_in_order_without_key_path_leak() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let env_var = unique_env_var("MERGE");
    let env_path = write_kubeconfig("env-context");
    let app_path = write_kubeconfig("app-context");
    let _selected_env = EnvVarGuard::set(env_var.clone(), &env_path);

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
    assert!(!source
        .key()
        .contains(&env_path.to_string_lossy().into_owned()));
    assert!(!source
        .key()
        .contains(&app_path.to_string_lossy().into_owned()));
    assert_eq!(
        contexts
            .iter()
            .map(|context| context.name.as_str())
            .collect::<Vec<_>>(),
        vec!["env-context", "app-context"]
    );
}

#[test]
fn unset_env_and_app_paths_merge_platform_default_in_order() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let selected_env_var = unique_env_var("UNSET_WITH_APP");
    let home = env::temp_dir().join(unique_env_var("HOME"));
    let default_path = home.join(".kube").join("config");
    fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
        .expect("create default kubeconfig directory");
    let default_source = write_kubeconfig("default-context");
    fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
    let app_path = write_kubeconfig("app-context");
    let _selected_env = EnvVarGuard::unset(selected_env_var.clone());
    let _kubeconfig = EnvVarGuard::unset(DEFAULT_KUBECONFIG_ENV_VAR);
    let _home = EnvVarGuard::set("HOME", &home);

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
}

#[test]
fn missing_app_path_warns_and_falls_back_when_default_exists() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let home = env::temp_dir().join(unique_env_var("MISSING_APP_HOME"));
    let default_path = home.join(".kube").join("config");
    fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
        .expect("create default kubeconfig directory");
    let default_source = write_kubeconfig("default-context");
    fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
    let missing = env::temp_dir().join(unique_env_var("MISSING_APP"));
    let _kubeconfig = EnvVarGuard::unset(DEFAULT_KUBECONFIG_ENV_VAR);
    let _home = EnvVarGuard::set("HOME", &home);
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
    assert!(warnings
        .iter()
        .any(|warning| warning.path.as_deref() == Some(missing.to_string_lossy().as_ref())));

    let _ = fs::remove_dir_all(home);
    let _ = fs::remove_file(default_source);
}

#[test]
fn invalid_env_var_returns_validation_error() {
    let Err(err) = ClusterAccess::resolve(Some("bad-name")) else {
        panic!("an invalid variable name must be rejected");
    };

    assert_eq!(err.kind, AppErrorKind::Validation);
    assert!(err.message.contains("kubeconfig env var name"));
}

#[test]
fn e2e_source_ignores_inherited_override() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let expected = write_kubeconfig("expected-e2e-context");
    let override_path = write_kubeconfig("inherited-context");
    let standard_path = write_kubeconfig("standard-context");
    let _override = EnvVarGuard::set(E2E_KUBECONFIG_SOURCE_ENV_VAR, &override_path);
    let _standard = EnvVarGuard::set(DEFAULT_KUBECONFIG_ENV_VAR, &standard_path);

    let source = KubeconfigSource::from_e2e_path(expected.clone());
    let contexts = cluster_contexts_from_kubeconfig(&source.read_kubeconfig().expect("kubeconfig"));

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].name, "expected-e2e-context");
    assert_eq!(source.effective_kubeconfig_paths(), vec![expected.clone()]);

    let _ = fs::remove_file(expected);
    let _ = fs::remove_file(override_path);
    let _ = fs::remove_file(standard_path);
}

// Failure modes: a cluster-changing intent that falls back to the default
// kubeconfig, a fail-closed intent served a client cached by a read that fell
// back, and a read or stream that stops falling back.
#[test]
fn cluster_changing_intents_fail_closed_when_configured_sources_do_not_load() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let home = env::temp_dir().join(unique_env_var("FAIL_CLOSED_HOME"));
    let default_path = home.join(".kube").join("config");
    fs::create_dir_all(default_path.parent().expect("default kubeconfig parent"))
        .expect("create default kubeconfig directory");
    let default_source = write_kubeconfig("default-context");
    fs::copy(&default_source, &default_path).expect("copy default kubeconfig");
    let missing = env::temp_dir().join(unique_env_var("FAIL_CLOSED_MISSING"));
    let env_var = unique_env_var("FAIL_CLOSED_ENV");
    let _kubeconfig = EnvVarGuard::set(&env_var, &missing);
    let _home = EnvVarGuard::set("HOME", &home);
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
#[test]
fn server_sessions_select_only_the_operator_source_while_the_desktop_selects_any() {
    let _env_lock = ENV_LOCK.lock().expect("environment lock");
    let client_var = unique_env_var("CLIENT_SELECTED");
    let client_path = write_kubeconfig("client-selected-context");
    let _client = EnvVarGuard::set(&client_var, &client_path);
    let operator_path = write_kubeconfig("operator-context");
    let _operator = EnvVarGuard::set(DEFAULT_KUBECONFIG_ENV_VAR, &operator_path);

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
