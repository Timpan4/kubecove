//! Typed topology, resource metrics and app process sampling commands.
use super::{NativeBackend, Read, Scope};
use kubecove_lib::{
    commands,
    models::{AppUsageMetrics, ResourceMetricsSummary, ResourceTopology},
    runtime::State,
};
use std::sync::Arc;

pub(super) fn topology(
    backend: &NativeBackend,
    scope: Scope,
    mode: &'static str,
) -> Read<Arc<ResourceTopology>> {
    let store = backend.store.clone();
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        commands::list_resource_topology(
            scope.context,
            scope.namespace.into_iter().collect(),
            Some(mode.into()),
            None,
            None,
            None,
            State::new(&store),
            State::new(&cancellations),
        )
        .await
    })
}

pub(super) fn metrics(backend: &NativeBackend, scope: Scope) -> Read<ResourceMetricsSummary> {
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        commands::list_resource_metrics(
            scope.context,
            scope.namespace.into_iter().collect(),
            None,
            None,
            None,
            State::new(&cancellations),
        )
        .await
    })
}

pub(super) fn app_usage(
    backend: &NativeBackend,
    measure_working_set: bool,
) -> Read<AppUsageMetrics> {
    let monitor = backend.usage.clone();
    backend.read(async move {
        commands::get_app_usage_metrics(measure_working_set, State::new(&monitor)).await
    })
}
