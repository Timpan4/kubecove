//! The Helm release details card in `HelmView.svelte`, with the
//! `helmDetailsQuery` and `helmReconciliationQuery` reads from `HelmSurface.svelte`.

use super::delivery_ui as dui;
use super::helm_card::{self, alert, table};
use super::{Inspector, friendly_error::Operation, surface};
use crate::gitops_surface::status_label;
use crate::helm;
use crate::theme::{HEADING, MONO, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{Context, Div, FontWeight, SharedString, Task, div, prelude::*, px};
use kubecove_lib::models::{AppError, HelmReleaseDetails, HelmReleaseReconciliation};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
enum Load<T> {
    #[default]
    Loading,
    Ready(T),
    Failed(AppError),
}

/// The details read, with its YAML kept as shared text for rendering.
struct Details {
    data: HelmReleaseDetails,
    yaml: SharedString,
}

#[derive(Default)]
pub(super) struct HelmDetails {
    details: Load<Box<Details>>,
    /// Resources sorted by `sortHelmReconciliationResources` when read.
    reconciliation: Load<Box<HelmReleaseReconciliation>>,
    request: Option<Rc<()>>,
    tasks: [Option<Task<()>>; 2],
    read_at: [Option<Instant>; 2],
    errors: [Option<AppError>; 2],
    fetching: [bool; 2],
}

impl HelmDetails {
    pub(super) fn cancel(&mut self) {
        self.tasks = [None, None];
        self.request = None;
        self.fetching = [false; 2];
    }
}

impl Inspector {
    /// `list.select`: opens or closes the details card and reads the release.
    pub(super) fn select_helm_release(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let page = &mut self.helm_page;
        page.details = HelmDetails::default();
        page.selected = index;
        self.refresh_helm_details(cx);
    }

    pub(super) fn refresh_helm_details(&mut self, cx: &mut Context<Self>) {
        let page = &mut self.helm_page;
        let Some(release) = page
            .selected
            .and_then(|index| page.releases().get(index).cloned())
        else {
            cx.notify();
            return;
        };
        let now = cx.background_executor().now();
        // HelmSurface.svelte: details staleTime 15_000, reconciliation 30_000.
        let stale = [15_000, 30_000].map(Duration::from_millis);
        let refresh = [0, 1].map(|index| {
            !page.details.fetching[index]
                && page.details.read_at[index]
                    .is_none_or(|at| now.duration_since(at) >= stale[index])
        });
        let request = page
            .details
            .request
            .get_or_insert_with(|| Rc::new(()))
            .clone();
        let details = refresh[0].then(|| {
            let backend = self.backend.clone();
            let release = release.clone();
            crate::read_retry::read(cx.background_executor().clone(), move || {
                backend.helm_release(release.clone())
            })
        });
        let reconciliation = refresh[1].then(|| {
            let backend = self.backend.clone();
            crate::read_retry::read(cx.background_executor().clone(), move || {
                backend.helm_reconciliation(release.clone())
            })
        });
        let current = move |this: &Inspector| {
            this.helm_page
                .details
                .request
                .as_ref()
                .is_some_and(|current| Rc::ptr_eq(current, &request))
        };
        let current_too = current.clone();
        if let Some(details) = details {
            self.helm_page.details.errors[0] = None;
            if matches!(self.helm_page.details.details, Load::Failed(_)) {
                self.helm_page.details.details = Load::Loading;
            }
            self.helm_page.details.fetching[0] = true;
            self.helm_page.details.tasks[0] = Some(cx.spawn(async move |this, cx| {
                let result = details.await;
                let _ = this.update(cx, |this, cx| {
                    if !current(this) {
                        return;
                    }
                    let state = &mut this.helm_page.details;
                    state.fetching[0] = false;
                    if let Err(error) = &result
                        && matches!(state.details, Load::Ready(_))
                    {
                        state.errors[0] = Some(error.clone());
                        cx.notify();
                        return;
                    }
                    state.errors[0] = None;
                    state.read_at[0] = result.is_ok().then(|| cx.background_executor().now());
                    state.details = match result {
                        Ok(data) => Load::Ready(Box::new(Details {
                            yaml: if data.yaml.is_empty() {
                                "No YAML returned".into()
                            } else {
                                // The web's `<pre>` does not add a final empty line.
                                data.yaml
                                    .strip_suffix('\n')
                                    .unwrap_or(&data.yaml)
                                    .to_owned()
                                    .into()
                            },
                            data,
                        })),
                        Err(error) => Load::Failed(error),
                    };
                    cx.notify();
                });
            }));
        }
        if let Some(reconciliation) = reconciliation {
            self.helm_page.details.errors[1] = None;
            if matches!(self.helm_page.details.reconciliation, Load::Failed(_)) {
                self.helm_page.details.reconciliation = Load::Loading;
            }
            self.helm_page.details.fetching[1] = true;
            self.helm_page.details.tasks[1] = Some(cx.spawn(async move |this, cx| {
                let result = reconciliation.await;
                let _ = this.update(cx, |this, cx| {
                    if !current_too(this) {
                        return;
                    }
                    let state = &mut this.helm_page.details;
                    state.fetching[1] = false;
                    if let Err(error) = &result
                        && matches!(state.reconciliation, Load::Ready(_))
                    {
                        state.errors[1] = Some(error.clone());
                        cx.notify();
                        return;
                    }
                    state.errors[1] = None;
                    state.read_at[1] = result.is_ok().then(|| cx.background_executor().now());
                    state.reconciliation = match result {
                        Ok(mut data) => {
                            helm::sort_reconciliation(&mut data.resources);
                            Load::Ready(Box::new(data))
                        }
                        Err(error) => Load::Failed(error),
                    };
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// The details card renders as a list item after the frame, so the page
    /// observes its loading phases during render.
    pub(super) fn observe_helm_details_loading(&self) {
        if self.helm_page.selected.is_none() {
            return;
        }
        let details = &self.helm_page.details;
        self.loading
            .phase("helm-details", matches!(details.details, Load::Loading));
        self.loading.phase(
            "helm-reconciliation",
            matches!(details.reconciliation, Load::Loading),
        );
    }

    pub(super) fn helm_details_card(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let Some(release) = page.selected.and_then(|index| page.releases().get(index)) else {
            return div();
        };
        let (namespace, name) = (release.namespace.clone(), release.name.clone());
        let header = div()
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
                            .id("helm-details-title")
                            .test_support()
                            .aria_label(release.name.clone()),
                    )
                    .child({
                        let description = format!(
                            "{} / {}:{}",
                            release.namespace, release.storage_kind, release.storage_name
                        );
                        ui::muted(ui::text(description.clone(), 12., 19.5))
                            .id("helm-details-description")
                            .test_support()
                            .aria_label(description)
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        surface::small_button(Some("external-link"), "View resources")
                            .id("helm-details-resources")
                            .test_support()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_helm_resources(
                                    namespace.clone(),
                                    name.clone(),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        ui::ghost_icon("x", 28., 14.)
                            .id("helm-close")
                            .test_support()
                            .aria_label("Close Helm details")
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, _, cx| this.select_helm_release(None, cx)),
                            ),
                    ),
            );
        let phase = self.loading.phase(
            "helm-details",
            matches!(page.details.details, Load::Loading),
        );
        let content = match &page.details.details {
            // HelmView.svelte checks the loading state first.
            _ if phase.shown() => pending("Loading Helm release details..."),
            _ if !phase.ready() => div(),
            Load::Loading => div(),
            Load::Failed(error) => div().child(self.page_read_failure(
                "Helm details unavailable",
                error,
                Operation::Details,
                cx,
            )),
            Load::Ready(details) => self.helm_details_body(details, cx),
        }
        .children(page.details.errors.iter().flatten().map(|error| {
            self.page_read_failure("Helm refresh failed", error, Operation::Details, cx)
        }));
        div().child(
            div()
                .id("helm-release-details")
                .test_support()
                .flex()
                .flex_col()
                .gap(px(12.))
                .py(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::card())
                .child(header)
                .child(div().px(px(12.)).child(content)),
        )
    }

    fn helm_details_body(&self, details: &Details, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let summary = &details.data.summary;
        let values = &details.data.values_summary;
        let manifest = &details.data.manifest_summary;
        // `lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]`: the viewport `lg` breakpoint.
        let columns = page.details_columns;
        let width = page.layout.grid.max(0.)
            + if page.layout.rail_beside {
                dui::RAIL + 16.
            } else {
                0.
            };
        let inner = width - 24. - 2.;
        let left = if columns { (inner - 16.) * 0.4 } else { inner };
        let dash = |value: Option<String>| value.unwrap_or_else(|| "-".into());
        let facts = [
            ("Chart", dash(summary.chart.clone())),
            ("App version", dash(summary.app_version.clone())),
            (
                "Revision",
                dash(summary.revision.map(|revision| revision.to_string())),
            ),
            (
                "Status",
                status_label(summary.status.as_deref().unwrap_or("unknown")),
            ),
            (
                "Updated",
                summary
                    .updated_at
                    .as_ref()
                    .map_or("\u{2014}".to_owned(), |_| summary.age.clone()),
            ),
            (
                "Values",
                if values.has_values {
                    format!("{} values", values.value_count)
                } else {
                    "No values".into()
                },
            ),
        ];
        let label = |text: &str| ui::muted(ui::xs(text.to_uppercase()));
        let manifest_rows = manifest
            .resources
            .iter()
            .map(|resource| {
                [
                    &resource.kind,
                    &resource.name,
                    &resource.namespace,
                    &resource.api_version,
                ]
                .map(|value| ui::sm(value.clone().unwrap_or_else(|| "-".into())).truncate())
                .into()
            })
            .collect();
        let left_column = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(facts.chunks(2).map(|pair| {
                        div()
                            .flex()
                            .gap(px(8.))
                            .children(pair.iter().map(|(name, value)| {
                                div()
                                    .flex_1()
                                    .flex_basis(px(0.))
                                    .min_w_0()
                                    .child(label(name))
                                    .child(ui::medium(ui::sm(value.clone())).truncate())
                            }))
                    })),
            )
            .child(
                div().child(label("Top-level values").mb(px(8.))).child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .when(values.top_level_keys.is_empty(), |row| {
                            row.child(ui::muted(ui::sm("None decoded")))
                        })
                        .children(
                            values
                                .top_level_keys
                                .iter()
                                .map(|key| ui::outline_badge(key.clone())),
                        ),
                ),
            )
            .child(
                div()
                    .child(
                        label(&format!("Manifest resources ({})", manifest.resource_count))
                            .mb(px(8.)),
                    )
                    .child(
                        div()
                            .p(px(12.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::card())
                            .overflow_hidden()
                            .child(table(
                                &["Kind", "Name", "Namespace", "API"],
                                manifest_rows,
                                "No manifest resources decoded",
                                720.,
                                left - 26.,
                            )),
                    )
                    .when(manifest.truncated, |block| {
                        block.child(
                            ui::muted(ui::xs("Manifest list truncated by backend.")).mt(px(8.)),
                        )
                    }),
            )
            .child(self.helm_reconciliation(left, cx));
        let yaml = div()
            .min_w_0()
            .flex_1()
            .child(label("Rendered YAML").mb(px(8.)))
            .child(
                div()
                    .id("helm-yaml")
                    .max_h(px(320.))
                    .overflow_scroll()
                    .p(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::muted().opacity(0.3))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .line_height(px(19.5))
                    .whitespace_nowrap()
                    .child(details.yaml.clone()),
            );
        if columns {
            div()
                .flex()
                .gap(px(16.))
                .child(left_column.w(px(left)).flex_none())
                .child(yaml)
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(left_column)
                .child(yaml)
        }
    }

    fn helm_reconciliation(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let state = &self.helm_page.details.reconciliation;
        let phase = self
            .loading
            .phase("helm-reconciliation", matches!(state, Load::Loading));
        let totals = match state {
            Load::Ready(data) if phase.ready() => Some(format!(
                "{} tracked, {} missing, {} label-only",
                data.totals.tracked, data.totals.missing, data.totals.label_only
            )),
            _ => None,
        };
        let heading = div()
            .mb(px(8.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(ui::muted(ui::xs("RECONCILIATION")))
            .when_some(totals, |row, totals| {
                row.child(
                    ui::muted(ui::xs(totals.clone()))
                        .id("helm-details-totals")
                        .test_support()
                        .aria_label(totals),
                )
            });
        let body = match state {
            _ if phase.shown() => pending("Loading reconciliation..."),
            _ if !phase.ready() => div(),
            Load::Loading => div(),
            Load::Failed(error) => div().child(self.partial_read_failure(
                "Helm reconciliation unavailable",
                error,
                Operation::Resources,
                cx,
            )),
            Load::Ready(data) => {
                let rows = data
                    .resources
                    .iter()
                    .map(|resource| {
                        let tone = helm::reconciliation_tone(resource.status);
                        vec![
                            ui::xs(helm::resource_label(resource)).truncate(),
                            ui::xs(resource.namespace.clone().unwrap_or_else(|| "-".into()))
                                .truncate(),
                            div().flex().child(helm_card::status_badge(
                                tone,
                                helm::reconciliation_label(resource.status),
                            )),
                            ui::xs(helm::reconciliation_source(resource)).truncate(),
                            ui::xs(resource.status_message.clone()).truncate(),
                        ]
                    })
                    .collect();
                div()
                    .when(!data.warnings.is_empty(), |body| {
                        body.child(alert("Reconciliation notes", &data.warnings).mb(px(12.)))
                    })
                    .child(
                        div()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .overflow_hidden()
                            .child(table(
                                &["Resource", "Namespace", "Status", "Source", "Message"],
                                rows,
                                "No manifest or explicit Helm-labeled live resources were found.",
                                760.,
                                width - 2.,
                            )),
                    )
            }
        };
        div().child(heading).child(body)
    }
}

/// `inline-flex items-center gap-2 text-sm text-muted-foreground` with a `Spinner`.
fn pending(label: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
        .child(
            ui::muted(ui::sm(label.to_owned()))
                .id(SharedString::from(format!("helm-pending:{label}")))
                .test_support()
                .aria_label(label.to_owned()),
        )
}
