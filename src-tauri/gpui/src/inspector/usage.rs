//! `AppUsageFooter.svelte`: h-7 border-t bg-sidebar px-4, refreshed every 2s.
use super::Inspector;
use crate::{theme::Tokens, ui};
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, SharedString, Task, div, prelude::*, px};
use kubecove_lib::models::{AppUsageMetrics, AppUsageMetricsBreakdown};
use std::time::Duration;

#[derive(Default)]
pub(super) struct UsageState {
    pub visible: bool,
    open: bool,
    sample: Option<Result<AppUsageMetrics, String>>,
    task: Option<Task<()>>,
}

fn memory(bytes: u64) -> String {
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
                    .child(ui::xs(memory(item.memory_bytes)))
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
                    let result = backend.app_usage().await.map_err(|error| error.message);
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
        let label = match &self.usage.sample {
            None => "Loading usage metrics...".into(),
            Some(Err(_)) => "Usage metrics unavailable".into(),
            Some(Ok(sample)) => format!(
                "CPU {:.1}% · Memory {} · {}",
                sample.cpu_percent.clamp(0., 100.),
                memory(sample.memory_bytes),
                count(sample.process_count)
            ),
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
                            .w(px(496.))
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
