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
    pub(super) revision: u64,
    /// Every answering context's metrics. `None` while loading or when no
    /// context answered.
    summary: Option<ResourceMetricsSummary>,
    /// Each read context in rank order with why its metrics are missing, or
    /// `None` when they are available. Empty while loading.
    contexts: Vec<(String, Option<String>)>,
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
        let Some(summary) = &self.metrics.summary else {
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

    /// "metrics available" while any context has metrics, else the first
    /// context's issue. With several contexts, each context without metrics is
    /// listed under the message.
    pub(super) fn metrics_card(&self) -> Div {
        let contexts = &self.metrics.contexts;
        let message = match contexts.first() {
            Some((_, Some(issue))) if contexts.iter().all(|(_, issue)| issue.is_some()) => {
                issue.as_str()
            }
            _ => "metrics available",
        };
        let listed = if contexts.len() > 1 {
            contexts.as_slice()
        } else {
            &[]
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
            .children(listed.iter().filter_map(|(context, issue)| {
                let line = format!("{context}: {}", issue.as_deref()?);
                Some(
                    ui::muted(ui::xs(line.clone()))
                        .id(format!("metrics-context:{context}"))
                        .test_support()
                        .aria_label(line),
                )
            }))
    }

    /// Reads every context's metrics at once. Rows find theirs by cluster.
    pub(super) fn load_metrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.metrics.task.take();
        self.metrics.revision += 1;
        self.metrics.summary = None;
        self.metrics.contexts.clear();
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
            let (summary, contexts) = merge(read.await);
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                this.metrics.slots = summary.as_ref().map(slots).unwrap_or_default();
                this.metrics.summary = summary;
                this.metrics.revision += 1;
                this.metrics.contexts = contexts;
                cx.notify();
            });
        }));
    }
}

/// Every answering context's metrics in one summary, and each context's issue
/// in rank order. Metrics keep their cluster, so the contexts never mix.
fn merge(
    results: crate::workspaces::reads::ContextResults<ResourceMetricsSummary>,
) -> (
    Option<ResourceMetricsSummary>,
    Vec<(String, Option<String>)>,
) {
    let mut merged: Option<ResourceMetricsSummary> = None;
    let mut contexts = Vec::with_capacity(results.len());
    for (context, result) in results {
        let issue = match result {
            Ok(summary) => {
                let issue = metrics::issue(&summary.availability).map(str::to_owned);
                if let Some(all) = &mut merged {
                    all.pods.extend(summary.pods);
                    all.nodes.extend(summary.nodes);
                    all.workloads.extend(summary.workloads);
                    all.warnings.extend(summary.warnings);
                } else {
                    merged = Some(summary);
                }
                issue
            }
            Err(error) => Some(metrics::read_issue(&error).to_owned()),
        };
        contexts.push((context, issue));
    }
    (merged, contexts)
}
