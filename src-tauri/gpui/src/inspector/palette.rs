//! Command palette, mirroring `CommandPalette.svelte` inside the shadcn
//! `CommandDialog` (`top-[20%] max-w-lg rounded-lg border bg-surface-2`).
//! Search reads use the shared backend and are cancelled when the palette closes.

#[path = "palette_search.rs"]
mod search;

use super::{Inspector, Page};
use crate::backend::CLUSTER_KINDS;
use crate::palette::{self, Destination, NavEntry};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, KeyDownEvent, SharedString, Subscription, Window, div,
    prelude::*, px,
};
use kubecove_lib::models::ResourceSummary;

/// `SEARCH_SHORTCUT_HINT` in `WorkspaceHeader.svelte`.
pub(super) const SHORTCUT_HINT: &str = if cfg!(target_os = "macos") {
    "⌘K"
} else {
    "Ctrl K"
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Resource(usize),
    Namespace(usize),
    Nav(usize),
}

impl Hit {
    /// The `CommandGroup` the hit renders in.
    fn group(self) -> usize {
        match self {
            Hit::Resource(_) => 0,
            Hit::Namespace(_) => 1,
            Hit::Nav(_) => 2,
        }
    }
}

pub(super) struct PaletteState {
    pub(super) open: bool,
    pub(super) kind: Option<&'static str>,
    query: Entity<InputState>,
    nav: Vec<NavEntry>,
    resources: Vec<ResourceSummary>,
    /// Search text per resource, built once when the palette opens.
    index: Vec<String>,
    namespaces: Vec<String>,
    hits: Vec<Hit>,
    namespace_hits: Vec<String>,
    /// The keyboard-highlighted hit; cmdk's default selection draws no highlight.
    active: Option<usize>,
    search: search::SearchRead,
    _subscriptions: [Subscription; 2],
}

impl PaletteState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search views, namespaces, and resources...")
        });
        let change = cx.subscribe_in(&query, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_palette(cx);
            }
        });
        // `shouldToggleCommandPaletteShortcut`: the window-level capture listener
        // runs before any focused control handles the key. `secondary` is Cmd on
        // macOS and Ctrl elsewhere.
        let owner = cx.entity().downgrade();
        let handle = window.window_handle();
        let shortcut = cx.intercept_keystrokes(move |event, window, cx| {
            let keystroke = &event.keystroke;
            let modifiers = keystroke.modifiers;
            if window.window_handle() != handle
                || !keystroke.key.eq_ignore_ascii_case("k")
                || !modifiers.secondary(cx)
                || modifiers.alt
                || modifiers.shift
            {
                return;
            }
            cx.stop_propagation();
            let _ = owner.update(cx, |this, cx| this.toggle_palette(window, cx));
        });
        Self {
            open: false,
            kind: None,
            query,
            nav: palette::navigation(true),
            resources: vec![],
            index: vec![],
            namespaces: vec![],
            hits: vec![],
            namespace_hits: vec![],
            active: None,
            search: search::SearchRead::default(),
            _subscriptions: [change, shortcut],
        }
    }
}

