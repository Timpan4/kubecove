//! Resource topology, RBAC access review and Deployment revision tools.

use super::{access_for, args};
use crate::compact::{self, page};
use crate::{handler::McpContext, kinds::custom_kinds};
use kubecove_inspection::commands::{
    deployment_revisions_from, resource_topology_from, review_rbac_access_from,
};
use kubecove_models::{
    AppError, AppErrorKind, RbacAccessReviewOutcome, ResourceTopology, TopologyRelation,
};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn get_resource_topology(
    context: &McpContext,
    arguments: args::TopologyArgs,
) -> Result<Value, AppError> {
    let focus = match (arguments.focus_kind, arguments.focus_name) {
        (Some(kind), Some(name)) => Some((kind, name)),
        (None, None) => None,
        _ => {
            return Err(AppError::new(
                "focusKind and focusName go together",
                AppErrorKind::Validation,
            ));
        }
    };
    compact::page_limit(arguments.limit)?;
    let (access, cluster) = access_for(arguments.context)?;
    let mode = match arguments.mode {
        args::TopologyMode::Ownership => "ownership",
        args::TopologyMode::NetworkFlow => "networkFlow",
    };
    let namespaces = vec![arguments.namespace];
    // Network flow reads no custom kinds. For ownership, reuse the custom kinds the live
    // store already found present in this namespace, else take the cached kind catalog
    // `list_resources` uses and let the collection check which have objects here. A user
    // who may not list CustomResourceDefinitions still gets the built-in graph.
    let mut kinds_warning = None;
    let (custom_kinds, custom_kinds_are_present) = match arguments.mode {
        args::TopologyMode::NetworkFlow => (Vec::new(), true),
        args::TopologyMode::Ownership => match context
            .live_store
            .cached_present_custom_resource_kinds(access.source_key(), &cluster, &namespaces)
        {
            Some(kinds) => (Arc::unwrap_or_clone(kinds), true),
            None => match custom_kinds(&access, &context.live_store, &cluster).await {
                Ok(kinds) => (Arc::unwrap_or_clone(kinds), false),
                Err(error) if error.kind == AppErrorKind::Forbidden => {
                    kinds_warning =
                        Some(format!("custom resources are not shown: {}", error.message));
                    (Vec::new(), true)
                }
                Err(error) => return Err(error),
            },
        },
    };
    let mut topology = resource_topology_from(
        &access,
        cluster.clone(),
        namespaces,
        Some(mode.to_owned()),
        Some(custom_kinds),
        custom_kinds_are_present,
    )
    .await?;
    topology.warnings.extend(kinds_warning);
    let mut body = topology_body(
        topology,
        focus
            .as_ref()
            .map(|(kind, name)| (kind.as_str(), name.as_str())),
        TopologyPage {
            limit: arguments.limit,
            offset: arguments.offset,
            edge_offset: arguments.edge_offset,
        },
    )?;
    body.insert("context".into(), cluster.into());
    Ok(Value::Object(body))
}

#[derive(Clone, Copy, Default)]
pub struct TopologyPage {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub edge_offset: Option<usize>,
}

/// Nodes carry their index in the whole (focused) graph, and edges name nodes by that
/// index. Nodes and edges page separately, because one node can have any number of
/// edges.
pub fn topology_body(
    topology: ResourceTopology,
    focus: Option<(&str, &str)>,
    paging: TopologyPage,
) -> Result<Map<String, Value>, AppError> {
    let ResourceTopology {
        mut nodes,
        edges,
        warnings,
    } = topology;
    if let Some((kind, name)) = focus {
        let keep = connected_to(&nodes, &edges, kind, name)?;
        nodes = nodes
            .into_iter()
            .zip(keep)
            .filter_map(|(node, keep)| keep.then_some(node))
            .collect();
    }
    let positions: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.id.as_str(), position))
        .collect();
    let edge_indices: Vec<(usize, usize, TopologyRelation)> = edges
        .iter()
        .filter_map(|edge| {
            Some((
                *positions.get(edge.source.as_str())?,
                *positions.get(edge.target.as_str())?,
                edge.relation,
            ))
        })
        .collect();
    let indexed: Vec<_> = nodes.iter().enumerate().collect();
    let mut body =
        page(indexed, paging.limit, paging.offset)?.into_json("nodes", |(position, node)| {
            let mut row = match compact::resource(&node.summary) {
                Value::Object(row) => row,
                _ => Map::new(),
            };
            row.insert("index".into(), position.into());
            if !node.port_hints.is_empty() {
                row.insert("ports".into(), json!(node.port_hints));
            }
            Value::Object(row)
        });
    let edges = page(edge_indices, paging.limit, paging.edge_offset)?;
    body.insert("edgeTotal".into(), edges.total.into());
    if let Some(next) = edges.next_offset {
        body.insert("nextEdgeOffset".into(), next.into());
    }
    body.insert(
        "edges".into(),
        Value::Array(
            edges
                .items
                .into_iter()
                .map(|(source, target, relation)| {
                    json!({"from": source, "to": target, "relation": relation_name(relation)})
                })
                .collect(),
        ),
    );
    if !warnings.is_empty() {
        body.insert("warnings".into(), json!(warnings));
    }
    Ok(body)
}

