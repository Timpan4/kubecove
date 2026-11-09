//! `AppUsageFooter.svelte`: h-7 border-t bg-sidebar px-4, refreshed every 2s.
use super::Inspector;
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{AnyElement, Context, Div, SharedString, Task, div, prelude::*, px};
use kubecove_lib::models::{AppUsageMemory, AppUsageMetrics, AppUsageMetricsBreakdown};
use std::time::Duration;

#[derive(Default)]
pub(super) struct UsageState {
    pub visible: bool,
    open: bool,
    sample: Option<Result<AppUsageMetrics, String>>,
    task: Option<Task<()>>,
}

/// `USAGE_MEMORY_FIGURES` in `usage-metrics.ts`: label and description.
const FIGURES: [(&str, &str); 3] = [
    (
        "WSS",
        "Working set size. Windows reports its working set. Linux reports memory touched since the previous refresh when working set measurement is on. Not available on macOS.",
    ),
    (
        "RSS",
        "Resident set size on Linux and macOS. Pages shared between processes count once per process.",
    ),
    (
        "PSS",
        "Proportional set size on Linux. Shared pages are split between the processes that map them.",
    ),
];

/// Each memory figure as "RSS 370 MB", or "WSS --" when it is not reported.
fn figures(memory: &AppUsageMemory) -> [String; 3] {
    let values = [memory.wss_bytes, memory.rss_bytes, memory.pss_bytes];
    std::array::from_fn(|at| {
        let value = values[at].map_or_else(|| "--".to_owned(), bytes);
        format!("{} {value}", FIGURES[at].0)
    })
}

/// `describeUsageMemory`: what each figure means, and the interval a measured
/// Linux WSS covers.
fn describe(sample: &AppUsageMetrics) -> String {
    let mut lines: Vec<String> = FIGURES
        .iter()
        .map(|(label, description)| format!("{label}: {description}"))
        .collect();
    if let Some(window) = sample.working_set_window_ms {
        lines.push(format!(
            "WSS covers the last {:.1} s.",
            window as f64 / 1000.
        ));
    }
    lines.join("\n")
}

fn bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut at = 0;
    while value >= 1024. && at < units.len() - 1 {
        value /= 1024.;
        at += 1;
    }
    let precision = usize::from(at != 0 && value < 10.);
    format!("{value:.precision$} {}", units[at])
}

fn count(value: usize) -> String {
    format!(
        "{value} {}",
        if value == 1 { "process" } else { "processes" }
    )
}

fn process_rows(items: &[AppUsageMetricsBreakdown], depth: usize) -> Div {
    div().flex().flex_col().children(items.iter().map(|item| {
        div()
            .child(
                div()
                    .id(SharedString::from(format!("usage-process:{}", item.label)))
                    .test_support()
                    .aria_label(item.label.clone())
                    .flex()
                    .gap(px(12.))
                    .pl(px(depth as f32 * 12.))
                    .child(ui::xs(item.label.clone()).flex_1())
                    .child(ui::xs(format!("{:.1}%", item.cpu_percent.clamp(0., 100.))))
                    .children(figures(&item.memory).map(ui::xs))
                    .child(ui::xs(count(item.process_count))),
            )
            .child(process_rows(&item.children, depth + 1))
    }))
}

impl Inspector {
    pub(super) fn toggle_usage(&mut self, cx: &mut Context<Self>) {
        self.usage.visible = !self.usage.visible;
        self.usage.open = false;
        self.usage.task.take();
        if self.usage.visible {
            let backend = self.backend.clone();
            self.usage.task = Some(cx.spawn(async move |this, cx| {
                loop {
                    // Read per sample, so a changed setting applies on the next refresh.
                    let Ok(measure_working_set) = this.update(cx, |this, cx| {
                        if this.usage.sample.as_ref().is_some_and(Result::is_err) {
                            this.usage.sample = None;
                            cx.notify();
                        }
                        this.persisted.preferences.measure_working_set
                    }) else {
                        break;
                    };
                    let source = backend.clone();
                    let result =
                        crate::read_retry::read(cx.background_executor().clone(), move || {
                            source.app_usage(measure_working_set)
                        })
                        .await
                        .map_err(|error| error.message);
                    if this
                        .update(cx, |this, cx| {
                            this.usage.sample = Some(result);
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                }
            }));
        }
        cx.notify();
    }

    pub(super) fn usage_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.usage.visible {
            return None;
        }
        let loading = self
            .loading
            .phase("usage-footer", self.usage.sample.is_none());
        let label = match &self.usage.sample {
            _ if loading.shown() => "Loading usage metrics...".into(),
            None => String::new(),
            _ if !loading.ready() => String::new(),
            Some(Err(_)) => "Usage metrics unavailable".into(),
            Some(Ok(sample)) => format!(
                "CPU {:.1}% · {} · {}",
                sample.cpu_percent.clamp(0., 100.),
                figures(&sample.memory).join(" · "),
                count(sample.process_count)
            ),
        };
        let description: Option<SharedString> = match &self.usage.sample {
            Some(Ok(sample)) => Some(describe(sample).into()),
            _ => None,
        };
        Some(
            div()
                .relative()
                .flex_none()
                .w_full()
                .h(px(28.))
                .px(px(16.))
                .flex()
                .items_center()
                .justify_end()
                .border_t_1()
                .border_color(Tokens::border())
                .bg(Tokens::card())
                .child(
                    div()
                        .id("usage-footer")
                        .test_support()
                        .aria_label(label.clone())
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .child(ui::icon("activity", 14., Tokens::muted_foreground()))
                        .child(ui::muted(ui::xs(label)))
                        .when_some(description, |footer, description| {
                            footer.tooltip(move |window, cx| {
                                Tooltip::new(description.clone()).build(window, cx)
                            })
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.usage.open = !this.usage.open;
                            cx.notify();
                        })),
                )
                .when(self.usage.open, |footer| {
                    footer.child(
                        div()
                            .absolute()
                            .bottom(px(28.))
                            .right(px(16.))
                            .w(px(640.))
                            .max_h(px(448.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::card())
                            .p(px(8.))
                            .child(ui::semibold(ui::xs("App process tree")).mb(px(8.)))
                            .children(
                                self.usage
                                    .sample
                                    .as_ref()
                                    .and_then(|r| r.as_ref().ok())
                                    .map(|sample| process_rows(&sample.breakdown, 0)),
                            ),
                    )
                })
                .into_any_element(),
        )
    }
}