impl Inspector {
    pub(super) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.open {
            self.close_palette(window, cx);
            return;
        }
        let launcher = &self.launcher;
        let mut resources: Vec<_> = self
            .rows
            .iter()
            .chain(self.cluster_rows())
            .filter(|row| launcher.in_scope(row))
            .cloned()
            .collect();
        // The web warms its index in workspace kind order, preserving the
        // API's order within each kind.
        resources.sort_by_key(|row| {
            crate::model::WORKSPACE_KINDS
                .iter()
                .position(|kind| *kind == row.kind)
        });
        let state = &mut self.palette;
        state.open = true;
        state.resources = resources;
        state.index = state.resources.iter().map(palette::search_text).collect();
        state.namespaces = self
            .namespaces
            .iter()
            .filter(|ns| launcher.shows_namespace(&ns.name))
            .map(|ns| ns.name.clone())
            .collect();
        let query = state.query.clone();
        query.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.refresh_palette(cx);
        self.warm_palette(cx);
    }

    pub(super) fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.palette;
        state.open = false;
        state.search = search::SearchRead::default();
        // The rows snapshot is only needed while the palette is open.
        state.resources = vec![];
        state.index = vec![];
        state.hits.clear();
        let query = state.query.clone();
        query.update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn refresh_palette(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.palette;
        let query = state.query.read(cx).value();
        state.namespace_hits = palette::filter_namespaces(&state.namespaces, &query)
            .into_iter()
            .map(str::to_owned)
            .collect();
        state.hits = palette::filter_resources(&state.index, &query)
            .into_iter()
            .map(Hit::Resource)
            .chain((0..state.namespace_hits.len()).map(Hit::Namespace))
            .chain(
                palette::filter_navigation(&state.nav, &query)
                    .into_iter()
                    .map(Hit::Nav),
            )
            .collect();
        state.active = None;
        cx.notify();
    }

    fn choose(&mut self, hit: Hit, window: &mut Window, cx: &mut Context<Self>) {
        let state = &self.palette;
        let chosen = match hit {
            Hit::Resource(index) => Chosen::Resource(Box::new(state.resources[index].clone())),
            Hit::Namespace(index) => Chosen::Namespace(state.namespace_hits[index].clone()),
            Hit::Nav(index) => Chosen::Nav(state.nav[index].destination),
        };
        self.close_palette(window, cx);
        match chosen {
            Chosen::Resource(resource) => {
                if CLUSTER_KINDS.contains(&resource.kind.as_str()) {
                    self.open_cluster_overview(window, cx);
                } else {
                    self.select_namespace(None, window, cx);
                }
                self.open_resource(*resource, window, cx);
            }
            Chosen::Namespace(namespace) => self.select_namespace(Some(namespace), window, cx),
            Chosen::Nav(destination) => self.go_to(destination, window, cx),
        }
    }

    fn go_to(&mut self, destination: Destination, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.kind = None;
        match destination {
            Destination::WorkspaceOverview => self.open_page(Page::Overview, cx),
            Destination::ClusterOverview => self.open_cluster_overview(window, cx),
            Destination::Resources(kind) => {
                if kind.is_some_and(|kind| CLUSTER_KINDS.contains(&kind)) {
                    self.open_cluster_overview(window, cx);
                } else {
                    self.select_namespace(None, window, cx);
                }
                self.palette.kind = kind;
                self.search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.filter(cx);
            }
            Destination::Incidents => self.open_page(Page::Incidents, cx),
            Destination::PortForwards => self.open_page(Page::PortForwards, cx),
            Destination::Rbac(view) => self.open_rbac(view, window, cx),
            Destination::Settings => self.open_settings(window, cx),
            Destination::Launcher => self.open_launcher(window, cx),
            Destination::Unavailable => {
                self.notice = Some("This view is not available in the native client yet.".into());
                cx.notify();
            }
        }
    }

    fn palette_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.palette.hits.len();
        let active = self.palette.active;
        match event.keystroke.key.as_str() {
            "escape" => self.close_palette(window, cx),
            "down" if count > 0 => {
                self.palette.active = Some(active.map_or(0, |index| (index + 1) % count))
            }
            "up" if count > 0 => {
                self.palette.active =
                    Some(active.map_or(count - 1, |index| (index + count - 1) % count))
            }
            "enter" => {
                if let Some(hit) = self.palette.hits.get(active.unwrap_or(0)).copied() {
                    self.choose(hit, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// `CommandItem`: `flex items-center gap-2 rounded-sm text-xs`. The dialog's
    /// `[cmdk-item]` padding selectors match nothing in Bits UI, so rows are as
    /// tall as their 24px icon.
    fn palette_item(&self, hit: Hit, position: usize, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.palette;
        let (id, label, description, icon) = match hit {
            Hit::Resource(index) => {
                let row = &state.resources[index];
                let namespace = row.namespace.as_deref().unwrap_or("");
                (
                    format!("palette-resource:{}:{namespace}:{}", row.kind, row.name),
                    row.name.clone(),
                    Some(format!("{} {namespace}", row.kind)),
                    "box",
                )
            }
            Hit::Namespace(index) => {
                let namespace = &state.namespace_hits[index];
                (
                    format!("palette-namespace:{namespace}"),
                    namespace.clone(),
                    None,
                    "folder",
                )
            }
            Hit::Nav(index) => {
                let entry = &state.nav[index];
                (
                    format!("palette-entry:{}", entry.id),
                    entry.label.clone(),
                    None,
                    "panels-top-left",
                )
            }
        };
        let trailing = match hit {
            Hit::Resource(index) => {
                let row = &state.resources[index];
                Some(
                    div()
                        .ml_auto()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(8.))
                        .text_color(Tokens::muted_foreground())
                        .child(row.cluster.clone())
                        .children(row.namespace.clone())
                        .child(row.kind.clone()),
                )
            }
            _ => None,
        };
        div()
            .id(SharedString::from(id))
            .test_support()
            .role(gpui_kit::Role::ListBoxOption)
            .aria_label(label.clone())
            .aria_selected(state.active == Some(position))
            .when_some(description, |row, description| {
                row.aria_description(description)
            })
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .rounded(px(4.))
            .text_size(px(12.))
            .line_height(px(16.))
            .cursor_default()
            .when(state.active == Some(position), |row| {
                row.bg(Tokens::muted())
            })
            .child(ui::icon(icon, 24., Tokens::muted_foreground()).flex_none())
            .child(div().min_w_0().truncate().child(label))
            .children(trailing)
            .on_click(cx.listener(move |this, _, window, cx| this.choose(hit, window, cx)))
            .into_any_element()
    }

    pub(super) fn palette_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut groups: [Vec<AnyElement>; 3] = Default::default();
        for position in 0..self.palette.hits.len() {
            let hit = self.palette.hits[position];
            groups[hit.group()].push(self.palette_item(hit, position, cx));
        }
        let empty = self.palette.hits.is_empty();
        // `CommandGroup` is `overflow-hidden py-1`; its heading renders without
        // the dialog's heading styles and takes no visible space.
        let list = div()
            .id("palette-list")
            .max_h(px(320.))
            .overflow_y_scroll()
            .children(
                groups
                    .into_iter()
                    .filter(|items| !items.is_empty())
                    .map(|items| div().py(px(4.)).children(items)),
            )
            .when(empty && !self.palette.search.loading(), |list| {
                list.child(
                    ui::muted(ui::xs("No results found."))
                        .id("palette-empty")
                        .test_support()
                        .aria_label("No results found.")
                        .py(px(24.))
                        .text_center(),
                )
            })
            .when_some(self.palette.search.message(), |list, message| {
                list.child(ui::muted(ui::xs(message)).p(px(12.)))
            });
        div()
            .id("palette")
            .absolute()
            .inset_0()
            .bg(gpui_kit::black().opacity(0.8))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
            )
            .child(
                div()
                    .id("palette-dialog")
                    .test_support()
                    .role(gpui_kit::Role::Dialog)
                    .aria_label("Search")
                    .absolute()
                    .top(gpui_kit::relative(0.2))
                    .left_0()
                    .right_0()
                    .mx_auto()
                    .w(px(512.))
                    .overflow_hidden()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::card())
                    .shadow_2xl()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx: &mut App| {
                        cx.stop_propagation()
                    })
                    .capture_key_down(cx.listener(Self::palette_key))
                    .child(self.palette_input())
                    .child(list),
            )
            .into_any_element()
    }

    /// `CommandInput`: `flex h-10 items-center gap-2 border-b px-3`.
    fn palette_input(&self) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(40.))
            .px(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(ui::icon("search", 16., Tokens::muted_foreground()))
            .child(
                Input::new(&self.palette.query)
                    .id("palette-query")
                    .aria_label("Search views, namespaces, and resources")
                    .appearance(false)
                    .text_sm()
                    .flex_1(),
            )
    }
}

enum Chosen {
    Resource(Box<ResourceSummary>),
    Namespace(String),
    Nav(Destination),
}
