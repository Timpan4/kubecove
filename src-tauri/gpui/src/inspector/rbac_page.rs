//! RBAC Cockpit, mirroring `RbacSurface.svelte` and `RbacView.svelte`: state,
//! loading, the access posture and page layout. `rbac_queue.rs` renders the
//! toolbar and queue, `rbac_inspector.rs` the selected object's evidence.

use super::controls::{Variant, button};
use super::rbac_queue::{has_level, tinted_badge};
use super::rbac_verify::Verifier;
use super::{Inspector, Page, friendly_error::Operation, surface};
use crate::rbac::permissions::{Identity, IdentityKind, Observed, observed};
use crate::rbac::review::ReviewRecord;
use crate::rbac::{self, Bucket, CockpitItem, ObjectRef, RbacView};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    Context, Div, Entity, FontWeight, Subscription, Task, TestSupportExt, Window, div, prelude::*,
    px,
};
use kubecove_lib::models::{AppError, RbacCoverageStatus, RbacInspectionSummary, RbacRiskLevel};
use std::rc::Rc;

/// `visible` in `RbacView.svelte`: the queue renders 50 cards and 50 more per click.
pub(super) const PAGE: usize = 50;

pub(super) enum RbacLoad {
    Loading,
    Ready(Box<RbacInspectionSummary>),
    Failed(AppError),
}

/// `IdentitySource`: which identity the inspector and verifier use.
#[derive(Clone)]
pub(super) enum IdentitySource {
    Selection,
    Manual,
    Subject(Identity),
}

pub(super) struct RbacState {
    context: String,
    pub(super) load: RbacLoad,
    pub(super) view: RbacView,
    /// Opened on a kind child, which the header names instead of "RBAC".
    pub(super) kind_location: bool,
    pub(super) bucket: Bucket,
    pub(super) search: Entity<InputState>,
    pub(super) items: Vec<CockpitItem>,
    /// Each risk filter's count, rebuilt with `items`.
    pub(super) counts: [usize; 6],
    pub(super) filtered: Vec<usize>,
    pub(super) shown: usize,
    selected: Option<String>,
    pub(super) identity: IdentitySource,
    pub(super) verifier: Verifier,
    pub(super) reviews: Vec<ReviewRecord>,
    pub(super) review_editor_key: Option<(String, String, Option<String>)>,
    /// Observed grants for the identity they were computed for.
    observed: Option<(Identity, Observed)>,
    request: Rc<()>,
    task: Option<Task<()>>,
    refresh_error: Option<AppError>,
    _subscriptions: Vec<Subscription>,
}

impl RbacState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(search_placeholder(RbacView::ServiceAccounts))
        });
        let mut subscriptions = vec![cx.subscribe_in(&search, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.rbac.shown = PAGE;
                this.refilter_rbac(cx);
            }
        })];
        let verifier = Verifier::new(window, cx, &mut subscriptions);
        Self {
            context: String::new(),
            load: RbacLoad::Loading,
            view: RbacView::ServiceAccounts,
            kind_location: false,
            bucket: Bucket::All,
            search,
            items: vec![],
            counts: [0; 6],
            filtered: vec![],
            shown: PAGE,
            selected: None,
            identity: IdentitySource::Selection,
            verifier,
            reviews: vec![],
            review_editor_key: None,
            observed: None,
            request: Rc::new(()),
            task: None,
            refresh_error: None,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn location(&self) -> &'static str {
        if self.kind_location {
            self.view.label()
        } else {
            "RBAC"
        }
    }

    pub(super) fn data(&self) -> Option<&RbacInspectionSummary> {
        match &self.load {
            RbacLoad::Ready(data) => Some(data),
            _ => None,
        }
    }

    /// `selectedCockpitItem`: the selected key among the filtered items, or the first.
    pub(super) fn selected_item(&self) -> Option<&CockpitItem> {
        let mut filtered = self.filtered.iter().map(|index| &self.items[*index]);
        let first = filtered.clone().next();
        self.selected
            .as_ref()
            .and_then(|key| filtered.find(|item| &item.key == key))
            .or(first)
    }

    /// `activeIdentity`.
    pub(super) fn active_identity(&self, cx: &gpui_kit::App) -> Identity {
        match &self.identity {
            IdentitySource::Subject(identity) => return identity.clone(),
            IdentitySource::Selection => {
                if let (Some(data), Some(item)) = (self.data(), self.selected_item())
                    && let ObjectRef::Account(account) = item.object.get(data)
                {
                    return Identity {
                        kind: IdentityKind::ServiceAccount,
                        name: account.name.clone(),
                        namespace: Some(account.namespace.clone()),
                        groups: vec![],
                    };
                }
            }
            IdentitySource::Manual => {}
        }
        self.verifier.manual_identity(cx)
    }
}

