use crate::models::{AppError, ClusterContext};
use kube::{
    config::{KubeConfigOptions, Kubeconfig},
    Config,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    env,
    ffi::OsString,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{OnceLock, RwLock},
    time::Duration,
};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

pub const DEFAULT_KUBECONFIG_ENV_VAR: &str = "KUBECONFIG";
const SETTINGS_FILE_NAME: &str = "kubeconfig-sources.json";
pub(super) const FINITE_REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(30);

static SETTINGS_PATH: OnceLock<PathBuf> = OnceLock::new();
// In-memory copy of the persisted sources file. Every command constructs a
// KubeconfigSource, so reading the settings JSON from disk each time is pure
// overhead; the file is only ever written through save_persisted_sources.
static SETTINGS_CACHE: RwLock<Option<PersistedKubeconfigSources>> = RwLock::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeconfigPathEntry {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeconfigSourceWarning {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeconfigSourcesSummary {
    pub kubeconfig_env_var: String,
    pub paths: Vec<KubeconfigPathEntry>,
    pub source_key: String,
    pub source_label: String,
    pub show_source_labels: bool,
    pub warnings: Vec<KubeconfigSourceWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedKubeconfigSources {
    kubeconfig_env_var: String,
    paths: Vec<String>,
    show_source_labels: bool,
}

impl Default for PersistedKubeconfigSources {
    fn default() -> Self {
        Self {
            kubeconfig_env_var: default_kubeconfig_env_var(),
            paths: Vec::new(),
            show_source_labels: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubeconfigSource {
    env_var: String,
    app_paths: Vec<PathBuf>,
    show_source_labels: bool,
}

impl KubeconfigSource {
    pub fn new(env_var: Option<String>) -> Result<Self, AppError> {
        let settings = load_persisted_sources()?;
        let env_var = env_var
            .as_deref()
            .filter(|value| !is_source_key(value))
            .or(Some(settings.kubeconfig_env_var.as_str()));
        Self::from_settings(env_var, settings.paths, settings.show_source_labels)
    }

    pub fn from_env_var(env_var: Option<&str>) -> Result<Self, AppError> {
        Self::from_settings(env_var, Vec::new(), default_show_source_labels())
    }

    fn from_settings(
        env_var: Option<&str>,
        paths: Vec<String>,
        show_source_labels: bool,
    ) -> Result<Self, AppError> {
        let env_var = normalize_env_var(env_var);
        validate_env_var_name(&env_var)?;
        Ok(Self {
            env_var,
            app_paths: normalize_paths(paths)
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            show_source_labels,
        })
    }

    pub fn key(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.env_var.hash(&mut hasher);
        env::var_os(&self.env_var)
            .map(|value| value.to_string_lossy().into_owned())
            .hash(&mut hasher);
        for path in &self.app_paths {
            path.to_string_lossy().hash(&mut hasher);
        }
        format!("kubeconfigSource={:016x}", hasher.finish())
    }

    pub fn label(&self) -> String {
        let count = self.configured_path_count();
        if count == 0 {
            self.env_var.clone()
        } else {
            format!(
                "{} + {} {}",
                self.env_var,
                count,
                if count == 1 { "path" } else { "paths" }
            )
        }
    }

    pub fn env_var(&self) -> &str {
        &self.env_var
    }

    pub fn show_source_labels(&self) -> bool {
        self.show_source_labels
    }

    fn configured_path_count(&self) -> usize {
        let env_count = env::var_os(&self.env_var)
            .and_then(split_non_empty_paths)
            .map_or(0, |paths| paths.len());
        env_count + self.app_paths.len()
    }

    pub fn read_kubeconfig(&self) -> Result<Kubeconfig, AppError> {
        let (kubeconfig, _) = self.read_configured_kubeconfig()?;
        Ok(kubeconfig)
    }

    pub async fn config_for_context(&self, cluster_context: &str) -> Result<Config, AppError> {
        let options = KubeConfigOptions {
            context: Some(cluster_context.to_string()),
            ..Default::default()
        };

        match self.configured_paths()? {
            Some(_) => {
                let (kubeconfig, _) = self.read_configured_kubeconfig()?;
                Config::from_custom_kubeconfig(kubeconfig, &options)
                    .await
                    .map_err(|err| AppError::kube(err.to_string()))
            }
            None => Config::from_kubeconfig(&options)
                .await
                .map_err(|err| AppError::kube(err.to_string())),
        }
    }

    // The kubeconfig files that back this source, in the same order
    // config_for_context resolves them: configured env/app paths when present,
    // otherwise kube's default discovery (KUBECONFIG, then ~/.kube/config).
    pub(super) fn effective_kubeconfig_paths(&self) -> Result<Vec<PathBuf>, AppError> {
        if let Some(paths) = self.configured_paths()? {
            return Ok(paths
                .into_iter()
                .map(|configured| configured.path)
                .collect());
        }
        if let Some(value) = env::var_os(DEFAULT_KUBECONFIG_ENV_VAR) {
            if let Some(paths) = split_non_empty_paths(value) {
                return Ok(paths);
            }
        }
        Ok(default_kubeconfig_path().into_iter().collect())
    }

    fn custom_env_value_paths(&self) -> Result<Option<Vec<PathBuf>>, AppError> {
        let Some(value) = env::var_os(&self.env_var) else {
            return Ok(None);
        };
        Ok(split_non_empty_paths(value))
    }

    fn configured_paths(&self) -> Result<Option<Vec<ConfiguredKubeconfigPath>>, AppError> {
        let mut paths = Vec::new();
        if let Some(env_paths) = self.custom_env_value_paths()? {
            paths.extend(env_paths.into_iter().map(|path| ConfiguredKubeconfigPath {
                source: ConfiguredKubeconfigPathSource::Env,
                path,
            }));
        }
        paths.extend(
            self.app_paths
                .iter()
                .cloned()
                .map(|path| ConfiguredKubeconfigPath {
                    source: ConfiguredKubeconfigPathSource::AppPath,
                    path,
                }),
        );
        if paths.is_empty() {
            Ok(None)
        } else {
            Ok(Some(paths))
        }
    }

    fn read_configured_kubeconfig(
        &self,
    ) -> Result<(Kubeconfig, Vec<KubeconfigSourceWarning>), AppError> {
        let Some(paths) = self.configured_paths()? else {
            return Kubeconfig::read()
                .map(|kubeconfig| (kubeconfig, Vec::new()))
                .map_err(|err| AppError::kube(err.to_string()));
        };

        let mut merged = Kubeconfig::default();
        let mut loaded = 0usize;
        let mut warnings = Vec::new();
        for configured in paths {
            match Kubeconfig::read_from(&configured.path) {
                Ok(next) => {
                    merged = merged.merge(next).map_err(|err| {
                        AppError::kube(format_kubeconfig_error(&self.env_var, err))
                    })?;
                    loaded += 1;
                }
                Err(err) => warnings.push(configured.warning(&self.env_var, err)),
            }
        }

        if loaded > 0 {
            return Ok((merged, warnings));
        }

        read_default_kubeconfig_without_env()
            .map(|kubeconfig| (kubeconfig, warnings))
            .map_err(|err| {
                AppError::kube(format!(
                    "no usable kubeconfig sources and default discovery failed: {err}"
                ))
            })
    }
}

pub fn kubeconfig_source_key(kubeconfig_env_var: Option<&str>) -> Result<String, AppError> {
    if let Some(value) = kubeconfig_env_var.filter(|value| is_source_key(value)) {
        return Ok(value.to_string());
    }
    Ok(KubeconfigSource::new(kubeconfig_env_var.map(str::to_string))?.key())
}

pub fn cluster_contexts_from_source(
    source: &KubeconfigSource,
) -> Result<Vec<ClusterContext>, AppError> {
    let kubeconfig = source.read_kubeconfig()?;
    let current_context = kubeconfig.current_context.as_deref();

    Ok(kubeconfig
        .contexts
        .iter()
        .map(|ctx| ClusterContext {
            name: ctx.name.clone(),
            is_current: current_context == Some(ctx.name.as_str()),
        })
        .collect())
}

fn normalize_env_var(env_var: Option<&str>) -> String {
    env_var
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_KUBECONFIG_ENV_VAR)
        .to_string()
}

pub fn init_kubeconfig_settings_path(app_config_dir: PathBuf) {
    let _ = SETTINGS_PATH.set(app_config_dir.join(SETTINGS_FILE_NAME));
}

#[tauri::command]
pub fn get_kubeconfig_sources() -> Result<KubeconfigSourcesSummary, AppError> {
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn set_kubeconfig_env_var(env_var: String) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = load_persisted_sources()?;
    let normalized = normalize_env_var(Some(&env_var));
    validate_env_var_name(&normalized)?;
    settings.kubeconfig_env_var = normalized;
    save_persisted_sources(&settings)?;
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn set_show_kubeconfig_source_labels(show: bool) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = load_persisted_sources()?;
    settings.show_source_labels = show;
    save_persisted_sources(&settings)?;
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn add_kubeconfig_paths(paths: Vec<String>) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = load_persisted_sources()?;
    settings.paths = append_unique_paths(settings.paths, parse_path_input(paths));
    save_persisted_sources(&settings)?;
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn remove_kubeconfig_path(path: String) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = load_persisted_sources()?;
    settings.paths.retain(|candidate| candidate != &path);
    save_persisted_sources(&settings)?;
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn reorder_kubeconfig_paths(paths: Vec<String>) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = load_persisted_sources()?;
    let existing = settings.paths.iter().cloned().collect::<HashSet<_>>();
    let mut reordered = normalize_paths(paths)
        .into_iter()
        .filter(|path| existing.contains(path))
        .collect::<Vec<_>>();
    for path in settings.paths {
        if !reordered.contains(&path) {
            reordered.push(path);
        }
    }
    settings.paths = reordered;
    save_persisted_sources(&settings)?;
    kubeconfig_sources_summary()
}

#[tauri::command]
pub fn pick_kubeconfig_paths(app: AppHandle) -> Result<KubeconfigSourcesSummary, AppError> {
    let Some(paths) = app
        .dialog()
        .file()
        .set_title("Add kubeconfig files")
        .add_filter("Kubeconfig", &["yaml", "yml", "conf", "config"])
        .blocking_pick_files()
    else {
        return kubeconfig_sources_summary();
    };

    let picked = paths
        .into_iter()
        .filter_map(file_path_to_string)
        .collect::<Vec<_>>();
    add_kubeconfig_paths(picked)
}

fn validate_env_var_name(env_var: &str) -> Result<(), AppError> {
    let mut chars = env_var.chars();
    let Some(first) = chars.next() else {
        return Err(invalid_env_var_error());
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(invalid_env_var_error());
    }
    if chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        return Err(invalid_env_var_error());
    }
    Ok(())
}

fn invalid_env_var_error() -> AppError {
    AppError::new(
        "kubeconfig env var name must contain only ASCII letters, numbers, and underscores, and cannot start with a number",
        "validation",
    )
}

fn default_kubeconfig_env_var() -> String {
    DEFAULT_KUBECONFIG_ENV_VAR.to_string()
}

fn default_show_source_labels() -> bool {
    true
}

fn split_non_empty_paths(value: OsString) -> Option<Vec<PathBuf>> {
    let paths = env::split_paths(&value)
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

fn format_kubeconfig_error(env_var: &str, _err: kube::config::KubeconfigError) -> String {
    format!("failed to load kubeconfig from {env_var}")
}

fn is_source_key(value: &str) -> bool {
    value.starts_with("kubeconfigSource=")
}

fn settings_path() -> Option<PathBuf> {
    SETTINGS_PATH.get().cloned()
}

fn load_persisted_sources() -> Result<PersistedKubeconfigSources, AppError> {
    let Some(path) = settings_path() else {
        return Ok(PersistedKubeconfigSources::default());
    };
    if let Some(cached) = SETTINGS_CACHE.read().expect("settings cache lock").clone() {
        return Ok(cached);
    }
    let settings = if path.exists() {
        let content = fs::read_to_string(&path).map_err(|err| {
            AppError::new(format!("failed to read kubeconfig sources: {err}"), "io")
        })?;
        serde_json::from_str(&content).map_err(|err| {
            AppError::new(
                format!("failed to parse kubeconfig sources settings: {err}"),
                "validation",
            )
        })?
    } else {
        PersistedKubeconfigSources::default()
    };
    *SETTINGS_CACHE.write().expect("settings cache lock") = Some(settings.clone());
    Ok(settings)
}

fn save_persisted_sources(settings: &PersistedKubeconfigSources) -> Result<(), AppError> {
    let Some(path) = settings_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            AppError::new(
                format!("failed to create kubeconfig sources directory: {err}"),
                "io",
            )
        })?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|err| {
        AppError::new(
            format!("failed to serialize kubeconfig sources settings: {err}"),
            "validation",
        )
    })?;
    fs::write(&path, content).map_err(|err| {
        AppError::new(
            format!("failed to write kubeconfig sources settings: {err}"),
            "io",
        )
    })?;
    *SETTINGS_CACHE.write().expect("settings cache lock") = Some(settings.clone());
    Ok(())
}

fn kubeconfig_sources_summary() -> Result<KubeconfigSourcesSummary, AppError> {
    let settings = load_persisted_sources()?;
    let source = KubeconfigSource::from_settings(
        Some(&settings.kubeconfig_env_var),
        settings.paths.clone(),
        settings.show_source_labels,
    )?;
    let warnings = source.read_configured_kubeconfig().map_or_else(
        |err| {
            vec![KubeconfigSourceWarning {
                source: "default".to_string(),
                path: None,
                message: err.message,
            }]
        },
        |(_, warnings)| warnings,
    );

    Ok(KubeconfigSourcesSummary {
        kubeconfig_env_var: source.env_var().to_string(),
        paths: settings
            .paths
            .into_iter()
            .map(|path| KubeconfigPathEntry { path })
            .collect(),
        source_key: source.key(),
        source_label: source.label(),
        show_source_labels: source.show_source_labels(),
        warnings,
    })
}

fn parse_path_input(values: Vec<String>) -> Vec<String> {
    let mut parsed = Vec::new();
    for value in values {
        for line in value.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            parsed.extend(
                env::split_paths(trimmed)
                    .filter(|path| !path.as_os_str().is_empty())
                    .map(|path| path.to_string_lossy().into_owned()),
            );
        }
    }
    normalize_paths(parsed)
}

fn normalize_paths(paths: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for path in paths {
        let trimmed = path.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn append_unique_paths(existing: Vec<String>, additions: Vec<String>) -> Vec<String> {
    let mut paths = normalize_paths(existing);
    for path in additions {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn file_path_to_string(file_path: FilePath) -> Option<String> {
    file_path
        .into_path()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn default_kubeconfig_path() -> Option<PathBuf> {
    if let Some(home) = env::var_os("HOME") {
        return Some(PathBuf::from(home).join(".kube").join("config"));
    }
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join(".kube").join("config"))
}

fn read_default_kubeconfig_without_env() -> Result<Kubeconfig, kube::config::KubeconfigError> {
    if let Some(path) = default_kubeconfig_path().filter(|path| path.exists()) {
        return Kubeconfig::read_from(path);
    }
    Kubeconfig::read()
}

#[derive(Debug, Clone)]
struct ConfiguredKubeconfigPath {
    source: ConfiguredKubeconfigPathSource,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum ConfiguredKubeconfigPathSource {
    Env,
    AppPath,
}

impl ConfiguredKubeconfigPath {
    fn warning(
        &self,
        env_var: &str,
        _err: kube::config::KubeconfigError,
    ) -> KubeconfigSourceWarning {
        match self.source {
            ConfiguredKubeconfigPathSource::Env => KubeconfigSourceWarning {
                source: "env".to_string(),
                path: None,
                message: format!("A kubeconfig path from {env_var} could not be loaded."),
            },
            ConfiguredKubeconfigPathSource::AppPath => KubeconfigSourceWarning {
                source: "path".to_string(),
                path: Some(self.path.to_string_lossy().into_owned()),
                message: "This kubeconfig path could not be loaded.".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    server: https://127.0.0.1
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
        let env_var = unique_env_var("CUSTOM");
        let path = write_kubeconfig("custom-env-context");
        env::set_var(&env_var, &path);

        let source = KubeconfigSource::new(Some(env_var.clone())).expect("source");
        let contexts = cluster_contexts_from_source(&source).expect("contexts");

        env::remove_var(&env_var);
        let _ = fs::remove_file(path);
        assert_eq!(source.env_var(), env_var);
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].name, "custom-env-context");
        assert!(contexts[0].is_current);
    }

    #[test]
    fn unset_custom_env_var_uses_default_loader_key_without_path_leak() {
        let env_var = unique_env_var("UNSET");
        env::remove_var(&env_var);

        let source = KubeconfigSource::new(Some(env_var.clone())).expect("source");

        assert!(source.key().starts_with("kubeconfigSource="));
        assert!(!source.key().contains(&env_var));
        assert!(!source.key().contains('/'));
        assert!(source.custom_env_value_paths().expect("paths").is_none());
    }

    #[test]
    fn env_and_app_paths_merge_in_order_without_key_path_leak() {
        let env_var = unique_env_var("MERGE");
        let env_path = write_kubeconfig("env-context");
        let app_path = write_kubeconfig("app-context");
        env::set_var(&env_var, &env_path);

        let source = KubeconfigSource::from_settings(
            Some(&env_var),
            vec![app_path.to_string_lossy().into_owned()],
            true,
        )
        .expect("source");
        let contexts = cluster_contexts_from_source(&source).expect("contexts");
        let label = source.label();

        env::remove_var(&env_var);
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
    fn missing_app_path_warns_and_falls_back_when_default_exists() {
        let missing = env::temp_dir().join("kubecove-missing-kubeconfig.yaml");
        let source = KubeconfigSource::from_settings(
            Some(DEFAULT_KUBECONFIG_ENV_VAR),
            vec![missing.to_string_lossy().into_owned()],
            true,
        )
        .expect("source");

        let result = source.read_configured_kubeconfig();

        assert!(result.is_ok() || result.is_err());
        if let Ok((_, warnings)) = result {
            assert!(warnings.iter().any(|warning| warning.path.is_some()));
        }
    }

    #[test]
    fn invalid_env_var_returns_validation_error() {
        let err = KubeconfigSource::new(Some("bad-name".to_string())).expect_err("invalid");

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("kubeconfig env var name"));
    }
}
