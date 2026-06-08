use crate::models::{AppError, ClusterContext};
use kube::{
    config::{KubeConfigOptions, Kubeconfig},
    Client, Config,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    env,
    ffi::OsString,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::OnceLock,
};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

pub const DEFAULT_KUBECONFIG_ENV_VAR: &str = "KUBECONFIG";

static KUBECONFIG_SETTINGS_PATH: OnceLock<PathBuf> = OnceLock::new();

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
struct KubeconfigSettings {
    kubeconfig_env_var: String,
    paths: Vec<String>,
    show_source_labels: bool,
}

impl Default for KubeconfigSettings {
    fn default() -> Self {
        Self {
            kubeconfig_env_var: DEFAULT_KUBECONFIG_ENV_VAR.to_string(),
            paths: Vec::new(),
            show_source_labels: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubeconfigSource {
    env_var: String,
    paths: Vec<PathBuf>,
    show_source_labels: bool,
}

impl KubeconfigSource {
    pub fn new(env_var: Option<String>) -> Result<Self, AppError> {
        Self::from_env_var(env_var.as_deref())
    }

    pub fn from_env_var(env_var: Option<&str>) -> Result<Self, AppError> {
        let env_var = normalize_env_var(env_var);
        validate_env_var_name(&env_var)?;
        Ok(Self {
            env_var,
            paths: Vec::new(),
            show_source_labels: false,
        })
    }

    pub fn key(&self) -> String {
        if self.paths.is_empty() {
            format!("kubeconfigEnv={}", self.env_var)
        } else {
            format!(
                "kubeconfigSource={:016x}",
                kubeconfig_source_hash(&self.env_var, &self.paths)
            )
        }
    }

    pub fn env_var(&self) -> &str {
        &self.env_var
    }

    pub fn label(&self) -> String {
        if self.paths.is_empty() {
            self.env_var.clone()
        } else {
            format!(
                "{} + {} {}",
                self.env_var,
                self.paths.len(),
                if self.paths.len() == 1 {
                    "path"
                } else {
                    "paths"
                }
            )
        }
    }

    pub fn show_source_labels(&self) -> bool {
        self.show_source_labels
    }

    pub fn read_kubeconfig(&self) -> Result<Kubeconfig, AppError> {
        match self.custom_env_value_paths()? {
            Some(paths) => paths
                .into_iter()
                .try_fold(Kubeconfig::default(), |merged, path| {
                    let next = Kubeconfig::read_from(&path).map_err(|err| {
                        AppError::kube(format_kubeconfig_error(&self.env_var, err))
                    })?;
                    merged
                        .merge(next)
                        .map_err(|err| AppError::kube(format_kubeconfig_error(&self.env_var, err)))
                }),
            None => Kubeconfig::read().map_err(|err| AppError::kube(err.to_string())),
        }
    }

    pub async fn config_for_context(&self, cluster_context: &str) -> Result<Config, AppError> {
        let options = KubeConfigOptions {
            context: Some(cluster_context.to_string()),
            ..Default::default()
        };

        match self.custom_env_value_paths()? {
            Some(paths) => {
                let kubeconfig =
                    paths
                        .into_iter()
                        .try_fold(Kubeconfig::default(), |merged, path| {
                            let next = Kubeconfig::read_from(&path).map_err(|err| {
                                AppError::kube(format_kubeconfig_error(&self.env_var, err))
                            })?;
                            merged.merge(next).map_err(|err| {
                                AppError::kube(format_kubeconfig_error(&self.env_var, err))
                            })
                        })?;
                Config::from_custom_kubeconfig(kubeconfig, &options)
                    .await
                    .map_err(|err| AppError::kube(err.to_string()))
            }
            None => Config::from_kubeconfig(&options)
                .await
                .map_err(|err| AppError::kube(err.to_string())),
        }
    }

    pub async fn client_for_context(&self, cluster_context: &str) -> Result<Client, AppError> {
        let config = self.config_for_context(cluster_context).await?;
        Client::try_from(config).map_err(|err| AppError::kube(err.to_string()))
    }

    pub async fn client_and_default_namespace(
        &self,
        cluster_context: &str,
    ) -> Result<(Client, String), AppError> {
        let config = self.config_for_context(cluster_context).await?;
        let default_namespace = config.default_namespace.clone();
        let client = Client::try_from(config).map_err(|err| AppError::kube(err.to_string()))?;
        Ok((client, default_namespace))
    }

    fn custom_env_value_paths(&self) -> Result<Option<Vec<PathBuf>>, AppError> {
        if !self.paths.is_empty() {
            return Ok(Some(self.paths.clone()));
        }

        if self.env_var == DEFAULT_KUBECONFIG_ENV_VAR {
            return Ok(None);
        }

        let Some(value) = env::var_os(&self.env_var) else {
            return Ok(None);
        };
        Ok(split_non_empty_paths(value))
    }
}

pub fn init_kubeconfig_settings_path(app_config_dir: PathBuf) {
    let _ = KUBECONFIG_SETTINGS_PATH.set(app_config_dir.join("kubeconfig-sources.json"));
}

#[tauri::command]
pub fn get_kubeconfig_sources() -> Result<KubeconfigSourcesSummary, AppError> {
    summarize_settings(read_settings()?)
}

#[tauri::command]
pub fn set_kubeconfig_env_var(env_var: String) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = read_settings()?;
    settings.kubeconfig_env_var = normalize_env_var(Some(&env_var));
    validate_env_var_name(&settings.kubeconfig_env_var)?;
    write_settings(&settings)?;
    summarize_settings(settings)
}

#[tauri::command]
pub fn set_show_kubeconfig_source_labels(show: bool) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = read_settings()?;
    settings.show_source_labels = show;
    write_settings(&settings)?;
    summarize_settings(settings)
}

#[tauri::command]
pub fn pick_kubeconfig_paths(app: AppHandle) -> Result<KubeconfigSourcesSummary, AppError> {
    let paths = app
        .dialog()
        .file()
        .add_filter("Kubeconfig", &["yaml", "yml", "conf", "config"])
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| {
            path.as_path()
                .map(|path| path.to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    add_kubeconfig_paths(paths)
}

#[tauri::command]
pub fn add_kubeconfig_paths(paths: Vec<String>) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = read_settings()?;
    for path in paths {
        let trimmed = path.trim();
        if !trimmed.is_empty() && !settings.paths.iter().any(|existing| existing == trimmed) {
            settings.paths.push(trimmed.to_string());
        }
    }
    write_settings(&settings)?;
    summarize_settings(settings)
}

#[tauri::command]
pub fn remove_kubeconfig_path(path: String) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = read_settings()?;
    settings.paths.retain(|existing| existing != &path);
    write_settings(&settings)?;
    summarize_settings(settings)
}

#[tauri::command]
pub fn reorder_kubeconfig_paths(paths: Vec<String>) -> Result<KubeconfigSourcesSummary, AppError> {
    let mut settings = read_settings()?;
    let known = settings.paths;
    settings.paths = paths
        .into_iter()
        .filter(|path| known.iter().any(|known_path| known_path == path))
        .collect();
    for path in known {
        if !settings.paths.iter().any(|ordered| ordered == &path) {
            settings.paths.push(path);
        }
    }
    write_settings(&settings)?;
    summarize_settings(settings)
}

pub fn kubeconfig_source_key(kubeconfig_env_var: Option<&str>) -> Result<String, AppError> {
    Ok(KubeconfigSource::from_env_var(kubeconfig_env_var)?.key())
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

fn read_settings() -> Result<KubeconfigSettings, AppError> {
    let Some(path) = KUBECONFIG_SETTINGS_PATH.get() else {
        return Ok(KubeconfigSettings::default());
    };
    if !path.exists() {
        return Ok(KubeconfigSettings::default());
    }
    let text = fs::read_to_string(path).map_err(settings_error)?;
    serde_json::from_str(&text).map_err(settings_error)
}

fn write_settings(settings: &KubeconfigSettings) -> Result<(), AppError> {
    let Some(path) = KUBECONFIG_SETTINGS_PATH.get() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(settings_error)?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(settings_error)?;
    fs::write(path, text).map_err(settings_error)
}

fn summarize_settings(settings: KubeconfigSettings) -> Result<KubeconfigSourcesSummary, AppError> {
    validate_env_var_name(&settings.kubeconfig_env_var)?;
    let source = KubeconfigSource {
        env_var: settings.kubeconfig_env_var.clone(),
        paths: settings.paths.iter().map(PathBuf::from).collect(),
        show_source_labels: settings.show_source_labels,
    };
    Ok(KubeconfigSourcesSummary {
        kubeconfig_env_var: settings.kubeconfig_env_var,
        paths: settings
            .paths
            .into_iter()
            .map(|path| KubeconfigPathEntry { path })
            .collect(),
        source_key: source.key(),
        source_label: source.label(),
        show_source_labels: source.show_source_labels(),
        warnings: source_warnings(&source),
    })
}

fn source_warnings(source: &KubeconfigSource) -> Vec<KubeconfigSourceWarning> {
    source
        .paths
        .iter()
        .filter(|path| !path.exists())
        .map(|path| KubeconfigSourceWarning {
            source: source.key(),
            path: Some(path.to_string_lossy().to_string()),
            message: "kubeconfig path does not exist".to_string(),
        })
        .collect()
}

fn kubeconfig_source_hash(env_var: &str, paths: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    env_var.hash(&mut hasher);
    for path in paths {
        path.hash(&mut hasher);
    }
    hasher.finish()
}

fn settings_error(error: impl std::fmt::Display) -> AppError {
    AppError::new(
        format!("failed to persist kubeconfig settings: {error}"),
        "io",
    )
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
        let path = env::temp_dir().join(format!("kubecove-{context}.yaml"));
        let yaml = format!(
            r#"apiVersion: v1
kind: Config
current-context: {context}
clusters:
- name: cluster-{context}
  cluster:
    server: https://127.0.0.1
contexts:
- name: {context}
  context:
    cluster: cluster-{context}
    user: user-{context}
users:
- name: user-{context}
  user:
    token: test-token
"#
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

        assert_eq!(source.key(), format!("kubeconfigEnv={env_var}"));
        assert!(!source.key().contains('/'));
        assert!(source.custom_env_value_paths().expect("paths").is_none());
    }

    #[test]
    fn invalid_env_var_returns_validation_error() {
        let err = KubeconfigSource::new(Some("bad-name".to_string())).expect_err("invalid");

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("kubeconfig env var name"));
    }
}
