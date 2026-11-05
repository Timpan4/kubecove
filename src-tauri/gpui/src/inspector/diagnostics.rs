//! Settings diagnostics for native command round trips.
use super::{
    Inspector,
    controls::{Variant, button},
    settings_page::settings_row,
};
use crate::{settings::Row, theme::Tokens, ui};
use gpui_kit::{
    AnyElement, ClipboardItem, Context, SharedString, TestSupportExt, div, prelude::*, px,
};
use std::collections::BTreeMap;

impl Inspector {
    pub(super) fn refresh_diagnostics(&mut self, clear: bool, cx: &mut Context<Self>) {
        self.settings.diagnostics_error = None;
        let backend = self.backend.clone();
        let enabled = self.persisted.preferences.debug_mode_enabled;
        let read = if clear {
            backend.diagnostics(enabled, true)
        } else {
            crate::read_retry::read(cx.background_executor().clone(), move || {
                backend.diagnostics(enabled, false)
            })
        };
        self.settings.diagnostics_task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(snapshot) => {
                        this.settings.diagnostics = snapshot;
                        this.settings.diagnostics_error = None;
                    }
                    Err(error) => this.settings.diagnostics_error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn documentation_control(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        settings_row(
            row.title,
            row.description.into(),
            button(Variant::Outline, 24., false)
                .id("open-documentation")
                .test_support()
                .aria_label("Open Wiki")
                .child("Open Wiki")
                .on_click(cx.listener(|this, _, window, cx| {
                    let read = this.backend.open_documentation();
                    this.settings.task = Some(cx.spawn_in(window, async move |this, cx| {
                        let result = read.await;
                        let _ = this.update(cx, |this, cx| {
                            this.notice = result.err().map(|e| e.message);
                            cx.notify();
                        });
                    }));
                }))
                .into_any_element(),
        )
        .into_any_element()
    }

    pub(super) fn latency_report(&self, row: &'static Row, cx: &mut Context<Self>) -> AnyElement {
        let events = self.timings.snapshot();
        let mut groups: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        for event in &events {
            groups
                .entry(event.name)
                .or_default()
                .push(event.duration_ms);
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .mt(px(16.))
            .child(ui::muted(ui::xs(format!(
                "Native {} events.",
                events.len()
            ))));
        let mut table =
            div()
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .child(
                    div()
                        .flex()
                        .py(px(8.))
                        .border_b_1()
                        .border_color(Tokens::border())
                        .children(["NAME", "COUNT", "P50", "P95", "MAX"].map(|label| {
                            ui::semibold(ui::muted(ui::xs(label))).flex_1().px(px(12.))
                        })),
                );
        if groups.is_empty() {
            table = table.child(
                ui::muted(ui::sm("No timed native commands yet."))
                    .py(px(32.))
                    .text_center()
                    .id("latency-empty")
                    .test_support()
                    .aria_label("No timed native commands yet."),
            );
        }
        for (name, mut values) in groups {
            values.sort_by(f64::total_cmp);
            let n = values.len();
            let percentile = |p: usize| values[(n * p).div_ceil(100).saturating_sub(1)];
            let text = format!(
                "{name} · {n} · {:.2} ms · {:.2} ms · {:.2} ms",
                percentile(50),
                percentile(95),
                values[n - 1]
            );
            let cells = [
                name.to_owned(),
                n.to_string(),
                format!("{:.2} ms", percentile(50)),
                format!("{:.2} ms", percentile(95)),
                format!("{:.2} ms", values[n - 1]),
            ];
            table = table.child(
                div()
                    .flex()
                    .py(px(8.))
                    .id(SharedString::from(format!("latency-command:{name}")))
                    .test_support()
                    .aria_label(text)
                    .children(
                        cells.map(|value| ui::xs(value).flex_1().min_w_0().truncate().px(px(12.))),
                    ),
            );
        }
        body = body.child(
            div()
                .child(ui::medium(ui::xs("Native command round trips")))
                .child(table.mt(px(8.))),
        );
        body = body.child(ui::medium(ui::xs("Backend timings")));
        for event in &self.settings.diagnostics.events {
            body = body.child(ui::xs(format!(
                "{} · {} ms",
                event.command, event.duration_ms
            )));
        }
        body = body.child(ui::medium(ui::xs("Backend caches")));
        for cache in &self.settings.diagnostics.caches {
            body = body.child(ui::xs(format!(
                "{} · Ready {} · Dirty {} · Loading {} · Hits {} · Misses {}",
                cache.label, cache.ready, cache.dirty, cache.loading, cache.hits, cache.misses
            )));
        }
        if let Some(error) = &self.settings.diagnostics_error {
            body = body.child(ui::xs(error.clone()).text_color(Tokens::destructive()));
        }
        let controls = div()
            .flex()
            .gap(px(8.))
            .child(
                button(Variant::Outline, 24., false)
                    .id("latency-refresh")
                    .test_support()
                    .aria_label("Refresh")
                    .child(ui::icon("refresh-ccw", 14., Tokens::foreground()))
                    .child("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh_diagnostics(false, cx))),
            )
            .child(
                button(Variant::Outline, 24., false)
                    .id("latency-clear")
                    .test_support()
                    .aria_label("Clear")
                    .child(ui::icon("trash-2", 14., Tokens::foreground()))
                    .child("Clear")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.timings.clear();
                        this.refresh_diagnostics(true, cx);
                        cx.notify();
                    })),
            )
            .child(
                button(Variant::Default, 24., false)
                    .id("latency-copy")
                    .test_support()
                    .aria_label("Copy")
                    .child(ui::icon("copy", 14., Tokens::primary_foreground()))
                    .child("Copy")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let backend: Vec<_> = this.settings.diagnostics.events.iter().map(|event|
                            serde_json::json!({"command": event.command, "durationMs": event.duration_ms, "status": event.status})
                        ).collect();
                        let text = serde_json::to_string_pretty(&serde_json::json!({
                            "native": this.timings.snapshot(), "backend": backend
                        })).expect("timings serialize");
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    })),
            );
        div()
            .id("setting-row:Latency report")
            .test_support()
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::sm("Latency report")))
                            .child(ui::muted(ui::xs(row.description)).mt(px(4.))),
                    )
                    .child(controls),
            )
            .child(body)
            .into_any_element()
    }
}
