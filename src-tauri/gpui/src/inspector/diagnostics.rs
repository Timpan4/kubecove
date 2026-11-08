//! Settings diagnostics mirroring the latency report in `DiagnosticsSettings.svelte`.
//! The report's `frontend` fields hold the native client's own timings: command
//! round trips, `<view>.present` spans and `*.render` counters, which
//! `presentation.rs` defines. Command round trips stand where the web logs
//! invoke and fetch spans.
use super::{
    Inspector,
    controls::{Variant, button},
    launcher_form::check_row,
    settings_page::settings_row,
};
use crate::targets::TestSupportExt;
use crate::{settings::Row, theme::Tokens, ui};
use gpui_kit::{AnyElement, ClipboardItem, Context, Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::{BackendCacheDiagnosticSnapshot, BackendDiagnosticEvent};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Nearest-rank summary of one timed name, as `summarizeDurations` in `src/lib/diagnostics.ts`.
struct Summary {
    name: String,
    count: usize,
    p50: f64,
    p95: f64,
    max: f64,
}

fn summaries<'a>(values: impl IntoIterator<Item = (&'a str, f64)>) -> Vec<Summary> {
    let mut groups: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for (name, duration) in values {
        groups.entry(name).or_default().push(duration);
    }
    groups
        .into_iter()
        .map(|(name, mut values)| {
            values.sort_by(f64::total_cmp);
            let n = values.len();
            let percentile = |p: usize| values[(n * p).div_ceil(100).saturating_sub(1)];
            Summary {
                name: name.to_owned(),
                count: n,
                p50: percentile(50),
                p95: percentile(95),
                max: values[n - 1],
            }
        })
        .collect()
}

fn summary_json(summary: &Summary) -> Value {
    json!({
        "name": summary.name,
        "count": summary.count,
        "p50Ms": summary.p50,
        "p95Ms": summary.p95,
        "maxMs": summary.max,
    })
}

/// The latency report's JSON, with the same top-level shape as the web's
/// `createLatencyReport`. Without identifiers it omits backend summary fields
/// and replaces cache labels with aliases.
fn latency_report_json(
    timings: &[crate::diagnostics::Timing],
    counters: &BTreeMap<&'static str, u64>,
    backend_events: &[BackendDiagnosticEvent],
    caches: &[BackendCacheDiagnosticSnapshot],
    include_identifiers: bool,
) -> String {
    let native = summaries(
        timings
            .iter()
            .map(|timing| (timing.name, timing.duration_ms)),
    );
    let backend = summaries(
        backend_events
            .iter()
            .map(|event| (event.command.as_str(), event.duration_ms as f64)),
    );
    let frontend_events: Vec<Value> = timings
        .iter()
        .map(|timing| {
            json!({
                "name": timing.name,
                "durationMs": timing.duration_ms,
                "success": timing.success,
            })
        })
        .collect();
    let backend_event_json: Vec<Value> = backend_events
        .iter()
        .map(|event| {
            let mut value = json!({
                "command": event.command,
                "durationMs": event.duration_ms,
                "status": event.status,
            });
            if include_identifiers {
                value["summary"] = json!(event.summary);
            }
            value
        })
        .collect();
    let cache_json: Vec<Value> = caches
        .iter()
        .enumerate()
        .map(|(index, cache)| {
            let label = if include_identifiers {
                cache.label.clone()
            } else {
                format!("cache-{}", index + 1)
            };
            json!({
                "label": label,
                "ready": cache.ready,
                "dirty": cache.dirty,
                "loading": cache.loading,
                "hits": cache.hits,
                "misses": cache.misses,
                "joins": cache.joins,
                "evictions": cache.evictions,
                "restoredReloadFailures": cache.restored_reload_failures,
                "retainedItems": cache.retained_items,
                "shallowPayloadBytes": cache.shallow_payload_bytes,
            })
        })
        .collect();
    let report = json!({
        "generatedAt": chrono::Utc::now().to_rfc3339(),
        "appVersion": env!("CARGO_PKG_VERSION"),
        "redacted": !include_identifiers,
        "frontend": {
            "summaries": native.iter().map(summary_json).collect::<Vec<_>>(),
            "counters": counters,
            "events": frontend_events,
        },
        "backend": {
            "summaries": backend.iter().map(summary_json).collect::<Vec<_>>(),
            "events": backend_event_json,
            "caches": cache_json,
        },
    });
    serde_json::to_string_pretty(&report).expect("latency report serializes")
}

