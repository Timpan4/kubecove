//! Helm page, mirroring `HelmSurface.svelte` and `HelmView.svelte` inside
//! `SurfaceFrame`. Like the GitOps page it is one virtualized list: the frame
//! header, the summary, the namespace rail with the table header and first
//! rows, the remaining card or list rows, then the release details card.

use super::delivery_summary as summary_ui;
use super::delivery_ui::{self as dui, Layout, ViewMode};
use super::helm_card;
use super::helm_details::HelmDetails;
use super::loading::Phase;
use super::{Inspector, friendly_error::Operation, request::Query, surface};
use crate::helm;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, AppContext, Context, Div, Entity, ListAlignment, ListState, ScrollHandle,
    Subscription, Window, div, list, prelude::*, px,
};
use kubecove_lib::models::{HelmReleaseList, HelmReleaseSummary};

mod rail;

/// List rows the table puts in the top item beside the rail.
const TOP_ROWS: [usize; 2] = [1, 4];

/// A release opened from another page, such as a resource's Open Helm release
/// action. The page selects it after the next successful list read.
pub(super) struct HelmTarget {
    pub(super) name: String,
    pub(super) namespace: Option<String>,
}

impl HelmTarget {
    /// `findHelmReleaseTarget`: a release with this name, in the namespace when one is given.
    fn matches(&self, release: &HelmReleaseSummary) -> bool {
        release.name == self.name
            && self
                .namespace
                .as_ref()
                .is_none_or(|namespace| &release.namespace == namespace)
    }
}

pub(super) struct HelmPage {
    /// The context `load` belongs to.
    context: String,
    /// Set by `open_helm_release`; consumed by the next successful list read.
    pub(super) target: Option<HelmTarget>,
    load: Query<HelmReleaseList>,
    /// `helmNamespace`; `None` is All releases.
    namespace: Option<String>,
    search: Entity<InputState>,
    /// `buildHelmReleaseState(...).filtered`, as indices into the releases.
    filtered: Vec<usize>,
    namespace_counts: Vec<(String, usize)>,
    pub(super) mode: ViewMode,
    pub(super) selected: Option<usize>,
    pub(super) layout: Layout,
    pub(super) x_scroll: ScrollHandle,
    /// The details card's viewport `lg` two-column grid.
    pub(super) details_columns: bool,
    list: ListState,
    shape: Option<(usize, ViewMode, Layout, bool)>,
    /// The release list's loading phase. List items render after the frame,
    /// so `helm_page_view` observes it during render.
    status: Phase,
    pub(super) details: HelmDetails,
    _search_changed: Subscription,
}

impl HelmPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search releases..."));
        let _search_changed = cx.subscribe_in(&search, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.filter_helm(cx);
            }
        });
        Self {
            context: String::new(),
            target: None,
            load: Query::default(),
            namespace: None,
            search,
            filtered: Vec::new(),
            namespace_counts: Vec::new(),
            mode: ViewMode::Cards,
            selected: None,
            layout: Layout::new(0., 0),
            x_scroll: ScrollHandle::new(),
            details_columns: false,
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            shape: None,
            status: Phase::Ready,
            details: HelmDetails::default(),
            _search_changed,
        }
    }

    pub(super) fn releases(&self) -> &[HelmReleaseSummary] {
        match self.load.data() {
            Some(list) => &list.releases,
            None => &[],
        }
    }

    fn row_count(&self) -> usize {
        match self.mode {
            ViewMode::Cards => self.filtered.len().div_ceil(self.layout.columns),
            ViewMode::List => self.filtered.len(),
        }
    }

    fn top_rows(&self) -> usize {
        if !self.layout.rail_beside {
            return 0;
        }
        TOP_ROWS[usize::from(self.mode == ViewMode::List)].min(self.row_count())
    }

    /// Header, summary, top, rows, then the details card when one is selected.
    fn items(&self) -> usize {
        match self.load.data() {
            Some(_) if self.status.ready() => {
                3 + self.row_count() - self.top_rows() + usize::from(self.selected.is_some())
            }
            _ => 2,
        }
    }
}

enum Item {
    Header,
    Status,
    Summary,
    Top,
    Rows(usize),
    Details,
}

