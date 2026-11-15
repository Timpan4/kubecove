use crate::models::AppError;
use crate::models::AppErrorKind;
use kube::config::{Cluster, Kubeconfig};
use std::{env, fs, path::PathBuf};

const E2E_ENV: &str = "KUBECOVE_E2E";
const KUBECONFIG_ENV: &str = "KUBECOVE_KUBECONFIG";
const DATA_DIR_ENV: &str = "KUBECOVE_DATA_DIR";
const CLUSTER_ENV: &str = "KUBECOVE_E2E_CLUSTER";

pub struct E2eConfig {
    pub data_dir: PathBuf,
}

pub(crate) fn is_enabled() -> bool {
    env::var(E2E_ENV).as_deref() == Ok("1")
}

pub(crate) fn kubeconfig_path() -> Result<PathBuf, AppError> {
    required_absolute_path(KUBECONFIG_ENV, false)
}

/// The desktop E2E flavor (ADR 0010): the kubeconfig must hold exactly the run cluster
/// named by `KUBECOVE_E2E_CLUSTER`, with its admin and restricted contexts.
pub fn startup_config() -> Result<E2eConfig, AppError> {
    config(ClusterRule::Required)
}

/// The native client's E2E flavor (ADR 0035). With `KUBECOVE_E2E_CLUSTER` set, the same
/// run-cluster rule as [`startup_config`] applies. Without it, any kubeconfig is accepted
/// whose every cluster is an HTTPS API server on loopback, such as a kind cluster.
pub fn pinned_config() -> Result<E2eConfig, AppError> {
    config(ClusterRule::Optional)
}

enum ClusterRule {
    Required,
    Optional,
}

fn config(rule: ClusterRule) -> Result<E2eConfig, AppError> {
    if !is_enabled() {
        return Err(AppError::new(
            "e2e feature requires KUBECOVE_E2E=1",
            AppErrorKind::Validation,
        ));
    }
    let kubeconfig = kubeconfig_path()?;
    let data_dir = required_absolute_path(DATA_DIR_ENV, true)?;
    let cluster = match rule {
        ClusterRule::Required => Some(required_value(CLUSTER_ENV)?),
        ClusterRule::Optional => env::var(CLUSTER_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty()),
    };
    let config = Kubeconfig::read_from(&kubeconfig).map_err(|error| {
        AppError::new("failed to load E2E kubeconfig", AppErrorKind::Validation).with_source(error)
    })?;
    match cluster {
        Some(cluster) => validate_kubeconfig(&config, &cluster)?,
        None => validate_loopback_clusters(&config)?,
    }
    Ok(E2eConfig { data_dir })
}

/// Every cluster must be an HTTPS API server on loopback, and there must be one.
fn validate_loopback_clusters(config: &Kubeconfig) -> Result<(), AppError> {
    if config.clusters.is_empty() {
        return Err(AppError::new(
            "E2E kubeconfig must contain a cluster",
            AppErrorKind::Validation,
        ));
    }
    for named in &config.clusters {
        validate_loopback_cluster(named.cluster.as_ref())?;
    }
    Ok(())
}

fn required_value(name: &str) -> Result<String, AppError> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::new(
                format!("{name} is required for E2E"),
                AppErrorKind::Validation,
            )
        })
}

fn required_absolute_path(name: &str, directory: bool) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(required_value(name)?);
    canonical_existing_path(name, path, directory)
}