/// Summary table with the web's columns. `empty_id` marks the empty state.
fn timing_table(
    row_prefix: &'static str,
    empty_id: &'static str,
    empty_text: &'static str,
    rows: &[Summary],
) -> Div {
    let mut table = div()
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
                .children(
                    ["NAME", "COUNT", "P50", "P95", "MAX"]
                        .map(|label| ui::semibold(ui::muted(ui::xs(label))).flex_1().px(px(12.))),
                ),
        );
    if rows.is_empty() {
        table = table.child(
            ui::muted(ui::sm(empty_text))
                .py(px(32.))
                .text_center()
                .id(empty_id)
                .test_support()
                .aria_label(empty_text),
        );
    }
    for summary in rows {
        let text = format!(
            "{} · {} · {:.2} ms · {:.2} ms · {:.2} ms",
            summary.name, summary.count, summary.p50, summary.p95, summary.max
        );
        let cells = [
            summary.name.clone(),
            summary.count.to_string(),
            format!("{:.2} ms", summary.p50),
            format!("{:.2} ms", summary.p95),
            format!("{:.2} ms", summary.max),
        ];
        table = table.child(
            div()
                .flex()
                .py(px(8.))
                .id(SharedString::from(format!("{row_prefix}:{}", summary.name)))
                .test_support()
                .aria_label(text)
                .children(
                    cells.map(|value| ui::xs(value).flex_1().min_w_0().truncate().px(px(12.))),
                ),
        );
    }
    table
}

// `diagnosticCounterRows` shows this many counters.
const MAX_COUNTER_ROWS: usize = 8;

/// `diagnosticCounterRows`: render counters, most first.
fn counter_table(counters: &BTreeMap<&'static str, u64>) -> Div {
    let mut rows: Vec<_> = counters
        .iter()
        .filter(|(name, _)| name.ends_with(".render"))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let mut table = div()
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
                .children(
                    ["NAME", "COUNT"]
                        .map(|label| ui::semibold(ui::muted(ui::xs(label))).flex_1().px(px(12.))),
                ),
        );
    if rows.is_empty() {
        let empty = "No render counts yet.";
        table = table.child(
            ui::muted(ui::sm(empty))
                .py(px(32.))
                .text_center()
                .id("render-counters-empty")
                .test_support()
                .aria_label(empty),
        );
    }
    for (name, count) in rows.into_iter().take(MAX_COUNTER_ROWS) {
        table = table.child(
            div()
                .flex()
                .py(px(8.))
                .id(SharedString::from(format!("render-counter:{name}")))
                .test_support()
                .aria_label(format!("{name} · {count}"))
                .children(
                    [name.to_string(), count.to_string()]
                        .map(|value| ui::xs(value).flex_1().min_w_0().truncate().px(px(12.))),
                ),
        );
    }
    table
}

impl Inspector {
    pub(super) fn refresh_diagnostics(&mut self, clear: bool, cx: &mut Context<Self>) {
        self.read_diagnostics(clear, None, cx);
    }

