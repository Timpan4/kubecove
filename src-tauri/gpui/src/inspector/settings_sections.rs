//! Disabled host-specific sections retain the controls and evidence layout in
//! `ResourceKindOrderSettings`, `DiagnosticsSettings`, `McpServerSettings`,
//! `RemoteConnectionSettings` and `UpdatesSettings.svelte`.

use super::controls::{Variant, button};
use super::settings_page::{Observed, settings_row, unavailable_control};
use crate::settings::Row;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Div, SharedString, div, prelude::*, px};

fn disabled(row: &'static Row, widget: impl IntoElement) -> AnyElement {
    unavailable_control(
        SharedString::from(format!("setting-control:{}", row.title)),
        row.unavailable
            .expect("host sections have an unavailable reason"),
        widget,
    )
}

fn panel(row: &'static Row) -> Observed {
    div()
        .id(SharedString::from(format!("setting-row:{}", row.title)))
        .test_support()
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
}

fn heading(row: &'static Row, controls: impl IntoElement) -> Div {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(ui::medium(ui::sm(row.title)))
                .child(ui::muted(ui::xs(row.description)).mt(px(4.))),
        )
        .child(controls)
}

fn actions(labels: &[&'static str]) -> Div {
    div().flex().gap(px(8.)).children(
        labels
            .iter()
            .map(|label| button(Variant::Outline, 24., true).child(*label)),
    )
}

pub(super) fn section(row: &'static Row) -> AnyElement {
    match row.title {
        "Interaction report" => report(row, true),
        "Latency report" => report(row, false),
        "MCP port" => settings_row(
            row.title,
            row.description.into(),
            disabled(row, field("MCP port", "")),
        )
        .into_any_element(),
        "Agent access token" => settings_row(
            row.title,
            row.description.into(),
            disabled(row, actions(&["Copy", "Rotate"])),
        )
        .into_any_element(),
        _ => {
            settings_row(row.title, row.description.into(), disabled(row, div())).into_any_element()
        }
    }
}

fn field(label: &'static str, placeholder: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::medium(ui::sm(label)))
        .child(
            div()
                .h(px(32.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::input())
                .bg(Tokens::input().opacity(0.3))
                .child(ui::muted(ui::xs(placeholder))),
        )
}

fn table(headers: &[&'static str], empty: &'static str) -> Div {
    div()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .overflow_hidden()
        .child(
            div()
                .flex()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .bg(Tokens::muted().opacity(0.3))
                .children(
                    headers
                        .iter()
                        .map(|header| ui::medium(ui::xs(*header)).flex_1()),
                ),
        )
        .child(ui::muted(ui::xs(empty)).p(px(12.)))
}

fn report(row: &'static Row, interactions: bool) -> AnyElement {
    let mut body = div().flex().flex_col().gap(px(16.)).mt(px(16.));
    if interactions {
        body = body
            .child(ui::muted(ui::xs("0 interactions recorded.")))
            .child(table(
                &[
                    "Time",
                    "Kind",
                    "Target",
                    "Dropped",
                    "Max frame",
                    "To paint",
                    "DOM",
                    "Top source",
                ],
                "No interactions with dropped frames yet.",
            ));
    } else {
        body = body.child(ui::muted(ui::xs("Frontend 0 events, backend 0 events.")));
        for (label, headers, empty) in [
            (
                "Frontend timings",
                &["Name", "Count", "p50", "p95", "Max"][..],
                "No timed frontend spans yet.",
            ),
            (
                "Render counters",
                &["Name", "Count"][..],
                "No render counts yet.",
            ),
            (
                "Backend timings",
                &["Name", "Count", "p50", "p95", "Max"][..],
                "No backend timings yet.",
            ),
        ] {
            body = body.child(
                div()
                    .child(ui::medium(ui::xs(label)).mb(px(8.)))
                    .child(table(headers, empty)),
            );
        }
        body =
            body.child(div().child(ui::medium(ui::xs("Backend caches"))).child(
                ui::muted(ui::xs("Enable diagnostics to inspect backend caches.")).mt(px(8.)),
            ));
    }
    panel(row)
        .child(heading(
            row,
            disabled(
                row,
                actions(if interactions {
                    &["Clear", "Copy"]
                } else {
                    &["Refresh", "Clear", "Copy"]
                }),
            ),
        ))
        .child(body)
        .child(
            ui::muted(ui::xs(
                row.unavailable.expect("report needs a host adapter"),
            ))
            .mt(px(12.)),
        )
        .into_any_element()
}
