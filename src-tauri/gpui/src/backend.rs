use kubecove_lib::{
    commands::{self, BackendCancellationRegistry, ClusterAccess, ClusterLiveStore},
    models::{
        AppError, AppErrorKind, ClusterContext, NamespaceSummary, ResourceDetailsFull,
        ResourceSummary, ResourceTopology, WatchResourceKey, WatchResourceKind,
    },
    runtime::State,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub type Read<T> = Pin<Box<dyn Future<Output = Result<T, AppError>> + Send>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub context: String,
    pub namespace: Option<String>,
}

pub struct ScopeResult {
    pub namespaces: Vec<NamespaceSummary>,
    pub resources: Vec<ResourceSummary>,
    pub warnings: Vec<String>,
}

/// Both production commands and controlled UI-test responses use this seam.
/// Dropping a pending read must cancel its host-owned task.
pub trait Backend: Send + Sync + 'static {
    fn contexts(&self) -> Read<Vec<ClusterContext>>;
    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult>;
    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull>;
    /// The ownership graph for the scope's namespaces (all when unscoped).
    fn topology(&self, scope: Scope) -> Read<ResourceTopology>;
}

/// The native client reads the default workspace kinds; custom resource
/// discovery is not implemented yet.
pub use crate::model::WORKSPACE_KINDS as INSPECTION_KINDS;

pub struct NativeBackend {
    runtime: Option<tokio::runtime::Runtime>,
    store: ClusterLiveStore,
    cancellations: BackendCancellationRegistry,
}

impl NativeBackend {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            runtime: Some(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?,
            ),
            store: ClusterLiveStore::default(),
            cancellations: BackendCancellationRegistry::default(),
        })
    }

    fn read<T: Send + 'static>(
        &self,
        future: impl Future<Output = Result<T, AppError>> + Send + 'static,
    ) -> Read<T> {
        let task = self
            .runtime
            .as_ref()
            .expect("runtime exists until drop")
            .spawn(future);
        let guard = AbortOnDrop(task.abort_handle());
        Box::pin(async move {
            let _guard = guard;
            task.await.map_err(|error| {
                AppError::new(
                    format!("Backend task failed: {error}"),
                    AppErrorKind::Cluster,
                )
            })?
        })
    }
}

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Drop for NativeBackend {
    fn drop(&mut self) {
        self.cancellations.cancel_all();
        self.store.cancel_loading();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn supported(kind: &str) -> Result<(), AppError> {
    if INSPECTION_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(AppError::new(
            format!("Unsupported inspection kind: {kind}"),
            AppErrorKind::Validation,
        ))
    }
}

impl Backend for NativeBackend {
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        self.read(async { commands::list_kube_contexts(None) })
    }

    fn resources(&self, scope: Scope, refresh: bool) -> Read<ScopeResult> {
        let store = self.store.clone();
        self.read(async move {
            let kinds = INSPECTION_KINDS.to_vec();
            if refresh {
                let access = ClusterAccess::resolve(None)?;
                let keys: Vec<_> = kinds
                    .iter()
                    .map(|kind| WatchResourceKey {
                        resource_kind: WatchResourceKind {
                            kind: (*kind).to_owned(),
                            group: None,
                            version: None,
                            api_version: None,
                            plural: None,
                            namespaced: Some(true),
                        },
                        namespace: scope.namespace.clone(),
                    })
                    .collect();
                let namespaces: Vec<_> = scope.namespace.iter().cloned().collect();
                store.refresh_view(access.source_key(), &scope.context, &keys, &namespaces);
            }
            let mut warnings = Vec::new();
            let namespaces =
                match commands::list_namespaces(scope.context.clone(), None, State::new(&store))
                    .await
                {
                    Ok(rows) => Arc::unwrap_or_clone(rows),
                    Err(error) if error.kind == AppErrorKind::Forbidden => {
                        warnings.push(format!("Namespaces: {}", error.message));
                        Vec::new()
                    }
                    Err(error) => return Err(error),
                };
            let mut resources = Vec::new();
            for kind in kinds {
                match commands::list_resources(
                    scope.context.clone(),
                    kind.to_owned(),
                    scope.namespace.clone(),
                    None,
                    State::new(&store),
                )
                .await
                {
                    Ok(rows) => resources.extend(Arc::unwrap_or_clone(rows)),
                    Err(error) if error.kind == AppErrorKind::Forbidden => {
                        warnings.push(format!("{kind}: {}", error.message))
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(ScopeResult {
                namespaces,
                resources,
                warnings,
            })
        })
    }

    fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
        let cancellations = self.cancellations.clone();
        self.read(async move {
            supported(&resource.kind)?;
            commands::get_resource_details(
                resource.cluster,
                resource.kind,
                resource.name,
                resource.namespace,
                None,
                None,
                None,
                State::new(&cancellations),
            )
            .await
        })
    }

    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        let store = self.store.clone();
        let cancellations = self.cancellations.clone();
        self.read(async move {
            commands::list_resource_topology(
                scope.context,
                scope.namespace.into_iter().collect(),
                Some("ownership".into()),
                None,
                None,
                None,
                State::new(&store),
                State::new(&cancellations),
            )
            .await
            .map(Arc::unwrap_or_clone)
        })
    }
}
