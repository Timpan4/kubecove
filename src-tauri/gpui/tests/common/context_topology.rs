//! The fixture's topology and metrics reads, including the ownership graphs of
//! the workspace contexts other than the fixture's primary context, which the
//! map's "Map context" select reads.

use super::*;
use std::sync::Arc;

/// One context's exported graphs: every namespace, and each namespace alone.
#[derive(Deserialize)]
pub(super) struct ContextTopology {
    all: ResourceTopology,
    namespaces: HashMap<String, ResourceTopology>,
}

impl Fixture {
    /// The graph of a non-primary context for `scope`, or `None` for the
    /// primary context, whose graphs are `topology` and `namespace_topologies`.
    pub(super) fn context_topology(&self, scope: &Scope) -> Option<ResourceTopology> {
        let graphs = self.context_topologies.get(&scope.context)?;
        let graph = match &scope.namespace {
            Some(namespace) => graphs.namespaces.get(namespace).unwrap_or_else(|| {
                panic!(
                    "fixture has no {} graph for namespace {namespace}",
                    scope.context
                )
            }),
            None => &graphs.all,
        };
        Some(graph.clone())
    }

    /// The all-namespaces graph, or a namespace's graph the fixture exported.
    pub(super) fn topology_fixture(&self, scope: Scope) -> Read<Arc<ResourceTopology>> {
        let context = scope.context.clone();
        let key = scope.namespace.clone().unwrap_or_default();
        let mut topology = self.context_topology(&scope).unwrap_or_else(|| {
            scope
                .namespace
                .and_then(|namespace| self.namespace_topologies.get(&namespace))
                .unwrap_or(&self.topology)
                .clone()
        });
        self.add_live_topology(&mut topology);
        let topology = Arc::new(topology);
        self.coverage_read(
            "list_resource_topology",
            self.timing
                .read("list_resource_topology", &context, &key, topology),
        )
    }

    /// The existing "add" script creates the Deployment and its ReplicaSet,
    /// as ownershipTopology builds them from the web's live resource rows.
    fn add_live_topology(&self, topology: &mut ResourceTopology) {
        let Some(source) = topology.nodes.iter().find(|node| {
            node.kind == "Deployment"
                && node.name == "payments-api"
                && node.namespace.as_deref() == Some("payments")
        }) else {
            return;
        };
        let Some(added) = self
            .live_rows(vec![source.summary.clone()])
            .into_iter()
            .find(|row| row.name != source.name)
        else {
            return;
        };
        let Some(replica_set) = topology.nodes.iter().find(|node| {
            node.kind == "ReplicaSet"
                && node.name == "payments-api-7d9c9b7f8d"
                && node.namespace == source.namespace
        }) else {
            return;
        };
        let mut deployment = source.clone();
        let mut replica_set = replica_set.clone();
        deployment.id = format!(
            "{}:apps/v1:Deployment:payments:{}",
            added.cluster, added.name
        );
        deployment.name = added.name.clone();
        deployment.deployment_revision = None;
        deployment.summary = added;
        replica_set.name = format!("{}-54b7f7bdbc", deployment.name);
        replica_set.summary.name = replica_set.name.clone();
        replica_set.id = format!(
            "{}:apps/v1:ReplicaSet:payments:{}",
            deployment.summary.cluster, replica_set.name
        );
        replica_set.deployment_revision = None;
        topology.edges.push(kubecove_lib::models::TopologyEdge {
            id: format!("{}->{}", deployment.id, replica_set.id),
            source: deployment.id.clone(),
            target: replica_set.id.clone(),
            relation: kubecove_lib::models::TopologyRelation::Owns,
        });
        topology.nodes.extend([deployment, replica_set]);
    }

    pub(super) fn network_topology_fixture(&self, scope: Scope) -> Read<Arc<ResourceTopology>> {
        let mut graph = self.network_topology.clone();
        if let Some(namespace) = &scope.namespace {
            graph
                .nodes
                .retain(|node| node.namespace.as_ref() == Some(namespace));
            let ids: std::collections::HashSet<_> =
                graph.nodes.iter().map(|node| &node.id).collect();
            graph
                .edges
                .retain(|edge| ids.contains(&edge.source) && ids.contains(&edge.target));
        }
        Box::pin(async move { Ok(Arc::new(graph)) })
    }

    pub(super) fn metrics_fixture(
        &self,
        scope: Scope,
    ) -> Read<kubecove_lib::models::ResourceMetricsSummary> {
        let mut metrics = self.metrics.clone();
        for metric in metrics
            .pods
            .iter_mut()
            .chain(&mut metrics.nodes)
            .chain(&mut metrics.workloads)
        {
            metric.cluster = scope.context.clone();
        }
        Box::pin(async move { Ok(metrics) })
    }
}
