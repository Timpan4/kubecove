//! Context, namespace, kind catalog and usage tools.

use super::{access_for, args};
use crate::{
    compact::{insert_some, page},
    handler::McpContext,
    kinds::{custom_kinds, table_kinds},
};
use kubecove_inspection::commands::{namespaces_summary_from, resource_metrics_from};
use kubecove_kube::commands::ClusterAccess;
use kubecove_models::{AppError, ResourceMetricSummary};
use serde_json::{Map, Value, json};

pub fn list_contexts() -> Result<Value, AppError> {
    let contexts = ClusterAccess::resolve(None)?.contexts()?;
    Ok(json!({
        "contexts": contexts
            .into_iter()
            .map(|context| json!({"name": context.name, "current": context.is_current}))
            .collect::<Vec<_>>(),
    }))
}

pub async fn list_namespaces(
    context: &McpContext,
    arguments: args::ContextArgs,
) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let loader_access = access.clone();
    let loader_cluster = cluster.clone();
    let namespaces = context
        .live_store
        .namespaces(
            access.source_key().to_owned(),
            cluster.clone(),
            move || async move { namespaces_summary_from(&loader_access, &loader_cluster).await },
        )
        .await?;
    Ok(json!({
        "context": cluster,
        "namespaces": namespaces
            .iter()
            .map(|namespace| json!({"name": namespace.name, "age": namespace.age}))
            .collect::<Vec<_>>(),
    }))
}

pub async fn list_resource_kinds(
    context: &McpContext,
    arguments: args::ListResourceKindsArgs,
) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let custom = custom_kinds(&access, &context.live_store, &cluster).await?;
    let query = arguments
        .query
        .map(|query| query.trim().to_ascii_lowercase())
        .filter(|query| !query.is_empty());
    let matches = |fields: &[&str]| {
        query.as_deref().is_none_or(|query| {
            fields
                .iter()
                .any(|field| field.to_ascii_lowercase().contains(query))
        })
    };
    let mut rows = Vec::new();
    for kind in table_kinds() {
        if matches(&[kind.kind, kind.plural, kind.group]) {
            rows.push(json!({
                "kind": kind.kind,
                "apiVersion": kind.api_version(),
                "plural": kind.plural,
                "namespaced": kind.namespaced,
                "builtin": true,
            }));
        }
    }
    for kind in custom.iter() {
        let short_names = kind.short_names.join(" ");
        if matches(&[&kind.kind, &kind.plural, &kind.group, &short_names]) {
            rows.push(json!({
                "kind": kind.kind,
                "apiVersion": kind.api_version,
                "plural": kind.plural,
                "shortNames": kind.short_names,
                "namespaced": kind.namespaced,
            }));
        }
    }
    let mut body = page(rows, arguments.limit, arguments.offset)?.into_json("kinds", |row| row);
    body.insert("context".into(), cluster.into());
    Ok(Value::Object(body))
}

pub async fn get_resource_metrics(arguments: args::MetricsArgs) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let namespaces = arguments.namespace.into_iter().collect();
    let summary = resource_metrics_from(&access, cluster.clone(), namespaces).await?;
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    body.insert("availability".into(), json!(summary.availability));
    for (key, mut rows) in [
        ("pods", summary.pods),
        ("workloads", summary.workloads),
        ("nodes", summary.nodes),
    ] {
        rows.sort_by(|left, right| {
            right
                .cpu_millicores
                .unwrap_or_default()
                .total_cmp(&left.cpu_millicores.unwrap_or_default())
        });
        let rows = page(rows, arguments.limit, arguments.offset)?;
        body.insert(
            key.into(),
            Value::Object(rows.into_json("items", |row| metric(&row))),
        );
    }
    if !summary.warnings.is_empty() {
        body.insert("warnings".into(), json!(summary.warnings));
    }
    Ok(Value::Object(body))
}

fn metric(row: &ResourceMetricSummary) -> Value {
    let mut item = Map::new();
    item.insert("kind".into(), row.kind.clone().into());
    item.insert("name".into(), row.name.clone().into());
    insert_some(&mut item, "namespace", row.namespace.clone());
    insert_some(&mut item, "cpuMillicores", row.cpu_millicores);
    insert_some(&mut item, "memoryBytes", row.memory_bytes);
    Value::Object(item)
}