/// `Search {view lowercase}…`.
fn search_placeholder(view: RbacView) -> String {
    format!("Search {}…", view.label().to_lowercase())
}

impl Inspector {
    /// Opens the cockpit on a category, or on Service Accounts from the section.
    pub(super) fn open_rbac(
        &mut self,
        view: Option<RbacView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rbac.kind_location = view.is_some();
        self.set_rbac_view(view.unwrap_or(RbacView::ServiceAccounts), window, cx);
        if self.page == Page::Rbac {
            cx.notify();
        } else {
            self.open_page(Page::Rbac, cx);
        }
    }

    pub(super) fn load_rbac(&mut self, cx: &mut Context<Self>) {
        if self.scope.context.is_empty() {
            return;
        }
        let stale = self.page_read_stale(Page::Rbac, cx);
        let state = &mut self.rbac;
        if state.context == self.scope.context
            && !matches!(state.load, RbacLoad::Failed(_))
            && !stale
        {
            return;
        }
        if state.context != self.scope.context {
            state.load = RbacLoad::Loading;
            state.selected = None;
            state.identity = IdentitySource::Selection;
            state.observed = None;
        }
        state.context = self.scope.context.clone();
        // A failed refresh stays visible until the next read resolves.
        state.request = Rc::new(());
        let request = state.request.clone();
        let backend = self.backend.clone();
        let context = state.context.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.rbac(context.clone())
        });
        let read = self.timings.measure("rbac", read);
        state.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.rbac.request, &request) {
                    return;
                }
                let success = result.is_ok();
                match result {
                    Ok(data) => {
                        this.rbac.load = RbacLoad::Ready(Box::new(data));
                        this.rbac.observed = None;
                        this.rbac.refresh_error = None;
                    }
                    Err(error) => {
                        if matches!(this.rbac.load, RbacLoad::Ready(_)) {
                            this.rbac.refresh_error = Some(error);
                        } else {
                            this.rbac.load = RbacLoad::Failed(error);
                        }
                    }
                }
                if success {
                    this.page_read_finished(Page::Rbac, cx);
                } else {
                    this.invalidate_page_read(Page::Rbac);
                }
                this.rebuild_rbac_items(cx);
            });
        }));
        cx.notify();
    }

    pub(super) fn refresh_rbac(&mut self, cx: &mut Context<Self>) {
        self.invalidate_page_read(Page::Rbac);
        self.load_rbac(cx);
    }

    pub(super) fn cancel_rbac_read(&mut self) {
        self.rbac.task.take();
        self.rbac.request = Rc::new(());
    }

    /// `setView`: clears the search, selection and subject.
    pub(super) fn set_rbac_view(
        &mut self,
        view: RbacView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.rbac;
        if state.view == view && !state.items.is_empty() {
            return;
        }
        state.view = view;
        state.selected = None;
        state.identity = IdentitySource::Selection;
        state.shown = PAGE;
        state.search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(search_placeholder(view), window, cx);
        });
        self.rebuild_rbac_items(cx);
    }

    fn rebuild_rbac_items(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.rbac;
        state.items = match &state.load {
            RbacLoad::Ready(data) => rbac::cockpit_items(data, state.view),
            _ => vec![],
        };
        state.counts = rbac::bucket_counts(&state.items);
        self.refilter_rbac(cx);
    }

    pub(super) fn refilter_rbac(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.rbac;
        let search = state.search.read(cx).value();
        state.filtered = rbac::filter_items(&state.items, state.bucket, &search);
        state.verifier.reset();
        cx.notify();
    }

    pub(super) fn select_rbac_item(&mut self, key: String, cx: &mut Context<Self>) {
        self.rbac.selected = Some(key);
        self.rbac.identity = IdentitySource::Selection;
        self.rbac.verifier.reset();
        cx.notify();
    }

    /// `inspectSubject`.
    pub(super) fn investigate_subject(&mut self, identity: Identity, cx: &mut Context<Self>) {
        self.rbac.identity = IdentitySource::Subject(identity);
        self.rbac.verifier.reset();
        cx.notify();
    }

    /// Observed grants for the active identity, recomputed only when it changes.
    pub(super) fn rbac_observed(&mut self, cx: &gpui_kit::App) -> Option<&Observed> {
        let identity = self.rbac.active_identity(cx);
        let state = &mut self.rbac;
        let data = state.data()?;
        if identity.name.is_empty() {
            state.observed = None;
            return None;
        }
        if state
            .observed
            .as_ref()
            .is_none_or(|(cached, _)| *cached != identity)
        {
            let grants = observed(data, &identity);
            state.observed = Some((identity, grants));
        }
        state.observed.as_ref().map(|(_, grants)| grants)
    }

    pub(super) fn rbac_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        self.sync_rbac_review_note(window, cx);
        let pending = surface::pending_slot(
            self.loading
                .phase("surface-rbac", matches!(self.rbac.load, RbacLoad::Loading)),
        );
        let body = match (&self.rbac.load, pending) {
            (_, Some(slot)) => slot,
            (RbacLoad::Loading, None) => div().into_any_element(),
            (RbacLoad::Failed(error), None) => {
                self.page_read_failure("RBAC inspection unavailable", error, Operation::Surface, cx)
            }
            (RbacLoad::Ready(_), None) => self.rbac_body(cx).into_any_element(),
        };
        surface::frame(
            "key-round",
            "RBAC Cockpit",
            true,
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .when_some(self.rbac.refresh_error.clone(), |body, error| {
                    body.child(self.page_read_failure(
                        "RBAC refresh failed",
                        &error,
                        Operation::Surface,
                        cx,
                    ))
                })
                .child(body),
        )
    }

    fn rbac_body(&mut self, cx: &mut Context<Self>) -> Div {
        let inspector = self.rbac_inspector(cx);
        let data = self.rbac.data().expect("ready");
        let complete = data
            .coverage
            .iter()
            .filter(|entry| entry.status == RbacCoverageStatus::Complete)
            .count();
        let total = data.coverage.len();
        let warnings = data.warnings.join("\n");
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(tinted_badge(
                        if total > 0 && complete == total {
                            "Coverage complete"
                        } else {
                            "Coverage partial"
                        },
                        Palette::emerald300(),
                        Palette::emerald500(0.35),
                        Palette::emerald500(0.1),
                    ))
                    .child(ui::outline_badge("Entire cluster")),
            )
            .child(self.rbac_posture(complete, total, cx))
            .when(!warnings.is_empty(), |body| {
                body.child(surface::failed("Partial RBAC data", warnings))
            })
            .child(self.rbac_toolbar(cx))
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .min_h(px(612.))
                    .child(self.rbac_queue(cx))
                    .child(inspector),
            )
    }

    fn rbac_posture(&self, complete: usize, total: usize, cx: &mut Context<Self>) -> Div {
        let data = self.rbac.data().expect("ready");
        let objects = data
            .service_accounts
            .iter()
            .map(|item| &item.risks)
            .chain(data.roles.iter().map(|item| &item.risks))
            .chain(data.cluster_roles.iter().map(|item| &item.risks))
            .chain(rbac::bindings(data).map(|item| &item.risks));
        let (mut high, mut unknown) = (0, 0);
        for risks in objects {
            high += usize::from(has_level(risks, RbacRiskLevel::High));
            unknown += usize::from(has_level(risks, RbacRiskLevel::Unknown));
        }
        let metrics = [
            ("Identities", data.service_accounts.len(), None),
            ("Roles", data.roles.len() + data.cluster_roles.len(), None),
            (
                "Bindings",
                data.role_bindings.len() + data.cluster_role_bindings.len(),
                None,
            ),
            ("High", high, (high > 0).then(Tokens::destructive)),
            ("Unknown", unknown, (unknown > 0).then(Palette::amber300)),
        ];
        let families = if total == 0 { 5 } else { total };
        // `grid-cols-[minmax(14rem,1.1fr)_minmax(30rem,2.4fr)_auto]` inside the
        // `size="sm"` card's `py-3` and its content's `px-3`.
        ui::card().py(px(12.)).child(
            div()
                .flex()
                .items_stretch()
                .gap(px(12.))
                .px(px(12.))
                .child(
                    div()
                        .flex_basis(px(0.))
                        .flex_grow(1.1)
                        .min_w(px(224.))
                        .px(px(12.))
                        .py(px(10.))
                        .child(ui::semibold(ui::sm("Access posture")))
                        .child(
                            ui::muted(ui::xs(format!(
                                "{} · {complete}/{families} RBAC families · {}",
                                data.cluster,
                                freshness(&data.refreshed_at)
                            )))
                            .mt(px(2.)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_basis(px(0.))
                        .flex_grow(2.4)
                        .min_w(px(480.))
                        .border_x_1()
                        .border_color(Tokens::border())
                        .children(metrics.into_iter().enumerate().map(
                            |(index, (label, value, tone))| {
                                div()
                                    .flex_1()
                                    .px(px(12.))
                                    .py(px(8.))
                                    .when(index < 4, |cell| {
                                        cell.border_r_1().border_color(Tokens::border())
                                    })
                                    .child(
                                        ui::caps(label, 10.)
                                            .line_height(px(16.25))
                                            .font_weight(FontWeight::MEDIUM),
                                    )
                                    .child(
                                        ui::semibold(ui::text(value.to_string(), 16., 24.))
                                            .mt(px(2.))
                                            .when_some(tone, |text, tone| text.text_color(tone)),
                                    )
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_end()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(10.))
                        .child(
                            button(Variant::Outline, 24., false)
                                .id("rbac-refresh")
                                .test_support()
                                .child(ui::icon("refresh-cw", 12., Tokens::foreground()))
                                .child("Refresh")
                                .on_click(cx.listener(|this, _, _, cx| this.refresh_rbac(cx))),
                        )
                        .child(
                            button(Variant::Default, 24., false)
                                .id("rbac-investigate")
                                .test_support()
                                .child(ui::icon("shield-check", 12., Tokens::primary_foreground()))
                                .child("Investigate identity")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.begin_identity(IdentityKind::User, window, cx)
                                })),
                        ),
                ),
        )
    }
}

/// `freshnessLabel`, with the web's `toLocaleString` shape for older reads.
fn freshness(value: &str) -> String {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(value) else {
        return "freshness unavailable".into();
    };
    let age = (chrono::Utc::now() - at.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0);
    if age < 60 {
        format!("refreshed {age}s ago")
    } else if age < 3600 {
        format!("refreshed {}m ago", age / 60)
    } else {
        format!(
            "refreshed {}",
            at.with_timezone(&chrono::Local)
                .format("%d/%m/%Y, %H:%M:%S")
        )
    }
}