fn canonical_existing_path(
    name: &str,
    path: PathBuf,
    directory: bool,
) -> Result<PathBuf, AppError> {
    if !path.is_absolute() {
        return Err(AppError::new(
            format!("{name} must be an absolute path"),
            AppErrorKind::Validation,
        ));
    }
    let metadata = fs::metadata(&path).map_err(|error| {
        AppError::new(
            format!("{name} path is unavailable"),
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        let kind = if directory {
            "directory"
        } else {
            "regular file"
        };
        return Err(AppError::new(
            format!("{name} must be a {kind}"),
            AppErrorKind::Validation,
        ));
    }
    let canonical = fs::canonicalize(&path).map_err(|error| {
        AppError::new(
            format!("{name} path is unavailable"),
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    Ok(canonical)
}

fn validate_kubeconfig(config: &Kubeconfig, cluster: &str) -> Result<(), AppError> {
    let expected_contexts = [format!("{cluster}-admin"), format!("{cluster}-restricted")];
    let mut actual_contexts = config
        .contexts
        .iter()
        .map(|context| context.name.as_str())
        .collect::<Vec<_>>();
    actual_contexts.sort_unstable();
    let mut expected_names = expected_contexts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    expected_names.sort_unstable();
    if actual_contexts != expected_names {
        return Err(AppError::new(
            "E2E kubeconfig must contain exactly admin and restricted contexts for this run",
            AppErrorKind::Validation,
        ));
    }
    if config.clusters.len() != 1 || config.clusters[0].name != cluster {
        return Err(AppError::new(
            "E2E kubeconfig must contain only the exact run cluster",
            AppErrorKind::Validation,
        ));
    }
    let mut actual_users = config
        .auth_infos
        .iter()
        .map(|user| user.name.as_str())
        .collect::<Vec<_>>();
    actual_users.sort_unstable();
    if actual_users != expected_names {
        return Err(AppError::new(
            "E2E kubeconfig must contain exactly admin and restricted users for this run",
            AppErrorKind::Validation,
        ));
    }
    for role in ["admin", "restricted"] {
        let context_name = format!("{cluster}-{role}");
        let context = config
            .contexts
            .iter()
            .find(|context| context.name == context_name)
            .and_then(|context| context.context.as_ref())
            .ok_or_else(|| {
                AppError::new(
                    format!("missing E2E context {context_name}"),
                    AppErrorKind::Validation,
                )
            })?;
        if context.cluster != cluster || context.user.as_deref() != Some(context_name.as_str()) {
            return Err(AppError::new(
                format!("E2E context {context_name} must target its exact cluster and user"),
                AppErrorKind::Validation,
            ));
        }
    }
    if config.current_context.as_deref() != Some(expected_contexts[0].as_str()) {
        return Err(AppError::new(
            "E2E kubeconfig current context must be the run admin context",
            AppErrorKind::Validation,
        ));
    }
    validate_loopback_cluster(
        config
            .clusters
            .iter()
            .find(|named| named.name == cluster)
            .and_then(|named| named.cluster.as_ref()),
    )
}

/// The cluster's API server must be HTTPS on loopback and reached directly. kube-rs
/// sends requests through a cluster's `proxy-url`, or else `HTTPS_PROXY`, where loopback
/// would name the proxy's host instead of this one.
fn validate_loopback_cluster(cluster: Option<&Cluster>) -> Result<(), AppError> {
    validate_direct(cluster, |name| env::var(name).ok())
}

fn validate_direct(
    cluster: Option<&Cluster>,
    variable: impl Fn(&str) -> Option<String>,
) -> Result<(), AppError> {
    let server = cluster
        .and_then(|cluster| cluster.server.as_deref())
        .ok_or_else(|| AppError::new("missing E2E cluster API server", AppErrorKind::Validation))?;
    validate_loopback_server(server)?;
    if cluster.is_some_and(|cluster| {
        cluster
            .proxy_url
            .as_deref()
            .is_some_and(|url| !url.is_empty())
    }) {
        return Err(AppError::new(
            "E2E cluster must not set proxy-url",
            AppErrorKind::Validation,
        ));
    }
    for name in ["HTTPS_PROXY", "https_proxy"] {
        if variable(name).is_some_and(|value| !value.is_empty()) {
            return Err(AppError::new(
                format!("unset {name} for E2E: the cluster must be reached directly"),
                AppErrorKind::Validation,
            ));
        }
    }
    Ok(())
}

fn validate_loopback_server(server: &str) -> Result<(), AppError> {
    let uri = server.parse::<http::Uri>().map_err(|error| {
        AppError::new(
            "E2E cluster API server must be a URL",
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    if uri.scheme_str() != Some("https") {
        return Err(AppError::new(
            "E2E cluster API server must use HTTPS",
            AppErrorKind::Validation,
        ));
    }
    let host = uri
        .host()
        .map(|value| value.trim_start_matches('[').trim_end_matches(']'))
        .ok_or_else(|| {
            AppError::new(
                "E2E cluster API server must have a host",
                AppErrorKind::Validation,
            )
        })?;
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return Err(AppError::new(
            "E2E cluster API server must use loopback",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> Kubeconfig {
        serde_yaml::from_str(
            "clusters:\n- name: run\n  cluster: { server: https://127.0.0.1:6443 }\ncontexts:\n- name: run-admin\n  context: { cluster: run, user: run-admin }\n- name: run-restricted\n  context: { cluster: run, user: run-restricted }\ncurrent-context: run-admin\nusers:\n- name: run-admin\n  user: { token: admin }\n- name: run-restricted\n  user: { token: restricted }\n",
        )
        .expect("config")
    }

    #[test]
    fn accepts_exact_loopback_config() {
        assert!(validate_kubeconfig(&valid_config(), "run").is_ok());
    }

    fn kind_config(servers: &[&str]) -> Kubeconfig {
        use std::fmt::Write as _;
        let mut clusters = String::new();
        for (index, server) in servers.iter().enumerate() {
            let _ = writeln!(
                clusters,
                "- name: c{index}\n  cluster: {{ server: {server} }}"
            );
        }
        serde_yaml::from_str(&format!(
            "clusters:\n{clusters}contexts:\n- name: kind-dev\n  context: {{ cluster: c0, user: dev }}\nusers:\n- name: dev\n  user: {{ token: dev }}\n"
        ))
        .expect("config")
    }

    #[test]
    fn pinned_flavor_accepts_any_context_names_on_loopback() {
        assert!(validate_loopback_clusters(&kind_config(&["https://127.0.0.1:41234"])).is_ok());
    }

    #[test]
    fn pinned_flavor_rejects_one_remote_cluster_among_loopback_ones() {
        let config = kind_config(&["https://127.0.0.1:41234", "https://prod.example.com"]);
        assert!(validate_loopback_clusters(&config).is_err());
    }

    #[test]
    fn pinned_flavor_rejects_plain_http_and_no_clusters() {
        assert!(validate_loopback_clusters(&kind_config(&["http://127.0.0.1:8080"])).is_err());
        let mut empty = kind_config(&["https://127.0.0.1:41234"]);
        empty.clusters.clear();
        assert!(validate_loopback_clusters(&empty).is_err());
    }

    /// A loopback server behind a remote proxy reaches the proxy's loopback, not this
    /// host's, so the flavor would reach whatever cluster the proxy host runs.
    #[test]
    fn pinned_flavor_rejects_a_loopback_server_behind_a_proxy() {
        let mut config = kind_config(&["https://127.0.0.1:41234"]);
        config.clusters[0]
            .cluster
            .as_mut()
            .expect("cluster")
            .proxy_url = Some("http://proxy.example.com:3128".into());
        assert!(validate_loopback_clusters(&config).is_err());

        let direct = kind_config(&["https://127.0.0.1:41234"]);
        let cluster = direct.clusters[0].cluster.as_ref();
        let proxied = |name: &str| (name == "https_proxy").then(|| "http://proxy:3128".to_owned());
        assert!(validate_direct(cluster, proxied).is_err());
        assert!(validate_direct(cluster, |_| None).is_ok());
    }

    #[test]
    fn rejects_lab_cluster_behind_a_proxy() {
        let mut config = valid_config();
        config.clusters[0]
            .cluster
            .as_mut()
            .expect("cluster")
            .proxy_url = Some("http://proxy.example.com:3128".into());
        assert!(validate_kubeconfig(&config, "run").is_err());
    }

    #[test]
    fn rejects_context_with_wrong_user() {
        let mut config = valid_config();
        config.contexts[0].context.as_mut().expect("context").user = Some("run-restricted".into());
        assert!(validate_kubeconfig(&config, "run").is_err());
    }

    #[test]
    fn rejects_non_loopback_server() {
        let mut config = valid_config();
        config.clusters[0].cluster.as_mut().expect("cluster").server =
            Some("https://example.com".into());
        assert!(validate_kubeconfig(&config, "run").is_err());
    }

    #[test]
    fn canonicalizes_existing_absolute_path() {
        let root = env::temp_dir().join(format!("kubecove-e2e-path-{}", std::process::id()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("temporary directory");
        let file = root.join("kubeconfig");
        fs::write(&file, "test").expect("temporary file");

        let result =
            canonical_existing_path(KUBECONFIG_ENV, nested.join("..").join("kubeconfig"), false)
                .expect("canonical path");
        let expected = fs::canonicalize(&file).expect("expected canonical path");

        let _ = fs::remove_dir_all(root);
        assert_eq!(result, expected);
    }
}
