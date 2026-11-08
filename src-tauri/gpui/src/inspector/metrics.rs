//! Resource metrics card and async reads from `ResourceBrowser.svelte`.
use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{metrics, ui};
use gpui_kit::{Context, Div, Task, Window, prelude::*, px};
use kubecove_lib::models::{ResourceMetricSummary, ResourceMetricsSummary, ResourceSummary};
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct MetricsState {
    summary: Option<Result<ResourceMetricsSummary, String>>,
    index: HashMap<(String, String, Option<String>, String), ResourceMetricSummary>,
    task: Option<Task<()>>,
}

fn key(row: &ResourceSummary) -> (String, String, Option<String>, String) {
    (
        row.cluster.clone(),
        row.kind.clone(),
        row.namespace.clone(),
        row.name.clone(),
    )
}

impl Inspector {
    pub(super) fn resource_metric<'a>(
        &'a self,
        row: &'a ResourceSummary,
    ) -> Option<&'a ResourceMetricSummary> {
        self.metrics.index.get(&key(row))
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

    pub(super) fn load_metrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.metrics.task.take();
        self.metrics.summary = None;
        self.metrics.index.clear();
        self.table_order.invalidate();
        let request = self.request.clone();
        let read = self.backend.metrics(self.scope.clone());
        let read = self.timings.measure("metrics", read);
        self.metrics.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                if let Ok(summary) = &result {
                    for metric in summary
                        .pods
                        .iter()
                        .chain(&summary.nodes)
                        .chain(&summary.workloads)
                    {
                        this.metrics.index.insert(
                            (
                                metric.cluster.clone(),
                                metric.kind.clone(),
                                metric.namespace.clone(),
                                metric.name.clone(),
                            ),
                            metric.clone(),
                        );
                    }
                }
                this.table_order.invalidate();
                this.metrics.summary = Some(result.map_err(|error| error.message));
                cx.notify();
            });
        }));
    }
}
