//! Resource topology, RBAC access review and Deployment revision tools.

use super::{access_for, args};
use crate::compact::{self, page};
use crate::{handler::McpContext, kinds::custom_kinds};
use kubecove_inspection::commands::{
    list_deployment_revisions as deployment_revisions, resource_topology_from,
    review_rbac_access_from,
};
use kubecove_models::{
    AppError, AppErrorKind, RbacAccessReviewOutcome, RbacAccessReviewRequest, ResourceTopology,
    TopologyRelation,
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
    let (access, cluster) = access_for(arguments.context)?;
    let mode = match arguments.mode {
        args::TopologyMode::Ownership => "ownership",
        args::TopologyMode::NetworkFlow => "networkFlow",
    };
    let namespaces = vec![arguments.namespace];
    // Reuse the custom kinds the live store already found present in this namespace. Else
    // take the cached kind catalog `list_resources` uses, and let the collection check
    // which of those kinds have objects here.
    let (custom_kinds, custom_kinds_are_present) = match context
        .live_store
        .cached_present_custom_resource_kinds(access.source_key(), &cluster, &namespaces)
    {
        Some(kinds) => (kinds, true),
        None => (
            custom_kinds(&access, &context.live_store, &cluster).await?,
            false,
        ),
    };
    let topology = resource_topology_from(
        &access,
        cluster.clone(),
        namespaces,
        Some(mode.to_owned()),
        Some(Arc::unwrap_or_clone(custom_kinds)),
        custom_kinds_are_present,
    )
    .await?;
    let mut body = topology_body(
        topology,
        focus
            .as_ref()
            .map(|(kind, name)| (kind.as_str(), name.as_str())),
        arguments.limit,
        arguments.offset,
    )?;
    body.insert("context".into(), cluster.into());
    Ok(Value::Object(body))
}

/// Nodes carry their index in the whole (focused) graph, and edges name nodes by that
/// index, so an edge may point at a node on another page. A page lists every edge that
/// touches one of its nodes.
pub fn topology_body(
    topology: ResourceTopology,
    focus: Option<(&str, &str)>,
    limit: Option<usize>,
    offset: Option<usize>,
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
    let node_page = page(indexed, limit, offset)?;
    let first = node_page.offset;
    let end = first + node_page.items.len();
    let mut body = node_page.into_json("nodes", |(position, node)| {
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
    body.insert(
        "edges".into(),
        Value::Array(
            edge_indices
                .into_iter()
                .filter(|(source, target, _)| {
                    (first..end).contains(source) || (first..end).contains(target)
                })
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
        RbacAccessReviewRequest {
            cluster_context: cluster.clone(),
            identity: arguments.identity,
            target: arguments.target,
            kubeconfig_env_var: None,
        },
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
    let (_, cluster) = access_for(arguments.context)?;
    let revisions =
        deployment_revisions(cluster.clone(), arguments.name, arguments.namespace, None).await?;
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
    // reversed edge; edge indices shift when a page or focus removes nodes; an edge to a
    // node on another page is lost; a focus that matches nothing returns an empty graph
    // instead of an error.
    #[test]
    fn focus_keeps_the_connected_resources_and_reindexes_edges() {
        let body = topology_body(topology(), Some(("pod", "a")), None, None).unwrap();
        assert_eq!(names(&body), ["web", "web", "a"]);
        assert_eq!(
            body["edges"],
            json!([{"from": 0, "to": 1, "relation": "routesTo"},
                   {"from": 1, "to": 2, "relation": "selects"}])
        );
        let error = topology_body(topology(), Some(("Pod", "missing")), None, None).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::NotFound);
    }

    #[test]
    fn a_page_lists_edges_that_reach_nodes_on_other_pages() {
        let body = topology_body(topology(), None, Some(1), Some(2)).unwrap();
        assert_eq!(names(&body), ["a"]);
        assert_eq!(body["nodes"][0]["index"], 2);
        assert_eq!(body["nextOffset"], 3);
        assert_eq!(
            body["edges"],
            json!([{"from": 1, "to": 2, "relation": "selects"}])
        );
    }
}
