//! Resource metrics card and async reads from `ResourceBrowser.svelte`.
use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{metrics, ui};
use gpui_kit::{Context, Div, Task, Window, prelude::*, px};
use kubecove_lib::models::{ResourceMetricSummary, ResourceMetricsSummary, ResourceSummary};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
};

#[derive(Default)]
pub(super) struct MetricsState {
    summary: Option<Result<ResourceMetricsSummary, String>>,
    /// Each metric's identity hash and its position in `all`, sorted by hash,
    /// so a row finds its metric without building a key.
    slots: Vec<(u64, u32)>,
    task: Option<Task<()>>,
}

fn identity(cluster: &str, kind: &str, namespace: Option<&str>, name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    (cluster, kind, namespace, name).hash(&mut hasher);
    hasher.finish()
}

/// Pod, node and workload metrics, in the order a later metric of the same
/// resource replaces an earlier one.
fn all(summary: &ResourceMetricsSummary) -> impl Iterator<Item = &ResourceMetricSummary> {
    summary
        .pods
        .iter()
        .chain(&summary.nodes)
        .chain(&summary.workloads)
}

fn slots(summary: &ResourceMetricsSummary) -> Vec<(u64, u32)> {
    let mut slots: Vec<(u64, u32)> = all(summary)
        .enumerate()
        .map(|(position, metric)| {
            let hash = identity(
                &metric.cluster,
                &metric.kind,
                metric.namespace.as_deref(),
                &metric.name,
            );
            (
                hash,
                u32::try_from(position).expect("metric count fits u32"),
            )
        })
        .collect();
    slots.sort_unstable();
    slots
}

impl Inspector {
    pub(super) fn resource_metric<'a>(
        &'a self,
        row: &'a ResourceSummary,
    ) -> Option<&'a ResourceMetricSummary> {
        let Some(Ok(summary)) = &self.metrics.summary else {
            return None;
        };
        let slots = &self.metrics.slots;
        let hash = identity(&row.cluster, &row.kind, row.namespace.as_deref(), &row.name);
        let start = slots.partition_point(|&(slot, _)| slot < hash);
        // Equal hashes sort by position, so the last match is the latest metric.
        slots[start..]
            .iter()
            .take_while(|&&(slot, _)| slot == hash)
            .map(|&(_, position)| {
                let mut position = position as usize;
                for list in [&summary.pods, &summary.nodes, &summary.workloads] {
                    if position < list.len() {
                        return &list[position];
                    }
                    position -= list.len();
                }
                unreachable!("slot positions index the metrics they were built from")
            })
            .filter(|metric| {
                metric.cluster == row.cluster
                    && metric.kind == row.kind
                    && metric.namespace == row.namespace
                    && metric.name == row.name
            })
            .last()
    }

    pub(super) fn metrics_card(&self) -> Div {
        let message = match &self.metrics.summary {
            Some(Ok(summary)) => metrics::availability(&summary.availability),
            Some(Err(_)) => "metrics API unavailable",
            None => "metrics available",
        };
        ui::panel()
            .flex_1()
            .px(px(12.))
            .py(px(8.))
            .child(ui::semibold(ui::xs("Resource metrics")))
            .child(
                ui::muted(ui::xs(message.to_owned()))
                    .id("metrics-message")
                    .test_support()
                    .aria_label(message),
            )
    }

    /// Reads every context's metrics at once. Rows find theirs by cluster.
    pub(super) fn load_metrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.metrics.task.take();
        self.metrics.summary = None;
        self.metrics.slots.clear();
        let request = self.request.clone();
        let reads = self
            .observation
            .contexts()
            .iter()
            .map(|context| {
                let read = self.backend.metrics(crate::backend::Scope {
                    context: context.clone(),
                    namespace: self.scope.namespace.clone(),
                });
                (context.clone(), self.timings.measure("metrics", read))
            })
            .collect();
        let read = crate::workspaces::reads::settle(reads);
        self.metrics.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = merge(read.await);
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                this.metrics.slots = result.as_ref().map_or_default(slots);
                this.metrics.summary = Some(result);
                cx.notify();
            });
        }));
    }
}

/// Every context's metrics in one summary. Metrics keep their cluster, so the
/// contexts never mix. The card shows the availability of the first context in
/// rank order that answered; the read fails only when every context failed.
fn merge(
    results: crate::workspaces::reads::ContextResults<ResourceMetricsSummary>,
) -> Result<ResourceMetricsSummary, String> {
    let mut merged: Option<ResourceMetricsSummary> = None;
    let mut first_error = None;
    for (_, result) in results {
        match result {
            Ok(summary) => {
                if let Some(all) = &mut merged {
                    all.pods.extend(summary.pods);
                    all.nodes.extend(summary.nodes);
                    all.workloads.extend(summary.workloads);
                    all.warnings.extend(summary.warnings);
                } else {
                    merged = Some(summary);
                }
            }
            Err(error) => {
                first_error.get_or_insert(error.message);
            }
        }
    }
    merged.ok_or_else(|| first_error.unwrap_or_default())
}
