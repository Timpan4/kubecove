//! Ownership graphs of the workspace contexts other than the fixture's primary
//! context, which the map's "Map context" select reads.

use super::*;

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
}
