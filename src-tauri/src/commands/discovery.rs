use crate::{
    commands::ClusterLiveStore,
    models::{AppError, DiscoveredResourceKind},
};
use kube::{
    config::KubeConfigOptions,
    discovery::{verbs, Discovery, Scope},
    Client,
};
use std::time::Instant;
use tauri::State;

pub async fn resource_kinds_from(
    cluster_context: String,
) -> Result<Vec<DiscoveredResourceKind>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;
    let discovery = Discovery::new(client)
        .run_aggregated()
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let mut kinds = Vec::new();

    for group in discovery.groups() {
        for (api_resource, capabilities) in group.recommended_resources() {
            if !capabilities.supports_operation(verbs::LIST) {
                continue;
            }

            kinds.push(DiscoveredResourceKind {
                group: api_resource.group,
                version: api_resource.version,
                api_version: api_resource.api_version,
                kind: api_resource.kind,
                plural: api_resource.plural,
                namespaced: matches!(capabilities.scope, Scope::Namespaced),
            });
        }
    }

    kinds.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then(a.kind.cmp(&b.kind))
            .then(a.version.cmp(&b.version))
            .then(a.plural.cmp(&b.plural))
    });
    kinds.dedup_by(|a, b| {
        a.group == b.group && a.version == b.version && a.kind == b.kind && a.plural == b.plural
    });

    Ok(kinds)
}

#[tauri::command]
pub async fn list_resource_kinds(
    cluster_context: String,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Vec<DiscoveredResourceKind>, AppError> {
    let started = Instant::now();
    eprintln!(
        "[kubecove:backend] list_resource_kinds start context={}",
        cluster_context
    );
    let result = live_store
        .resource_kinds(cluster_context.clone(), {
            let cluster_context = cluster_context.clone();
            move || resource_kinds_from(cluster_context)
        })
        .await;
    match &result {
        Ok(rows) => eprintln!(
            "[kubecove:backend] list_resource_kinds done context={} rows={} ms={}",
            cluster_context,
            rows.len(),
            started.elapsed().as_millis()
        ),
        Err(err) => eprintln!(
            "[kubecove:backend] list_resource_kinds error context={} kind={} message={} ms={}",
            cluster_context,
            err.kind,
            err.message,
            started.elapsed().as_millis()
        ),
    }
    result
}
