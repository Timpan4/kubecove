//! Resource list, describe and YAML tools.

use super::{access_for, args};
use crate::{
    compact::{self, page},
    handler::McpContext,
    kinds::{ResolvedKind, resolve_kind},
};
use kubecove_inspection::commands::{
    dynamic_resource_details_from, resource_events_from, resource_yaml_from,
    resources::{
        PreparedResourceDetails, ResourceScopeResponse, dynamic_resource_details_document_from,
        resource_details_document_from, resource_scope_shared_from,
    },
};
use kubecove_kube::commands::ClusterAccess;
use kubecove_models::{AppError, AppErrorKind, ResourceSummary, YamlViewMode};
use serde_json::{Map, Value, json};

// kubectl stores the complete previous document here; it repeats the spec and can be large.
const LAST_APPLIED_ANNOTATION: &str = "kubectl.kubernetes.io/last-applied-configuration";

pub async fn list_resources(
    context: &McpContext,
    arguments: args::ListResourcesArgs,
) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let kind = resolve_kind(
        &access,
        &context.live_store,
        &cluster,
        &arguments.kind,
        arguments.api_version.as_deref(),
    )
    .await?;
    let request = kind.list_request(arguments.namespace);
    let scope = resource_scope_shared_from(
        &access,
        cluster.clone(),
        vec![request],
        context.live_store.clone(),
    )
    .await?;
    let name_filter = arguments
        .name_contains
        .map(|text| text.to_ascii_lowercase())
        .filter(|text| !text.is_empty());
    let mut body = page_resource_rows(&scope, arguments.limit, arguments.offset, |row| {
        name_filter
            .as_deref()
            .is_none_or(|text| row.name.to_ascii_lowercase().contains(text))
            && (arguments.health == args::HealthFilter::All || compact::is_unhealthy(row))
    })?;
    body.insert("context".into(), cluster.into());
    body.insert("kind".into(), kind.kind().into());
    Ok(Value::Object(body))
}

pub fn page_resource_rows(
    scope: &ResourceScopeResponse,
    limit: Option<usize>,
    offset: Option<usize>,
    filter: impl Fn(&ResourceSummary) -> bool,
) -> Result<Map<String, Value>, AppError> {
    let mut rows: Vec<_> = scope.rows().filter(|row| filter(row)).collect();
    rows.sort_by(|left, right| (&left.namespace, &left.name).cmp(&(&right.namespace, &right.name)));
    Ok(page(rows, limit, offset)?.into_json("resources", compact::resource))
}

struct Target {
    access: ClusterAccess,
    cluster: String,
    kind: ResolvedKind,
    name: String,
    namespace: Option<String>,
}

async fn target(context: &McpContext, arguments: args::ResourceArgs) -> Result<Target, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let kind = resolve_kind(
        &access,
        &context.live_store,
        &cluster,
        &arguments.kind,
        arguments.api_version.as_deref(),
    )
    .await?;
    let namespace = arguments.namespace.filter(|_| kind.namespaced());
    if kind.namespaced() && namespace.as_deref().is_none_or(str::is_empty) {
        return Err(AppError::new(
            format!("namespace is required for {}", kind.kind()),
            AppErrorKind::Validation,
        ));
    }
    Ok(Target {
        access,
        cluster,
        kind,
        name: arguments.name,
        namespace,
    })
}

async fn details(target: &Target) -> Result<PreparedResourceDetails, AppError> {
    match &target.kind {
        ResolvedKind::Builtin(kind) => {
            resource_details_document_from(
                &target.access,
                &target.cluster,
                kind.kind,
                &target.name,
                target.namespace.as_deref(),
            )
            .await
        }
        ResolvedKind::Custom(kind) => {
            dynamic_resource_details_document_from(
                &target.access,
                &target.cluster,
                kind,
                &target.name,
                target.namespace.as_deref(),
            )
            .await
        }
    }
}

pub async fn describe_resource(
    context: &McpContext,
    arguments: args::ResourceArgs,
) -> Result<Value, AppError> {
    let target = target(context, arguments).await?;
    let details = details(&target).await?;
    let uid = details
        .metadata
        .get("uid")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let events = resource_events_from(
        &target.access,
        target.cluster.clone(),
        target.kind.kind().to_owned(),
        target.name.clone(),
        target.namespace.clone(),
        uid,
    )
    .await;
    let mut body = describe_resource_body(target.cluster, details);
    match events {
        Ok(events) => {
            body.insert(
                "events".into(),
                Value::Array(events.iter().map(compact::event).collect()),
            );
        }
        // The resource was readable; missing Event access should not hide it.
        Err(error) if error.kind == AppErrorKind::Forbidden => {
            body.insert(
                "warnings".into(),
                json!([format!("events are not listed: {}", error.message)]),
            );
        }
        Err(error) => return Err(error),
    }
    Ok(Value::Object(body))
}

pub fn describe_resource_body(
    cluster: String,
    details: PreparedResourceDetails,
) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    body.insert("summary".into(), compact::resource(&details.summary));
    body.insert("metadata".into(), metadata(&details.metadata));
    if let Value::Object(mut fields) = details.document {
        for key in ["apiVersion", "kind", "metadata", "status"] {
            fields.remove(key);
        }
        body.insert("fields".into(), Value::Object(fields));
    }
    if let Some(status) = details.status {
        body.insert("status".into(), status);
    }
    body
}

pub async fn get_resource_yaml(
    context: &McpContext,
    arguments: args::ResourceArgs,
) -> Result<Value, AppError> {
    let target = target(context, arguments).await?;
    let yaml = match &target.kind {
        ResolvedKind::Builtin(kind) => {
            resource_yaml_from(
                &target.access,
                target.cluster.clone(),
                kind.kind.to_owned(),
                target.name.clone(),
                target.namespace.clone(),
                Some(YamlViewMode::Kubectl),
                None,
            )
            .await?
        }
        ResolvedKind::Custom(kind) => {
            dynamic_resource_details_from(
                &target.access,
                target.cluster.clone(),
                kind.clone(),
                target.name,
                target.namespace,
                Some(YamlViewMode::Kubectl),
                None,
            )
            .await?
            .yaml
        }
    };
    Ok(json!({"context": target.cluster, "yaml": yaml}))
}

fn metadata(metadata: &Value) -> Value {
    let mut kept = Map::new();
    for key in [
        "uid",
        "creationTimestamp",
        "deletionTimestamp",
        "generation",
        "labels",
        "finalizers",
    ] {
        if let Some(value) = metadata.get(key) {
            kept.insert(key.into(), value.clone());
        }
    }
    if let Some(Value::Object(annotations)) = metadata.get("annotations") {
        let annotations: Map<String, Value> = annotations
            .iter()
            .filter(|(key, _)| key.as_str() != LAST_APPLIED_ANNOTATION)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if !annotations.is_empty() {
            kept.insert("annotations".into(), Value::Object(annotations));
        }
    }
    if let Some(Value::Array(owners)) = metadata.get("ownerReferences") {
        let owners: Vec<Value> = owners
            .iter()
            .map(|owner| json!({"kind": owner.get("kind"), "name": owner.get("name")}))
            .collect();
        kept.insert("owners".into(), Value::Array(owners));
    }
    Value::Object(kept)
}
