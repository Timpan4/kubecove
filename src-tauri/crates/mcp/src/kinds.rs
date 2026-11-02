//! Resolves the kind an agent names to a built-in table kind or a discovered custom
//! resource kind. Agents write kinds the way `kubectl` accepts them, so kind, plural,
//! singular, short name and `plural.group` all resolve.

use kubecove_inspection::commands::resource_kinds_from;
use kubecove_kube::commands::{
    ClusterAccess, ClusterLiveStore,
    builtin_kinds::{BUILTIN_KINDS, BuiltinKind},
};
use kubecove_models::{AppError, AppErrorKind, DiscoveredResourceKind, ResourceListRequest};

/// Kubernetes' own short names for the built-in table kinds.
const BUILTIN_SHORT_NAMES: [(&str, &str); 14] = [
    ("po", "Pod"),
    ("svc", "Service"),
    ("cm", "ConfigMap"),
    ("pvc", "PersistentVolumeClaim"),
    ("no", "Node"),
    ("pv", "PersistentVolume"),
    ("deploy", "Deployment"),
    ("rs", "ReplicaSet"),
    ("sts", "StatefulSet"),
    ("ds", "DaemonSet"),
    ("ing", "Ingress"),
    ("cj", "CronJob"),
    ("sc", "StorageClass"),
    ("crd", "CustomResourceDefinition"),
];

pub enum ResolvedKind {
    Builtin(&'static BuiltinKind),
    Custom(DiscoveredResourceKind),
}

impl ResolvedKind {
    pub fn kind(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.kind,
            Self::Custom(kind) => &kind.kind,
        }
    }

    pub fn namespaced(&self) -> bool {
        match self {
            Self::Builtin(kind) => kind.namespaced,
            Self::Custom(kind) => kind.namespaced,
        }
    }

    pub fn list_request(&self, namespace: Option<String>) -> ResourceListRequest {
        let namespace = namespace.filter(|_| self.namespaced());
        match self {
            Self::Builtin(kind) => ResourceListRequest {
                kind: Some(kind.kind.to_owned()),
                resource_kind: None,
                namespace,
            },
            Self::Custom(kind) => ResourceListRequest {
                kind: None,
                resource_kind: Some(kind.clone()),
                namespace,
            },
        }
    }
}

/// The built-in kinds `list_resources` serves.
pub fn table_kinds() -> impl Iterator<Item = &'static BuiltinKind> {
    BUILTIN_KINDS.iter().filter(|kind| kind.table)
}

/// Kubernetes' short names for one built-in kind.
pub fn builtin_short_names(kind: &BuiltinKind) -> Vec<&'static str> {
    BUILTIN_SHORT_NAMES
        .iter()
        .filter(|(_, name)| *name == kind.kind)
        .map(|(short, _)| *short)
        .collect()
}

fn names_builtin(kind: &BuiltinKind, query: &str) -> bool {
    let singular = kind.kind.to_ascii_lowercase();
    query.eq_ignore_ascii_case(kind.kind)
        || query.eq_ignore_ascii_case(kind.plural)
        || query == singular
        || builtin_short_names(kind).contains(&query)
        || (!kind.group.is_empty()
            && query.eq_ignore_ascii_case(&format!("{}.{}", kind.plural, kind.group)))
}

fn names_custom(kind: &DiscoveredResourceKind, query: &str) -> bool {
    query.eq_ignore_ascii_case(&kind.kind)
        || query.eq_ignore_ascii_case(&kind.plural)
        || kind
            .short_names
            .iter()
            .any(|short| query.eq_ignore_ascii_case(short))
        || query.eq_ignore_ascii_case(&format!("{}.{}", kind.plural, kind.group))
        || query.eq_ignore_ascii_case(&format!("{}.{}", kind.kind, kind.group))
}

/// Picks the one kind `query` names. An ambiguous name lists the candidates so the
/// agent can retry with `apiVersion`.
pub fn match_kind(
    query: &str,
    api_version: Option<&str>,
    catalog: &[DiscoveredResourceKind],
) -> Result<ResolvedKind, AppError> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Err(AppError::new("kind is required", AppErrorKind::Validation));
    }
    let version_matches = |candidate: &str| api_version.is_none_or(|wanted| wanted == candidate);
    if let Some(kind) = table_kinds()
        .find(|kind| names_builtin(kind, &query) && version_matches(&kind.api_version()))
    {
        return Ok(ResolvedKind::Builtin(kind));
    }
    let mut matches = catalog
        .iter()
        .filter(|kind| names_custom(kind, &query) && version_matches(&kind.api_version));
    match (matches.next(), matches.next()) {
        (Some(kind), None) => Ok(ResolvedKind::Custom(kind.clone())),
        (None, _) => Err(AppError::new(
            format!(
                "unknown kind {query}{}; call list_resource_kinds to see available kinds",
                api_version
                    .map(|version| format!(" in {version}"))
                    .unwrap_or_default()
            ),
            AppErrorKind::Validation,
        )),
        (Some(first), Some(second)) => {
            let candidates = [first, second]
                .into_iter()
                .chain(matches)
                .map(|kind| format!("{} ({})", kind.kind, kind.api_version))
                .collect::<Vec<_>>()
                .join(", ");
            Err(AppError::new(
                format!("kind {query} is ambiguous: {candidates}; pass apiVersion"),
                AppErrorKind::Validation,
            ))
        }
    }
}

/// The cluster's custom resource kinds, from the same cache the resource browser uses.
pub async fn custom_kinds(
    access: &ClusterAccess,
    live_store: &ClusterLiveStore,
    cluster_context: &str,
) -> Result<std::sync::Arc<Vec<DiscoveredResourceKind>>, AppError> {
    let loader_access = access.clone();
    let context = cluster_context.to_owned();
    live_store
        .resource_kinds(
            access.source_key().to_owned(),
            cluster_context.to_owned(),
            move || async move { resource_kinds_from(&loader_access, &context).await },
        )
        .await
}

/// Resolves `query`, reading the custom kind catalog only when no built-in kind matches.
pub async fn resolve_kind(
    access: &ClusterAccess,
    live_store: &ClusterLiveStore,
    cluster_context: &str,
    query: &str,
    api_version: Option<&str>,
) -> Result<ResolvedKind, AppError> {
    match match_kind(query, api_version, &[]) {
        Ok(kind) => Ok(kind),
        Err(error) if error.kind == AppErrorKind::Validation && !query.trim().is_empty() => {
            let catalog = custom_kinds(access, live_store, cluster_context).await?;
            match_kind(query, api_version, &catalog)
        }
        Err(error) => Err(error),
    }
}
