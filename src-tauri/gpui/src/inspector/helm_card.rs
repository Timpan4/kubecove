//! Helm release cards and list rows from `HelmView.svelte`, plus the `Alert`,
//! `Empty` and table pieces its details card shares.

use super::Inspector;
use super::controls::Variant;
use super::delivery_ui::{self as dui, CARD_GAP, CARD_HEIGHT};
use crate::gitops_surface::status_label;
use crate::helm::{self, Tone};
use crate::targets::TestSupportExt;
use crate::theme::{HEADING, Palette, Tokens};
use crate::ui;
use gpui_kit::{Context, Div, FontWeight, Hsla, ScrollHandle, SharedString, div, prelude::*, px};
use kubecove_lib::models::HelmReleaseSummary;

/// `helmStatusRailClass`.
fn rail_color(status: Option<&str>) -> Hsla {
    match helm::status_tone(status) {
        Tone::Success => Palette::emerald500(1.),
        Tone::Warning => Palette::amber500(1.),
        Tone::Error => Tokens::destructive(),
        Tone::Neutral => Tokens::muted_foreground().opacity(0.45),
    }
}

/// The status `Badge`: `helmStatusVariant` plus `helmStatusClass`.
pub(super) fn status_badge(tone: Tone, label: impl Into<SharedString>) -> Div {
    dui::tinted_badge(
        match tone {
            Tone::Success => ui::Tone::Success,
            Tone::Warning => ui::Tone::Warning,
            Tone::Error => ui::Tone::Error,
            Tone::Neutral => ui::Tone::Neutral,
        },
        label,
    )
}

fn release_badge(release: &HelmReleaseSummary) -> Option<Div> {
    let status = release.status.as_deref()?;
    Some(status_badge(
        helm::status_tone(Some(status)),
        status_label(status),
    ))
}

/// The Details and resources buttons.
fn actions(
    index: usize,
    release: &HelmReleaseSummary,
    resources: &str,
    cx: &mut Context<Inspector>,
) -> Div {
    let key = format!("{}:{}", release.namespace, release.name);
    let (namespace, name) = (release.namespace.clone(), release.name.clone());
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            dui::sm_button(Variant::Ghost, "Details")
                .id(SharedString::from(format!("helm-details:{key}")))
                .test_support()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.select_helm_release(Some(index), cx)),
                ),
        )
        .child(
            dui::sm_button(Variant::Outline, resources)
                .id(SharedString::from(format!("helm-resources:{key}")))
                .test_support()
                .ml_auto()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_helm_resources(namespace.clone(), name.clone(), window, cx)
                })),
        )
}

/// `rounded-md border bg-surface-0 px-2.5 py-1.5` with a `text-[0.62rem]` label.
fn fact(label: &str, value: String) -> Div {
    div()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background())
        .child(ui::muted(ui::text(label.to_owned(), 9.92, 16.)))
        .child(ui::medium(ui::xs(value)).truncate())
}