/// Marks every node reachable from the focused resource, following edges both ways.
fn connected_to(
    nodes: &[kubecove_models::TopologyNode],
    edges: &[kubecove_models::TopologyEdge],
    kind: &str,
    name: &str,
) -> Result<Vec<bool>, AppError> {
    let positions: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.id.as_str(), position))
        .collect();
    let mut neighbours = vec![Vec::new(); nodes.len()];
    for edge in edges {
        if let (Some(&source), Some(&target)) = (
            positions.get(edge.source.as_str()),
            positions.get(edge.target.as_str()),
        ) {
            neighbours[source].push(target);
            neighbours[target].push(source);
        }
    }
    let mut keep = vec![false; nodes.len()];
    let mut pending: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind.eq_ignore_ascii_case(kind) && node.name == name)
        .map(|(position, _)| position)
        .collect();
    if pending.is_empty() {
        return Err(AppError::new(
            format!("{kind} {name} is not in the topology"),
            AppErrorKind::NotFound,
        ));
    }
    while let Some(position) = pending.pop() {
        if !std::mem::replace(&mut keep[position], true) {
            pending.extend(neighbours[position].iter().copied());
        }
    }
    Ok(keep)
}

fn relation_name(relation: TopologyRelation) -> &'static str {
    match relation {
        TopologyRelation::Owns => "owns",
        TopologyRelation::Creates => "creates",
        TopologyRelation::RoutesTo => "routesTo",
        TopologyRelation::Selects => "selects",
        TopologyRelation::Targets => "targets",
    }
}

pub async fn review_rbac_access(arguments: args::RbacReviewArgs) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let result = review_rbac_access_from(
        &access,
        &cluster,
        &arguments.identity.into(),
        &arguments.target.into(),
    )
    .await?;
    let mut body = Map::new();
    body.insert("context".into(), cluster.into());
    let outcome = match result.outcome {
        RbacAccessReviewOutcome::Allowed => "allowed",
        RbacAccessReviewOutcome::Denied => "denied",
        RbacAccessReviewOutcome::NoOpinion => "noOpinion",
    };
    body.insert("outcome".into(), outcome.into());
    compact::insert_some(&mut body, "reason", result.reason);
    compact::insert_some(&mut body, "evaluationError", result.evaluation_error);
    Ok(Value::Object(body))
}

pub async fn list_deployment_revisions(
    arguments: args::DeploymentRevisionsArgs,
) -> Result<Value, AppError> {
    let (access, cluster) = access_for(arguments.context)?;
    let client = access.read_client(&cluster).await?;
    let revisions =
        deployment_revisions_from(client, &arguments.name, &arguments.namespace).await?;
    let rows: Vec<Value> = revisions
        .into_iter()
        .map(|revision| {
            let mut row = Map::new();
            row.insert("replicaSet".into(), revision.name.into());
            compact::insert_some(&mut row, "revision", revision.revision);
            compact::insert_some(&mut row, "changeCause", revision.change_cause);
            compact::insert_some(&mut row, "createdAt", revision.created_at);
            row.insert("podTemplate".into(), revision.pod_template_yaml.into());
            Value::Object(row)
        })
        .collect();
    Ok(json!({"context": cluster, "revisions": rows}))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ingress -> Service -> Pod a, plus an unrelated Pod b.
    fn topology() -> ResourceTopology {
        let node = |kind: &str, name: &str| {
            json!({
                "id": format!("ctx/{kind}/shop/{name}"), "kind": kind, "name": name,
                "namespace": "shop", "status": null, "health": "healthy", "selectable": true,
                "summary": {"kind": kind, "cluster": "ctx", "name": name, "namespace": "shop",
                            "age": "1m"},
            })
        };
        let edge = |source: &str, target: &str, relation: &str| {
            json!({"id": format!("{source}->{target}"), "source": source, "target": target,
                   "relation": relation})
        };
        serde_json::from_value(json!({
            "nodes": [node("Ingress", "web"), node("Service", "web"), node("Pod", "a"),
                      node("Pod", "b")],
            "edges": [edge("ctx/Ingress/shop/web", "ctx/Service/shop/web", "routesTo"),
                      edge("ctx/Service/shop/web", "ctx/Pod/shop/a", "selects")],
            "warnings": [],
        }))
        .unwrap()
    }

    fn names(body: &Map<String, Value>) -> Vec<&str> {
        body["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["name"].as_str().unwrap())
            .collect()
    }

    // Failure modes: a focus keeps unrelated resources or drops ones reached through a
    // reversed edge; edge indices shift when a page or focus removes nodes; edges escape
    // the page limit or are skipped or repeated across edge pages; a focus that matches
    // nothing returns an empty graph instead of an error.
    #[test]
    fn focus_keeps_the_connected_resources_and_reindexes_edges() {
        let body = topology_body(topology(), Some(("pod", "a")), TopologyPage::default()).unwrap();
        assert_eq!(names(&body), ["web", "web", "a"]);
        assert_eq!(
            body["edges"],
            json!([{"from": 0, "to": 1, "relation": "routesTo"},
                   {"from": 1, "to": 2, "relation": "selects"}])
        );
        let error = topology_body(
            topology(),
            Some(("Pod", "missing")),
            TopologyPage::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::NotFound);
    }

    #[test]
    fn nodes_and_edges_page_separately_by_global_index() {
        let page = |offset, edge_offset| {
            topology_body(
                topology(),
                None,
                TopologyPage {
                    limit: Some(1),
                    offset: Some(offset),
                    edge_offset: Some(edge_offset),
                },
            )
            .unwrap()
        };
        let first = page(2, 0);
        assert_eq!(names(&first), ["a"]);
        assert_eq!(first["nodes"][0]["index"], 2);
        assert_eq!(first["nextOffset"], 3);
        assert_eq!(first["edgeTotal"], 2);
        assert_eq!(first["nextEdgeOffset"], 1);
        assert_eq!(
            first["edges"],
            json!([{"from": 0, "to": 1, "relation": "routesTo"}])
        );
        let second = page(2, 1);
        assert_eq!(
            second["edges"],
            json!([{"from": 1, "to": 2, "relation": "selects"}])
        );
        assert!(second.get("nextEdgeOffset").is_none());
    }
}
