//! Settings, mirroring `SettingsSurface.svelte` and `SettingsRow.svelte`. Rows
//! the native host cannot back render disabled with the reason as their
//! tooltip. The Kubeconfig category lives in `settings_kubeconfig.rs`.

use super::controls::{Variant, button};
use super::{Inspector, Page};
use crate::backend::KubeconfigSourcesSummary;
use crate::settings::{self, Category, Control, Row};
use crate::targets::TestSupportExt;
use crate::theme::{HEADING, Tokens};
use crate::ui;
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FontWeight, SharedString, Stateful, Subscription, Task,
    Window, div, prelude::*, px,
};

pub(super) type Observed = crate::targets::ObservedElement<Stateful<Div>>;

pub(super) struct SettingsState {
    pub(super) kind_focus: std::collections::HashMap<String, gpui_kit::FocusHandle>,
    pub(super) diagnostics: crate::backend::adapters::BackendDiagnostics,
    pub(super) diagnostics_error: Option<String>,
    /// The last latency-report action's result, shown under the report.
    pub(super) diagnostics_status: Option<String>,
    /// Whether Copy includes identifiers. Redacted by default, as on the web.
    pub(super) latency_identifiers: bool,
    pub(super) diagnostics_task: Option<Task<()>>,
    pub(super) confirming_skip_review: bool,
    pub(super) integrations: super::settings_integrations::Integrations,
    pub(super) mcp: super::settings_mcp::McpState,
    category: Category,
    search: Entity<InputState>,
    pub(super) env_var: Entity<InputState>,
    pub(super) pasted_paths: Entity<TextareaState>,
    /// The single-line server path field that remote mode uses instead of `pasted_paths`.
    pub(super) server_path: Entity<InputState>,
    pub(super) sources: Option<Result<KubeconfigSourcesSummary, String>>,
    pub(super) busy: bool,
    pub(super) task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings..."));
        let env_var = cx.new(|cx| InputState::new(window, cx).placeholder("KUBECONFIG"));
        let pasted_paths = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("/path/to/kubeconfig.yaml")
                .rows(2)
        });
        let server_path =
            cx.new(|cx| InputState::new(window, cx).placeholder("/path/on/server/kubeconfig.yaml"));
        let subscriptions = vec![cx.subscribe_in(&search, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self {
            kind_focus: Default::default(),
            diagnostics: Default::default(),
            diagnostics_error: None,
            diagnostics_status: None,
            latency_identifiers: false,
            diagnostics_task: None,
            confirming_skip_review: false,
            integrations: super::settings_integrations::Integrations::new(window, cx),
            mcp: super::settings_mcp::McpState::new(window, cx),
            category: Category::General,
            search,
            env_var,
            pasted_paths,
            server_path,
            sources: None,
            busy: false,
            task: None,
            _subscriptions: subscriptions,
        }
    }
}