/// `Card size="sm" elevation="raised"` at `h-[18.5rem] p-0`. The card's
/// `data-[size=sm]:py-3` and the sections' `group-data-[size=sm]/card:px-3`
/// outrank `p-0` and `pl-4`, so every side keeps 12px.
pub(super) fn card(
    index: usize,
    release: &HelmReleaseSummary,
    selected: bool,
    width: f32,
    cx: &mut Context<Inspector>,
) -> impl IntoElement + use<> {
    let dash = |value: Option<String>| value.unwrap_or_else(|| "-".into());
    div()
        .id(SharedString::from(format!(
            "helm-release:{}:{}",
            release.namespace, release.name
        )))
        .test_support()
        .aria_label(release.name.clone())
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(12.))
        .py(px(12.))
        .w(px(width))
        .h(px(CARD_HEIGHT))
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .shadow_sm()
        .when(selected, |card| {
            card.border_color(Tokens::ring().opacity(0.4))
        })
        .hover(|card| {
            card.border_color(Tokens::primary().opacity(0.3))
                .shadow_md()
        })
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(4.))
                .rounded_l(px(10.))
                .bg(rail_color(release.status.as_deref())),
        )
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .px(px(12.))
                .child(
                    div()
                        .min_w_0()
                        .child(
                            ui::text(release.name.clone(), 14., 20.)
                                .font_family(HEADING)
                                .font_weight(FontWeight::MEDIUM)
                                .truncate(),
                        )
                        .child(
                            ui::muted(ui::text(release.namespace.clone(), 12., 19.5)).truncate(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .child(ui::icon("package", 14., Tokens::muted_foreground())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap(px(12.))
                .px(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .min_h(px(20.))
                        .gap(px(6.))
                        .children(release_badge(release)),
                )
                .child(
                    ui::muted(ui::xs(format!(
                        "{} · {}",
                        release.storage_kind, release.storage_name
                    )))
                    .truncate()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::background()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(fact("Chart", dash(release.chart.clone())))
                                .child(fact("App version", dash(release.app_version.clone()))),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(fact(
                                    "Revision",
                                    dash(release.revision.map(|r| r.to_string())),
                                ))
                                .child(fact("Age", release.age.clone())),
                        ),
                ),
        )
        .child(
            div()
                .mt_auto()
                .px(px(12.))
                .child(actions(index, release, "View resources", cx)),
        )
        // `ring-2 ring-ring/40` on the selected card.
        .when(selected, |card| {
            card.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(10.))
                    .border_2()
                    .border_color(Tokens::ring().opacity(0.4)),
            )
        })
}

/// `grid-cols-[minmax(12rem,1.1fr)_8rem_minmax(14rem,1.2fr)_minmax(9rem,0.9fr)
/// _minmax(7rem,0.7fr)_5rem_5rem_9rem]` at `min-w-[76rem]`.
fn columns(width: f32) -> [f32; 8] {
    dui::tracks(
        [
            (192., 1.1),
            (128., 0.),
            (224., 1.2),
            (144., 0.9),
            (112., 0.7),
            (80., 0.),
            (80., 0.),
            (144., 0.),
        ],
        1216.,
        width,
    )
}

pub(super) fn list_header(width: f32, scroll: &ScrollHandle) -> Div {
    let labels = [
        "Name",
        "Status",
        "Storage",
        "Chart",
        "App version",
        "Revision",
        "Age",
        "Actions",
    ];
    let body = div()
        .flex()
        .gap(px(CARD_GAP))
        .px(px(12.))
        .py(px(8.))
        .overflow_hidden()
        .border_b_1()
        .border_color(Tokens::border())
        .bg(Tokens::muted().opacity(0.3))
        .children(labels.iter().zip(columns(width)).map(|(label, width)| {
            ui::semibold(ui::muted(ui::text(label.to_uppercase(), 10.4, 14.)))
                .flex_none()
                .w(px(width))
                .whitespace_nowrap()
                .when(*label == "Actions", |cell| cell.flex().justify_end())
        }));
    dui::horizontal(body, width, 1216., scroll, "helm-columns".into())
}

pub(super) fn list_row(
    index: usize,
    release: &HelmReleaseSummary,
    selected: bool,
    width: f32,
    scroll: &ScrollHandle,
    cx: &mut Context<Inspector>,
) -> impl IntoElement + use<> {
    let widths = columns(width - 2.);
    let cell = |index: usize| div().flex_none().w(px(widths[index])).min_w_0();
    let text = |value: Option<String>| ui::sm(value.unwrap_or_else(|| "-".into())).truncate();
    let body = div()
        .id(SharedString::from(format!(
            "helm-release:{}:{}",
            release.namespace, release.name
        )))
        .test_support()
        .aria_label(release.name.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(CARD_GAP))
        .px(px(12.))
        .py(px(10.))
        .overflow_hidden()
        .bg(if selected {
            Tokens::muted()
        } else {
            Tokens::card()
        })
        .when(!selected, |row| {
            row.hover(|row| row.bg(Tokens::muted().opacity(0.4)))
        })
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(4.))
                .bg(rail_color(release.status.as_deref())),
        )
        .child(
            cell(0)
                .pl(px(4.))
                .child(
                    ui::text(release.name.clone(), 14., 20.)
                        .font_family(HEADING)
                        .font_weight(FontWeight::MEDIUM)
                        .truncate(),
                )
                .child(ui::muted(ui::xs(release.namespace.clone())).truncate()),
        )
        .child(match release_badge(release) {
            Some(badge) => cell(1).flex().child(badge),
            None => cell(1).child(ui::muted(ui::sm("-"))),
        })
        .child(
            cell(2)
                .child(ui::sm(release.storage_kind.clone()).truncate())
                .child(ui::muted(ui::xs(release.storage_name.clone())).truncate()),
        )
        .child(cell(3).child(text(release.chart.clone())))
        .child(cell(4).child(text(release.app_version.clone())))
        .child(cell(5).child(text(release.revision.map(|revision| revision.to_string()))))
        .child(cell(6).child(ui::sm(release.age.clone())))
        .child(
            cell(7)
                .flex()
                .justify_end()
                .child(actions(index, release, "Resources", cx).gap(px(4.))),
        )
        // `ring-1 ring-ring` on the selected row.
        .when(selected, |row| {
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .border_1()
                    .border_color(Tokens::ring()),
            )
        });
    dui::horizontal(
        div().child(body),
        width,
        1216.,
        scroll,
        format!("helm-columns:{}:{}", release.namespace, release.name),
    )
}

