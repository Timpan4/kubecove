//! Pod log, Argo CD Application and Helm release tools.

use super::{access_for, args};
use crate::compact::{health_state, insert_some};
use kubecove_gitops::commands::{
    list_argocd_applications as argo_applications, list_helm_releases as helm_releases,
};
use kubecove_live::commands::read_pod_logs;
use kubecove_models::{AppError, ArgoApplicationSummary, HelmReleaseSummary, PodLogStreamRequest};
use serde_json::{Map, Value, json};

pub async fn get_pod_logs(arguments: args::PodLogsArgs) -> Result<Value, AppError> {
    let (_, cluster) = access_for(arguments.context)?;
    let read = read_pod_logs(
        PodLogStreamRequest {
            batch_lines: false,
            cluster_context: cluster.clone(),
            kubeconfig_env_var: None,
            namespace: arguments.namespace,
            pod_name: arguments.pod,
            container: arguments.container,
            tail_lines: arguments.tail_lines,
            since_seconds: arguments.since_seconds,
        },
        arguments.previous,
    )
    .await?;
    Ok(json!({
        "context": cluster,
        "container": read.container,
        "lineCount": read.lines.len(),
        "lines": read.lines,
    }))
}

pub async fn list_argocd_applications(arguments: args::ContextArgs) -> Result<Value, AppError> {
    let (_, cluster) = access_for(arguments.context)?;
    let applications = argo_applications(cluster.clone(), None).await?;
    Ok(json!({
        "context": cluster,
        "applications": applications.items.iter().map(application).collect::<Vec<_>>(),
    }))
}

fn application(app: &ArgoApplicationSummary) -> Value {
    let mut row = Map::new();
    row.insert("name".into(), app.name.clone().into());
    insert_some(&mut row, "namespace", app.namespace.clone());
    insert_some(&mut row, "project", app.project.clone());
    insert_some(&mut row, "syncStatus", app.sync_status.clone());
    insert_some(&mut row, "healthStatus", app.health_status.clone());
    row.insert(
        "health".into(),
        health_state(app.health_assessment.state).into(),
    );
    insert_some(&mut row, "operationPhase", app.operation_phase.clone());
    insert_some(
        &mut row,
        "destinationNamespace",
        app.destination_namespace.clone(),
    );
    insert_some(
        &mut row,
        "destinationServer",
        app.destination_server.clone(),
    );
    insert_some(&mut row, "sourceRepo", app.source_repo.clone());
    insert_some(&mut row, "sourceRevision", app.source_revision.clone());
    row.insert("age".into(), app.age.clone().into());
    Value::Object(row)
}

pub async fn list_helm_releases(arguments: args::ContextArgs) -> Result<Value, AppError> {
    let (_, cluster) = access_for(arguments.context)?;
    let list = helm_releases(cluster.clone(), None).await?;
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    body.insert(
        "releases".into(),
        Value::Array(list.releases.iter().map(release).collect()),
    );
    if !list.warnings.is_empty() {
        body.insert("warnings".into(), json!(list.warnings));
    }
    Ok(Value::Object(body))
}

fn release(release: &HelmReleaseSummary) -> Value {
    let mut row = Map::new();
    row.insert("name".into(), release.name.clone().into());
    row.insert("namespace".into(), release.namespace.clone().into());
    insert_some(&mut row, "chart", release.chart.clone());
    insert_some(&mut row, "appVersion", release.app_version.clone());
    insert_some(&mut row, "revision", release.revision);
    insert_some(&mut row, "status", release.status.clone());
    insert_some(&mut row, "updatedAt", release.updated_at.clone());
    Value::Object(row)
}