    /// Reads backend timings and caches. `announce` is the status the panel shows on success.
    fn read_diagnostics(
        &mut self,
        clear: bool,
        announce: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
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
                        if let Some(message) = announce {
                            this.settings.diagnostics_status = Some(message.to_owned());
                        }
                    }
                    Err(error) => {
                        this.settings.diagnostics_status = None;
                        this.settings.diagnostics_error = Some(error.message);
                    }
                }
                cx.notify();
            });
        }));
    }

    /// Copies the report. Only the identifiers toggle decides what it includes.
    fn copy_latency_report(&mut self, cx: &mut Context<Self>) {
        let include_identifiers = self.settings.latency_identifiers;
        let timings = self.timings.snapshot();
        let text = latency_report_json(
            &timings,
            &self.timings.counters(),
            &self.settings.diagnostics.events,
            &self.settings.diagnostics.caches,
            include_identifiers,
        );
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.settings.diagnostics_status = Some(
            if include_identifiers {
                "Report copied."
            } else {
                "Redacted report copied."
            }
            .to_owned(),
        );
        cx.notify();
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
        let timings = self.timings.snapshot();
        let native = summaries(
            timings
                .iter()
                .map(|timing| (timing.name, timing.duration_ms)),
        );
        let backend_events = &self.settings.diagnostics.events;
        let backend = summaries(
            backend_events
                .iter()
                .map(|event| (event.command.as_str(), event.duration_ms as f64)),
        );
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.read_diagnostics(false, Some("Trace refreshed."), cx)
                    })),
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
                        this.read_diagnostics(true, Some("Trace cleared."), cx);
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
                    .on_click(cx.listener(|this, _, _, cx| this.copy_latency_report(cx))),
            );
        let identifiers = self.settings.latency_identifiers;
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .mt(px(16.))
            .child(ui::muted(ui::xs(format!(
                "Native {} events, backend {} events.",
                timings.len(),
                backend_events.len()
            ))))
            .child(
                check_row(
                    SharedString::from("latency-identifiers"),
                    "Include identifiers",
                    identifiers,
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.latency_identifiers = !this.settings.latency_identifiers;
                    cx.notify();
                })),
            )
            .child(ui::medium(ui::xs("Native timings")))
            .child(timing_table(
                "latency-command",
                "latency-empty",
                "No native timings yet.",
                &native,
            ))
            .child(ui::medium(ui::xs("Render counters")))
            .child(counter_table(&self.timings.counters()))
            .child(ui::medium(ui::xs("Backend timings")))
            .child(timing_table(
                "backend-command",
                "backend-empty",
                "No backend timings yet.",
                &backend,
            ))
            .child(ui::medium(ui::xs("Backend caches")));
        if self.settings.diagnostics.caches.is_empty() {
            body = body.child(ui::muted(ui::xs(
                "Enable diagnostics to inspect backend caches.",
            )));
        }
        for cache in &self.settings.diagnostics.caches {
            let metrics = [
                ("Ready", cache.ready.to_string()),
                ("Dirty", cache.dirty.to_string()),
                ("Loading", cache.loading.to_string()),
                ("Hits", cache.hits.to_string()),
                ("Misses", cache.misses.to_string()),
                ("Joins", cache.joins.to_string()),
                ("Evictions", cache.evictions.to_string()),
                (
                    "Restored failures",
                    cache.restored_reload_failures.to_string(),
                ),
                ("Items", cache.retained_items.to_string()),
                ("Shallow bytes", cache.shallow_payload_bytes.to_string()),
            ];
            let mut rows = div()
                .flex()
                .flex_col()
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .child(ui::medium(ui::xs(cache.label.clone())));
            for (metric, value) in metrics {
                rows = rows.child(
                    div()
                        .flex()
                        .justify_between()
                        .py(px(4.))
                        .child(ui::xs(metric))
                        .child(ui::xs(value)),
                );
            }
            body = body.child(rows);
        }
        if let Some(status) = &self.settings.diagnostics_status {
            body = body.child(
                ui::xs(status.clone())
                    .id("latency-status")
                    .test_support()
                    .aria_label(status.clone()),
            );
        }
        if let Some(error) = &self.settings.diagnostics_error {
            body = body.child(ui::xs(error.clone()).text_color(Tokens::destructive()));
        }
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