/// `Empty` for no releases or no matches, with Clear filters when filtered.
pub(super) fn empty(no_releases: bool, cx: &mut Context<Inspector>) -> impl IntoElement + use<> {
    let (title, description) = if no_releases {
        (
            "No Helm releases",
            "No Helm release storage objects were found in this cluster.",
        )
    } else {
        (
            "No matching releases",
            "Clear the search or choose another namespace.",
        )
    };
    let clear = (!no_releases).then(|| {
        dui::sm_button(Variant::Outline, "Clear filters")
            .id("helm-clear-filters")
            .test_support()
            .on_click(cx.listener(|this, _, window, cx| this.clear_helm_filters(window, cx)))
            .into_any_element()
    });
    dui::empty("helm-empty", title, description, clear)
}

/// `Alert` (`rounded-lg border px-2 py-1.5 text-xs/relaxed`) with a bulleted list.
pub(super) fn alert(title: &str, items: &[String]) -> Div {
    div()
        .px(px(8.))
        .py(px(6.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(
            ui::medium(ui::text(title.to_owned(), 12., 19.5))
                .id(SharedString::from(format!("alert:{title}")))
                .test_support()
                .aria_label(title.to_owned())
                .aria_description(items.join(" ")),
        )
        .child(
            div()
                .mt(px(2.))
                .pl(px(16.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(items.iter().map(|item| {
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(ui::muted(ui::text("•", 12., 19.5)))
                        .child(ui::muted(ui::text(item.clone(), 12., 19.5)))
                })),
        )
}

/// `SimpleTable`-style table: `table-fixed` equal columns at least `min_width`
/// wide, `px-3 py-2` uppercase heads and `border-b` rows, clipped to the box.
pub(super) fn table(
    headers: &[&str],
    rows: Vec<Vec<Div>>,
    empty: &str,
    min_width: f32,
    width: f32,
) -> Div {
    let column = width.max(min_width) / headers.len() as f32;
    let row = |cells: Vec<Div>| {
        div()
            .flex()
            .border_b_1()
            .border_color(Tokens::border())
            .children(cells.into_iter().map(|cell| {
                cell.flex_none()
                    .w(px(column))
                    .min_w_0()
                    .px(px(12.))
                    .py(px(8.))
            }))
    };
    div()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .h(px(40.))
                .items_center()
                .border_b_1()
                .border_color(Tokens::border())
                .children(headers.iter().map(|header| {
                    ui::semibold(ui::muted(ui::xs(header.to_uppercase())))
                        .flex_none()
                        .w(px(column))
                        .px(px(12.))
                        .whitespace_nowrap()
                })),
        )
        .when(rows.is_empty(), |table| {
            table.child(
                ui::muted(ui::sm(empty.to_owned()))
                    .w(px(width))
                    .py(px(32.))
                    .flex()
                    .justify_center()
                    .border_b_1()
                    .border_color(Tokens::border()),
            )
        })
        .children(rows.into_iter().map(row))
}
