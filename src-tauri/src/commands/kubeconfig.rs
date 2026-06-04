use crate::models::{AppError, ClusterContext};
use kube::{
    config::{KubeConfigOptions, Kubeconfig},
    Client, Config,
};
use std::{env, ffi::OsString, path::PathBuf};

pub const DEFAULT_KUBECONFIG_ENV_VAR: &str = "KUBECONFIG";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubeconfigSource {
    env_var: String,
}

impl KubeconfigSource {
    pub fn new(env_var: Option<String>) -> Result<Self, AppError> {
        Self::from_env_var(env_var.as_deref())
    }

    pub fn from_env_var(env_var: Option<&str>) -> Result<Self, AppError> {
        let env_var = normalize_env_var(env_var);
        validate_env_var_name(&env_var)?;
        Ok(Self { env_var })
    }

    pub fn key(&self) -> String {
        format!("kubeconfigEnv={}", self.env_var)
    }

    pub fn env_var(&self) -> &str {
        &self.env_var
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
        if self.env_var == DEFAULT_KUBECONFIG_ENV_VAR {
            return Ok(None);
        }

        let Some(value) = env::var_os(&self.env_var) else {
            return Ok(None);
        };
        Ok(split_non_empty_paths(value))
    }
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