impl Inspector {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.category = Category::General;
        self.settings
            .search
            .update(cx, |input, cx| input.set_value("", window, cx));
        if self.settings.sources.as_ref().is_some_and(Result::is_err) {
            self.settings.sources = None;
        }
        let read = self.kubeconfig_sources_read(cx);
        let read = self.timings.measure("kubeconfig_sources", read);
        self.run_kubeconfig(read, window, cx);
        self.initialize_settings_integrations(window, cx);
        self.initialize_mcp(window, cx);
        self.open_page(Page::Settings, cx);
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        // Without an open workspace there is no Workspace Overview, so Back returns to the
        // launcher, as the web's settings view does.
        if !self.workspace_open() {
            self.launcher.open = true;
            cx.notify();
            return;
        }
        // The web's closeSettings lands on the Workspace Overview, whatever page opened Settings.
        self.open_page(Page::Overview, cx);
    }

    /// The remote this client is connected to. Its server backs Server information.
    fn connected_remote(&self) -> Option<&crate::backend::RemoteConnectionSummary> {
        self.settings
            .integrations
            .remote
            .as_ref()
            .filter(|remote| remote.connected)
    }

    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings;
        let query = state.search.read(cx).value().trim().to_owned();
        let searching = !query.is_empty();
        let heading = if searching {
            "Search results"
        } else {
            state.category.label()
        };
        let server = self.connected_remote().is_some();
        let mut sections = div().flex().flex_col().gap(px(16.));
        if searching && !settings::has_matches(&query, server) {
            let empty = format!("No settings match \"{query}\".");
            sections = sections.child(
                ui::muted(ui::sm(empty.clone()))
                    .id("settings-empty")
                    .test_support()
                    .aria_label(empty)
                    .p(px(16.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::card()),
            );
        }
        for (category, ..) in Category::ALL {
            if settings::shown(&query, state.category, category, server) {
                sections = sections.child(self.settings_category(category, cx));
            }
        }
        div()
            .flex()
            .items_start()
            .gap(px(20.))
            .w_full()
            .max_w(px(1280.))
            .mx_auto()
            .p(px(24.))
            .child(self.settings_nav(searching, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(1024.))
                    .gap(px(16.))
                    .child(
                        ui::semibold(ui::text(heading, 20., 28.))
                            .id("settings-heading")
                            .test_support()
                            .aria_label(heading)
                            .font_family(HEADING),
                    )
                    .child(sections),
            )
    }

    fn settings_nav(&self, searching: bool, cx: &mut Context<Self>) -> Div {
        let state = &self.settings;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(240.))
            .gap(px(12.))
            .child(
                div()
                    .id("settings-back")
                    .test_support()
                    .aria_label("Back to app")
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(ui::muted(ui::sm("Back to app")).font_weight(FontWeight::MEDIUM))
                    .on_click(cx.listener(|this, _, _, cx| this.close_settings(cx))),
            )
            .child(
                Input::new(&state.search)
                    .id("settings-search")
                    .aria_label("Search settings")
                    .prefix(ui::icon("search", 16., Tokens::muted_foreground()))
                    // `text-sm` loses to the input's `md:text-xs` at this width.
                    .text_xs()
                    .h_9(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(Category::ALL.into_iter().map(|(category, label, icon)| {
                        let active = category == state.category && !searching;
                        button(
                            if active {
                                Variant::Secondary
                            } else {
                                Variant::Ghost
                            },
                            32.,
                            false,
                        )
                        .id(SharedString::from(format!("settings-section:{label}")))
                        .test_support()
                        .aria_label(label)
                        .aria_selected(active)
                        .justify_start()
                        .gap(px(10.))
                        // `px-3` with the inline-start icon's `pl-1.5` and `size-3.5`.
                        .pl(px(6.))
                        .pr(px(12.))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .child(ui::icon(icon, 14., Tokens::foreground()))
                        .child(label)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.settings.category = category;
                                this.settings
                                    .search
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                                cx.notify();
                            },
                        ))
                    })),
            )
    }

    fn settings_category(&self, category: Category, cx: &mut Context<Self>) -> AnyElement {
        if category == Category::Agents {
            return self.mcp_settings_section(cx).into_any_element();
        }
        if category == Category::Remote {
            return self.remote_settings_section(cx).into_any_element();
        }
        if category == Category::Updates {
            return self.updates_settings_section(cx).into_any_element();
        }
        if category == Category::Kubeconfig {
            return self.kubeconfig_section(cx).into_any_element();
        }
        let remote = self.connected_remote();
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(settings::rows(category, remote.is_some()).map(|row| {
                if row.title == "KubeCove documentation" {
                    self.documentation_control(row, cx)
                } else if let Some(remote) =
                    remote.filter(|_| row.title == settings::SERVER_INFORMATION)
                {
                    super::settings_sections::server_information(row, remote)
                } else if row.title == "Latency report" {
                    self.latency_report(row, cx)
                } else if row.title == "Record interactions" {
                    self.interaction_switch(row, cx)
                } else if row.title == "Interaction report" {
                    self.interaction_report(row, cx)
                } else if row.title == "Topology spike" {
                    self.topology_spike_control(row, cx)
                } else if row.unavailable.is_none() && row.control != Control::Section {
                    self.preference_control(row, cx)
                } else if row.title == "Resource group order" {
                    self.resource_order_controls(row, cx)
                } else if row.control == Control::Section {
                    super::settings_sections::section(row)
                } else {
                    setting_row(row, control(row)).into_any_element()
                }
            }))
            .into_any_element()
    }
}