impl Inspector {
    /// `helmQuery`: read on every page load, keeping the shown list while a
    /// read for the same context runs.
    pub(super) fn load_helm(&mut self, cx: &mut Context<Self>) {
        if self.scope.context.is_empty() {
            return;
        }
        let page = &mut self.helm_page;
        if page.context != self.scope.context {
            page.context = self.scope.context.clone();
            page.load = Query::default();
            page.namespace = None;
            page.selected = None;
            page.details = HelmDetails::default();
            page.filtered.clear();
        }
        page.load.clear_refresh_error();
        let backend = self.backend.clone();
        let context = page.context.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.helm_releases(context.clone())
        });
        let read = self.timings.measure("helm_releases", read);
        page.load.start(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let page = &mut this.helm_page;
                let previous = page
                    .selected
                    .and_then(|index| page.releases().get(index).cloned());
                let success = result.is_ok();
                page.load.settle(result);
                page.namespace_counts = helm::groups(page.releases())
                    .into_iter()
                    .map(|group| (group.namespace.to_owned(), group.count))
                    .collect();
                let namespace = page.namespace.take();
                // A pending target applies after a successful read and stays pending after a failed one.
                let target = if success { page.target.take() } else { None };
                let namespace = target
                    .as_ref()
                    .and_then(|target| target.namespace.clone())
                    .or(namespace);
                page.namespace = helm::resolve_namespace(page.releases(), namespace);
                let target_index = target.as_ref().and_then(|target| {
                    page.releases()
                        .iter()
                        .position(|release| target.matches(release))
                });
                // `selectedHelmReleaseExists`: a vanished release closes its details.
                page.selected = target_index.or_else(|| {
                    previous.and_then(|previous| {
                        page.releases()
                            .iter()
                            .position(|release| helm::same_release(release, &previous))
                    })
                });
                if target_index.is_some() {
                    page.details = HelmDetails::default();
                }
                if page.selected.is_none() {
                    page.details = HelmDetails::default();
                }
                this.filter_helm(cx);
                if success {
                    this.page_read_finished(super::Page::Helm, cx);
                } else {
                    this.invalidate_page_read(super::Page::Helm);
                }
                this.refresh_helm_details(cx);
            });
        }));
    }

    pub(super) fn cancel_helm_reads(&mut self) {
        self.helm_page.load.cancel();
        self.helm_page.details.cancel();
    }

    fn filter_helm(&mut self, cx: &mut Context<Self>) {
        let search = self.helm_page.search.read(cx).value().to_string();
        let page = &mut self.helm_page;
        page.filtered = helm::filter(page.releases(), &search, page.namespace.as_deref());
        cx.notify();
    }

    /// `selectHelmNamespace`: a namespace filter drops a selection outside it.
    fn select_helm_namespace(&mut self, namespace: Option<String>, cx: &mut Context<Self>) {
        let page = &mut self.helm_page;
        if let (Some(namespace), Some(index)) = (&namespace, page.selected)
            && page
                .releases()
                .get(index)
                .is_some_and(|release| &release.namespace != namespace)
        {
            page.selected = None;
            page.details = HelmDetails::default();
        }
        page.namespace = namespace;
        self.filter_helm(cx);
    }

    /// `onOpenResources(namespace, name)`: the namespace's resources, searched by release.
    pub(super) fn open_helm_resources(
        &mut self,
        namespace: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.search
            .update(cx, |search, cx| search.set_value(name, window, cx));
        self.select_namespace(Some(namespace), window, cx);
    }

    fn helm_item(&self, index: usize) -> Item {
        let page = &self.helm_page;
        let rows = page.row_count() - page.top_rows();
        match index {
            0 => Item::Header,
            1 if page.items() == 2 => Item::Status,
            1 => Item::Summary,
            2 => Item::Top,
            index if index - 3 < rows => Item::Rows(index - 3 + page.top_rows()),
            _ => Item::Details,
        }
    }

    pub(super) fn helm_page_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let frame = self.frame_width(window);
        let status = self
            .loading
            .phase("surface-helm", self.helm_page.load.pending());
        self.observe_helm_details_loading();
        let page = &mut self.helm_page;
        page.status = status;
        page.layout = Layout::new(frame, page.filtered.len());
        page.details_columns = window.viewport_size().width >= px(1024.);
        let shape = Some((
            page.items(),
            page.mode,
            page.layout,
            page.selected.is_some(),
        ));
        if page.shape != shape {
            page.shape = shape;
            page.list.reset(page.items());
        }
        div()
            .id("helm-list")
            .test_support()
            .size_full()
            .child(
                list(
                    page.list.clone(),
                    cx.processor(|this: &mut Inspector, index: usize, _, cx| {
                        this.helm_list_item(index, cx)
                    }),
                )
                .size_full(),
            )
            .into_any_element()
    }

    fn helm_list_item(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let last = index + 1 == self.helm_page.items();
        let item = match self.helm_item(index) {
            Item::Header => dui::frame_header("package", "Helm", Vec::new()).pt(px(20.)),
            Item::Status => div().pt(px(16.)).child(
                match (
                    surface::pending_slot(self.helm_page.status),
                    self.helm_page.load.failure(),
                ) {
                    (Some(slot), _) => slot,
                    (None, Some(error)) => self.page_read_failure(
                        "Helm releases unavailable",
                        error,
                        Operation::Surface,
                        cx,
                    ),
                    (None, _) => div().into_any_element(),
                },
            ),
            Item::Summary => self.helm_summary(cx).pt(px(16.)),
            Item::Top => self.helm_top(cx).pt(px(16.)),
            Item::Rows(row) => self.helm_row(row, cx),
            Item::Details => self.helm_details_card(cx).pt(px(16.)),
        };
        item.w_full()
            .px(px(20.))
            .when(last, |item| item.pb(px(20.)))
            .into_any_element()
    }

    fn helm_summary(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let Some(data) = page.load.data() else {
            return div();
        };
        let counts = helm::counts(&data.releases);
        let facts = [
            ("Releases", data.releases.len(), Tokens::foreground()),
            (
                "Namespaces",
                helm::groups(&data.releases).len(),
                Tokens::foreground(),
            ),
            ("Deployed", counts.deployed, Palette::emerald300()),
            ("Pending", counts.pending, Palette::amber300()),
            ("Failed", counts.failed, Tokens::destructive()),
        ]
        .map(|(label, value, color)| summary_ui::Fact {
            label,
            value,
            color,
        });
        let badges = summary_ui::Badges::Line(
            (!data.releases.is_empty()).then(|| "Helm storage detected".to_owned()),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(summary_ui::summary(
                "helm",
                "Helm",
                badges,
                (data.releases.len(), "Helm releases"),
                &facts,
                page.layout.summary_columns,
            ))
            .when_some(page.load.refresh_error().cloned(), |column, error| {
                column.child(self.page_read_failure(
                    "Helm refresh failed",
                    &error,
                    Operation::Surface,
                    cx,
                ))
            })
            .when(!data.warnings.is_empty(), |column| {
                column.child(helm_card::alert(
                    "Some Helm releases may be missing",
                    &data.warnings,
                ))
            })
    }

    /// The table header: title, search, Clear, item count and view toggle.
    fn helm_table_header(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let count = page.filtered.len();
        let count = format!("{count} item{}", if count == 1 { "" } else { "s" });
        let searching = !page.search.read(cx).value().is_empty();
        // `@min-[48rem]:max-w-sm` on the search box.
        let narrow = page.layout.grid >= 768.;
        div()
            .flex()
            .flex_wrap()
            .items_end()
            .justify_between()
            .gap(px(12.))
            .pb(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .child(ui::semibold(ui::text("Helm Releases", 16., 24.)))
                    .child(ui::muted(ui::sm(
                        "Chart, namespace, revision, and storage source.",
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(224.))
                            .when(narrow, |search| search.max_w(px(384.)))
                            .child(
                                Input::new(&page.search)
                                    .id("helm-search")
                                    .aria_label("Search Helm releases")
                                    .prefix(ui::icon("search", 16., Tokens::muted_foreground()))
                                    .bg(Tokens::input().opacity(0.3))
                                    .text_xs()
                                    .w_full(),
                            ),
                    )
                    .when(searching, |row| {
                        row.child(
                            surface::small_button(Some("x"), "Clear")
                                .id("helm-clear")
                                .test_support()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.clear_helm_search(window, cx)
                                })),
                        )
                    })
                    .child(
                        ui::muted(ui::xs(count.clone()))
                            .id("helm-count")
                            .test_support()
                            .aria_label(count),
                    )
                    .child(dui::view_toggle(
                        "helm-view",
                        page.mode,
                        |this, mode| this.helm_page.mode = mode,
                        cx,
                    )),
            )
    }

    fn helm_top(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let mut section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(16.))
            .child(self.helm_table_header(cx));
        if page.filtered.is_empty() {
            section = section.child(helm_card::empty(page.releases().is_empty(), cx));
        } else if page.top_rows() > 0 || page.mode == ViewMode::List {
            let rows = (0..page.top_rows())
                .map(|row| self.helm_row_body(row, cx).into_any_element())
                .collect();
            let closes = page.top_rows() == page.row_count();
            let header = helm_card::list_header(page.layout.grid - 2., &page.x_scroll);
            section = section.child(dui::rows_block(page.mode, header, rows, closes));
        }
        let rail = self.helm_rail(cx);
        if page.layout.rail_beside {
            div()
                .flex()
                .gap(px(16.))
                .child(rail.self_stretch())
                .child(section)
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(rail)
                .child(section)
        }
    }

    fn helm_row(&self, row: usize, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let last = row + 1 == page.row_count();
        let gap = if row == 0 { 16. } else { dui::CARD_GAP };
        let body = dui::continuation(page.mode, self.helm_row_body(row, cx), last, gap);
        if page.layout.rail_beside {
            div()
                .flex()
                .gap(px(16.))
                .child(dui::rail_spacer().self_stretch())
                .child(body)
        } else {
            div().child(body)
        }
    }

    fn helm_row_body(&self, row: usize, cx: &mut Context<Self>) -> Div {
        let page = &self.helm_page;
        let releases = page.releases();
        match page.mode {
            ViewMode::Cards => {
                let start = row * page.layout.columns;
                let end = (start + page.layout.columns).min(page.filtered.len());
                div()
                    .flex()
                    .gap(px(dui::CARD_GAP))
                    .children(page.filtered[start..end].iter().map(|index| {
                        helm_card::card(
                            *index,
                            &releases[*index],
                            page.selected == Some(*index),
                            page.layout.card,
                            cx,
                        )
                    }))
            }
            ViewMode::List => div().children(page.filtered.get(row).map(|index| {
                helm_card::list_row(
                    *index,
                    &releases[*index],
                    page.selected == Some(*index),
                    page.layout.grid,
                    &page.x_scroll,
                    cx,
                )
            })),
        }
    }
}
