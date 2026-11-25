//! Pod log, Argo CD Application and Helm release tools.

use super::{access_for, args};
use crate::compact::{self, health_state, insert_some, page};
use kubecove_gitops::commands::{
    argocd_applications_from, helm_release_details_from, helm_release_from,
    helm_release_reconciliation_from, helm_releases_from,
};
use kubecove_live::commands::{MAX_LOG_READ_BYTES, read_pod_logs};
use kubecove_models::{
    AppError, ArgoApplicationSummary, HelmReconciliationResource, HelmReconciliationStatus,
    HelmReleaseSummary, PodLogStreamRequest,
};
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
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    body.insert("container".into(), read.container.into());
    body.insert("lineCount".into(), read.lines.len().into());
    body.insert("lines".into(), json!(read.lines));
    if read.truncated {
        body.insert("truncated".into(), true.into());
        body.insert(
            "truncatedReason".into(),
            format!(
                "older lines dropped or the newest line shortened to stay within \
                 {MAX_LOG_READ_BYTES} bytes"
            )
            .into(),
        );
    }
    Ok(Value::Object(body))
}

pub async fn list_argocd_applications(arguments: args::GitOpsListArgs) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let namespace = arguments
        .namespace
        .filter(|namespace| !namespace.is_empty());
    let mut applications = argocd_applications_from(&access, &cluster, namespace.as_deref())
        .await?
        .items;
    applications
        .sort_by(|left, right| (&left.namespace, &left.name).cmp(&(&right.namespace, &right.name)));
    let mut body = page(applications, arguments.limit, arguments.offset)
        .into_json("applications", |app| application(&app));
    body.insert("context".into(), cluster.into());
    Ok(Value::Object(body))
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

pub async fn list_helm_releases(arguments: args::GitOpsListArgs) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let namespace = arguments
        .namespace
        .filter(|namespace| !namespace.is_empty());
    let list = helm_releases_from(&access, &cluster, namespace.as_deref()).await?;
    let mut releases = list.releases;
    releases
        .sort_by(|left, right| (&left.namespace, &left.name).cmp(&(&right.namespace, &right.name)));
    let mut body = page(releases, arguments.limit, arguments.offset)
        .into_json("releases", |summary| release(&summary));
    body.insert("context".into(), cluster.into());
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

/// The latest revision's summary, value keys, manifest resources and live drift. Values
/// and manifest text never leave: they can hold credentials.
pub async fn get_helm_release(arguments: args::HelmReleaseArgs) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    // Reading only the release's namespace needs no cluster-wide storage access. The
    // storage reads use the requested namespace: a summary's namespace comes from the
    // decoded payload, which need not match where the storage object lives.
    let namespace = arguments.namespace.as_str();
    let (summary, mut warnings) =
        helm_release_from(&access, &cluster, namespace, &arguments.name).await?;
    let (storage_kind, storage_name) = (&summary.storage_kind, &summary.storage_name);
    let details = helm_release_details_from(
        &access,
        &cluster,
        namespace,
        storage_kind,
        storage_name,
        None,
        None,
    )
    .await?;
    let reconciliation =
        helm_release_reconciliation_from(&access, &cluster, namespace, storage_kind, storage_name)
            .await?;
    let manifest = &details.manifest_summary;
    let totals = &reconciliation.totals;
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    body.insert("release".into(), release(&details.summary));
    body.insert(
        "valueKeys".into(),
        json!(details.values_summary.top_level_keys),
    );
    body.insert(
        "valueCount".into(),
        details.values_summary.value_count.into(),
    );
    body.insert(
        "manifestResourceCount".into(),
        manifest.resource_count.into(),
    );
    if manifest.truncated {
        body.insert("manifestResourcesTruncated".into(), true.into());
    }
    body.insert(
        "totals".into(),
        json!({
            "tracked": totals.tracked,
            "unlabeledLive": totals.unlabeled_live,
            "missing": totals.missing,
            "labelOnly": totals.label_only,
            "unavailable": totals.unavailable,
        }),
    );
    let resources = page(
        reconciliation.resources.iter().collect(),
        arguments.limit,
        arguments.offset,
    );
    body.extend(resources.into_json("resources", drift));
    warnings.extend(reconciliation.warnings);
    if !warnings.is_empty() {
        body.insert("warnings".into(), json!(warnings));
    }
    Ok(Value::Object(body))
}

fn drift(resource: &HelmReconciliationResource) -> Value {
    let mut row = Map::new();
    insert_some(&mut row, "kind", resource.kind.clone());
    insert_some(&mut row, "name", resource.name.clone());
    insert_some(&mut row, "namespace", resource.namespace.clone());
    let status = match resource.status {
        HelmReconciliationStatus::Tracked => "tracked",
        HelmReconciliationStatus::UnlabeledLive => "unlabeledLive",
        HelmReconciliationStatus::Missing => "missing",
        HelmReconciliationStatus::LabelOnly => "labelOnly",
        HelmReconciliationStatus::Unavailable => "unavailable",
    };
    row.insert("status".into(), status.into());
    row.insert("message".into(), resource.status_message.clone().into());
    if let Some(live) = &resource.live_resource {
        row.insert("live".into(), compact::resource(live));
    }
    Value::Object(row)
}