/// `SettingsRow.svelte`: `rounded-md border bg-card p-3`, title and description
/// on the left, the control on the right.
pub(super) fn settings_row(title: &str, description: String, control: AnyElement) -> Observed {
    div()
        .id(SharedString::from(format!("setting-row:{title}")))
        .test_support()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(ui::medium(ui::sm(title.to_owned())).line_height(px(20.)))
                .child(ui::muted(ui::xs(description)).line_height(px(18.))),
        )
        .child(div().flex().flex_none().child(control))
}

fn setting_row(row: &'static Row, control: AnyElement) -> Observed {
    settings_row(row.title, row.description.to_owned(), control)
}

/// `Switch`: `h-6 w-10 rounded-full border p-0.5` with a `size-4` thumb.
pub(super) fn switch(label: &'static str, checked: bool) -> Observed {
    div()
        .id(SharedString::from(format!("setting:{label}")))
        .test_support()
        .aria_label(label)
        .aria_toggled(if checked {
            gpui_kit::accesskit::Toggled::True
        } else {
            gpui_kit::accesskit::Toggled::False
        })
        .flex()
        .flex_none()
        .items_center()
        .w(px(40.))
        .h(px(24.))
        .p(px(2.))
        .rounded_full()
        .border_1()
        .border_color(if checked {
            Tokens::primary()
        } else {
            Tokens::input()
        })
        .bg(if checked {
            Tokens::primary()
        } else {
            Tokens::input().opacity(0.4)
        })
        .cursor_pointer()
        .child(
            div()
                .size(px(16.))
                .rounded_full()
                .bg(Tokens::background())
                .when(checked, |thumb| thumb.ml(px(16.))),
        )
}

/// A control the native host cannot back: `disabled:opacity-50` with the reason.
pub(super) fn unavailable_control(
    id: impl Into<gpui_kit::ElementId>,
    reason: &'static str,
    control: impl IntoElement,
) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .aria_description(reason)
        .opacity(0.5)
        .tooltip(move |window, cx| Tooltip::new(reason).build(window, cx))
        .child(control)
        .into_any_element()
}

/// The row's widget in its default state.
fn control(row: &'static Row) -> AnyElement {
    let reason = row
        .unavailable
        .unwrap_or("Not available in the native client yet.");
    let widget = match row.control {
        Control::Switch(checked) => switch(row.title, checked).into_any_element(),
        Control::Segments(options, active) => div()
            .flex()
            .items_center()
            .p(px(2.))
            .rounded(px(10.))
            .bg(Tokens::background())
            .border_1()
            .border_color(Tokens::border().opacity(0.6))
            .children(options.iter().enumerate().map(|(index, option)| {
                let selected = index == active;
                div()
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .when(selected, |segment| segment.bg(Tokens::card()))
                    .child(ui::medium(ui::xs(*option)).text_color(if selected {
                        Tokens::foreground()
                    } else {
                        Tokens::muted_foreground()
                    }))
            }))
            .into_any_element(),
        Control::Button(label) => button(Variant::Outline, 24., false)
            .child(label)
            .into_any_element(),
        Control::Section => return div().into_any_element(),
    };
    unavailable_control(
        SharedString::from(format!("setting-control:{}", row.title)),
        reason,
        widget,
    )
}
