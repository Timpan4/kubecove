//! Ownership map panel, mirroring `OwnershipMap.svelte`,
//! `OwnershipResourceNode.svelte` and `StandaloneKindGroupNode.svelte` as
//! xyflow draws them. GPUI has no element transforms, so every flow length
//! is multiplied by the zoom when the node is built.

mod cards;
mod focus;
mod links;
mod paint;
pub(super) mod spike;
mod viewport;
use paint::PreparedEdges;

use super::Inspector;
use super::resources::Panel;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::topology::{self, Viewport};
use crate::ui;
use gpui_kit::{
    AnyElement, Context, Div, FocusHandle, KeyDownEvent, Pixels, Point, SharedString,
    WeakFocusHandle, div, prelude::*, px,
};
use kubecove_lib::models::{AppError, AppErrorKind, ResourceSummary, ResourceTopology};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;

pub(super) enum TopologyLoad {
    Loading,
    Ready(Arc<ResourceTopology>),
    Failed(AppError),
}

type CachedEdges = (Rc<topology::View>, Rc<PreparedEdges>);

pub(super) struct MapState {
    pub topology: TopologyLoad,
    node_ids: Vec<SharedString>,
    pub(super) expanded: HashSet<String>,
    pub network: bool,
    /// The workspace context chosen in the "Map context" select (`map_context.rs`).
    pub(super) context: Option<String>,
    pub(super) context_open: bool,
    pub cache: Option<Rc<topology::View>>,
    edge_cache: RefCell<Option<CachedEdges>>,
    unselected_edge_cache: RefCell<Option<CachedEdges>>,
    /// The graph viewport's size, measured during prepaint.
    size: Rc<Cell<(f32, f32)>>,
    /// The graph viewport's origin in window coordinates, measured with its size.
    origin: Rc<Cell<(f32, f32)>>,
    /// A pan or zoom, kept while the fitted viewport it started from is current.
    adjusted: Option<(Viewport, Viewport)>,
    drag: Option<(Point<Pixels>, Viewport)>,
    /// Set once a press on the map moves past `NODE_CLICK_DISTANCE`. The node under
    /// the release then gets no click, so a pan does not select a node.
    pan_moved: bool,
    /// Focus follows node identity when selection reorders the graph.
    resource_focus: Vec<Option<FocusHandle>>,
    group_focus: HashMap<SharedString, FocusHandle>,
    /// The window's focus at the last render. Focus moving onto a node pans
    /// that node into view.
    last_focus: Option<WeakFocusHandle>,
    /// The immutable view whose node focus handles are synchronized.
    focus_view: Weak<topology::View>,
    /// The zoom in, zoom out and fit controls, in Tab order.
    zoom_focus: Option<[FocusHandle; 3]>,
    /// The Ownership and Network Flow mode buttons, in Tab order.
    mode_focus: Option<[FocusHandle; 2]>,
    /// The "skip graph" link, a tab stop before the graph nodes.
    skip_link: Option<FocusHandle>,
    /// Where "skip graph" moves focus: the element right after the graph.
    pub skip_target: Option<FocusHandle>,
    /// The last built view and the inputs it was built from. Pan, zoom and
    /// hover redraw the map without changing them.
    built: RefCell<Option<(ViewInputs, Rc<topology::View>)>>,
    unselected_built: RefCell<Option<(ViewInputs, Rc<topology::View>)>>,
    prepared: RefCell<Option<PreparedMap>>,
    /// Set when the kind selection may have changed, so the next rebuild
    /// recomputes which nodes join as links.
    pub(super) links_stale: bool,
}

struct PreparedMap {
    rows: u64,
    /// The "Hide historical ReplicaSets" toggle the layout was filtered with.
    hide_historical: bool,
    links: Option<Vec<bool>>,
    nodes: (usize, usize),
    edges: (usize, usize),
    request: Rc<()>,
    source: topology::Source,
    data: Rc<topology::Prepared>,
}

impl MapState {
    pub(super) fn replace_topology(&mut self, topology: TopologyLoad) {
        let node_ids: Vec<SharedString> = match &topology {
            TopologyLoad::Ready(topology) => topology
                .nodes
                .iter()
                .map(|node| SharedString::from(format!("map-node:{}", node.id)))
                .collect(),
            _ => Vec::new(),
        };
        let previous: HashMap<_, _> = self
            .node_ids
            .iter()
            .zip(&mut self.resource_focus)
            .filter_map(|(id, handle)| handle.take().map(|handle| (id.clone(), handle)))
            .collect();
        self.resource_focus = node_ids
            .iter()
            .map(|id| previous.get(id).cloned())
            .collect();
        self.node_ids = node_ids;
        self.topology = topology;
        self.prepared.take();
        self.built.take();
        self.unselected_built.take();
        self.edge_cache.take();
        self.unselected_edge_cache.take();
    }
    pub fn new() -> Self {
        Self {
            topology: TopologyLoad::Loading,
            node_ids: Vec::new(),
            expanded: HashSet::new(),
            network: false,
            context: None,
            context_open: false,
            cache: None,
            edge_cache: RefCell::new(None),
            unselected_edge_cache: RefCell::new(None),
            size: Rc::default(),
            origin: Rc::default(),
            adjusted: None,
            drag: None,
            pan_moved: false,
            resource_focus: Vec::new(),
            group_focus: HashMap::new(),
            last_focus: None,
            focus_view: Weak::new(),
            zoom_focus: None,
            mode_focus: None,
            skip_link: None,
            skip_target: None,
            built: RefCell::default(),
            unselected_built: RefCell::default(),
            prepared: RefCell::default(),
            links_stale: false,
        }
    }
}

/// Every input to the derived view is compared before reusing it.
struct ViewInputs {
    expanded: HashSet<String>,
    network: bool,
    data: Rc<topology::Prepared>,
    selected: Option<SelectionIdentity>,
}

#[derive(PartialEq)]
struct SelectionIdentity {
    cluster: String,
    kind: String,
    namespace: Option<String>,
    name: String,
    api_version: Option<String>,
}

impl ViewInputs {
    fn matches(
        &self,
        data: &Rc<topology::Prepared>,
        selected: Option<&ResourceSummary>,
        expanded: &HashSet<String>,
        network: bool,
    ) -> bool {
        Rc::ptr_eq(&self.data, data)
            && self.expanded == *expanded
            && self.network == network
            && match (&self.selected, selected) {
                (None, None) => true,
                (Some(identity), Some(resource)) => {
                    identity.cluster == resource.cluster
                        && identity.kind == resource.kind
                        && identity.namespace == resource.namespace
                        && identity.name == resource.name
                        && identity.api_version == resource.api_version
                }
                _ => false,
            }
    }
}

const ZOOM_STEP: f32 = 1.2;
/// The handle's 6px circle is centered on the card edge; edges meet its outer side.
const HANDLE_HALF: f32 = 3.;

impl Inspector {
    pub(super) fn rebuild_map(&mut self) {
        let links_stale = std::mem::take(&mut self.map.links_stale);
        let topology = match &self.map.topology {
            TopologyLoad::Ready(topology) => Some(topology),
            _ => None,
        };
        let view = topology.map(|topology| {
            let _span = crate::frame_profile::scope("map", "prepare_view");
            let rows = self.visible.generation();
            let selected = if self.persisted.preferences.show_full_topology_on_selection {
                None
            } else {
                self.detail.resource()
            };
            let nodes = (topology.nodes.as_ptr() as usize, topology.nodes.len());
            let edges = (topology.edges.as_ptr() as usize, topology.edges.len());
            let mut prepared = self.map.prepared.borrow_mut();
            let same_source = prepared.as_ref().is_some_and(|previous| {
                previous.nodes == nodes
                    && previous.edges == edges
                    && Rc::ptr_eq(&previous.request, &self.request)
                    && previous.source.matches(&self.visible)
            });
            let hide_historical = self.filters.hide_historical_replicasets;
            let current = same_source
                && prepared.as_ref().unwrap().rows == rows
                && prepared.as_ref().unwrap().hide_historical == hide_historical;
            // The link mask follows the kind selection, so it is recomputed only
            // when the rows, the graph, the toggle or the selection change, not every frame.
            if !current || links_stale {
                let links = self.map_links(topology);
                if same_source {
                    let prepared = prepared.as_mut().unwrap();
                    if !current || prepared.links != links {
                        prepared.data = Rc::new(topology::prepare(
                            topology,
                            &self.visible,
                            &prepared.source,
                            links.as_deref(),
                            hide_historical,
                        ));
                        prepared.rows = rows;
                        prepared.links = links;
                        prepared.hide_historical = hide_historical;
                    }
                } else {
                    let source = topology::Source::new(topology, &self.visible);
                    let data = Rc::new(topology::prepare(
                        topology,
                        &self.visible,
                        &source,
                        links.as_deref(),
                        hide_historical,
                    ));
                    *prepared = Some(PreparedMap {
                        rows,
                        hide_historical,
                        links,
                        nodes,
                        edges,
                        request: self.request.clone(),
                        source,
                        data,
                    });
                }
            }
            let data = &prepared.as_ref().unwrap().data;
            // Selection changes preserve the full graph for closing the detail pane.
            let cache = if selected.is_none() {
                &self.map.unselected_built
            } else {
                &self.map.built
            };
            let mut built = cache.borrow_mut();
            match &*built {
                Some((inputs, view))
                    if inputs.matches(data, selected, &self.map.expanded, self.map.network) =>
                {
                    view.clone()
                }
                _ => {
                    let view = Rc::new(topology::view(
                        topology,
                        data,
                        selected,
                        &self.map.expanded,
                        self.map.network,
                    ));
                    let inputs = ViewInputs {
                        expanded: self.map.expanded.clone(),
                        network: self.map.network,
                        data: data.clone(),
                        selected: selected.map(|resource| SelectionIdentity {
                            cluster: resource.cluster.clone(),
                            kind: resource.kind.clone(),
                            namespace: resource.namespace.clone(),
                            name: resource.name.clone(),
                            api_version: resource.api_version.clone(),
                        }),
                    };
                    *built = Some((inputs, view.clone()));
                    view
                }
            }
        });
        if topology.is_none() {
            self.map.built.take();
            self.map.unselected_built.take();
            self.map.prepared.take();
        }
        self.map.cache = view;
    }

    pub(super) fn map_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::frame_profile::scope("map", "build");
        let topology = match &self.map.topology {
            TopologyLoad::Ready(topology) => Some(topology),
            _ => None,
        };
        let view = self.map.cache.clone();
        let edges = view.as_ref().map_or(0, |view| view.edges.len());
        let subtitle = match &view {
            Some(view) if view.focused => format!(
                "{} focused / {} nodes / {edges} edges",
                view.nodes.len(),
                view.topology_nodes
            ),
            _ => format!(
                "{} nodes / {edges} edges",
                view.as_ref().map_or(0, |view| view.topology_nodes)
            ),
        };
        let warnings = topology.map_or(&[][..], |topology| &topology.warnings[..]);
        // Built before `segment` borrows `cx` for its click listeners.
        let context_select = self.map_context_select(cx);
        let mode_focus = self.map.mode_focus.clone();
        let segment = |icon: &str, label: &'static str, active: bool| {
            let network = label == "Network Flow";
            let focus = mode_focus
                .as_ref()
                .map(|handles| handles[usize::from(network)].clone());
            div()
                .id(SharedString::from(format!("map-mode:{label}")))
                .test_support()
                .aria_label(label)
                .aria_selected(active)
                .cursor_pointer()
                .when_some(focus, |segment, focus| {
                    segment
                        .track_focus(&focus)
                        .focus(|style| style.border_2().border_color(Tokens::ring()))
                })
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_map_mode(network, window, cx)),
                )
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.set_map_mode(network, window, cx);
                        cx.stop_propagation();
                    }
                }))
                .flex()
                .items_center()
                .gap(px(4.))
                .h(px(28.))
                .px(px(8.))
                .rounded(px(6.))
                .when(active, |segment| segment.bg(Tokens::secondary()))
                .child(ui::icon(icon, 14., Tokens::foreground()))
                .child(ui::medium(ui::xs(label.to_owned())))
        };
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        ui::ghost_icon("panel-left-close", 28., 16.)
                            .id("panel-control:Collapse ownership map")
                            .test_support()
                            .aria_label("Collapse ownership map")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_panel_open(Panel::Map, false);
                                cx.notify();
                            })),
                    )
                    .child(ui::icon(
                        if self.map.network {
                            "network"
                        } else {
                            "git-branch"
                        },
                        24.,
                        Tokens::foreground(),
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                ui::semibold(ui::xs(if self.map.network {
                                    "Network Flow"
                                } else {
                                    "Ownership Map"
                                }))
                                .truncate(),
                            )
                            .child(
                                ui::muted(ui::text(subtitle.clone(), 11., 16.))
                                    .id("map-counts")
                                    .test_support()
                                    .aria_label(subtitle),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .children(context_select)
                    .child(
                        div()
                            .flex()
                            .h(px(32.))
                            .p(px(2.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::background())
                            .child(segment("git-branch", "Ownership", !self.map.network))
                            .child(segment("network", "Network Flow", self.map.network)),
                    )
                    .when(!warnings.is_empty(), |row| {
                        row.child(ui::outline_badge(warnings.len().to_string()))
                    }),
            );
        let centered = |content: Div| {
            div()
                .flex()
                .flex_1()
                .min_h(px(320.))
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(content)
        };
        let loading = self
            .loading
            .phase("map", matches!(self.map.topology, TopologyLoad::Loading));
        let body: Vec<AnyElement> = match (&self.map.topology, &view) {
            // As in OwnershipMap.svelte, a held indicator precedes a failure.
            _ if !loading.ready() => {
                if loading.shown() {
                    vec![centered(ui::muted(ui::xs("Loading topology"))).id("map-loading").test_support().aria_label("Loading topology").into_any_element()]
                } else {
                    vec![centered(div()).into_any_element()]
                }
            }
            (TopologyLoad::Failed(error), _) => vec![
                div()
                    .p(px(12.))
                    .child(
                        div()
                            .id("map-error")
                            .test_support()
                            .aria_description(error.message.clone())
                            .role(gpui_kit::Role::Alert)
                            .aria_label(if error.kind == AppErrorKind::Forbidden {
                                "KubeCove does not have permission for this"
                            } else if self.map.network {
                                "Failed to load network flow"
                            } else {
                                "Failed to load ownership map"
                            })
                            .p(px(12.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::destructive().opacity(0.4))
                            .child(ui::semibold(ui::xs(if error.kind == AppErrorKind::Forbidden { "KubeCove does not have permission for this" } else if self.map.network { "Failed to load network flow" } else { "Failed to load ownership map" })))
                            .child(ui::muted(ui::xs(error.message.clone()))),
                    )
                    .into_any_element(),
            ],
            (TopologyLoad::Ready(_), Some(view)) if !view.nodes.is_empty() => {
                let mut body = vec![
                    div()
                        .flex()
                        .flex_wrap()
                        .px(px(12.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(Tokens::border())
                        .child(ui::muted(ui::text(
                            "Drag to pan. Scroll or use controls to zoom. Press Tab to reach graph nodes, or\u{a0}",
                            11.,
                            16.,
                        )))
                        // Button `variant="link"`: `text-primary`.
                        .child(
                            ui::text("skip graph", 11., 16.)
                                .id("map-skip-graph")
                                .test_support()
                                .aria_label("skip graph")
                                .text_color(Tokens::primary())
                                .cursor_pointer()
                                .when_some(self.map.skip_link.as_ref(), |link, focus| {
                                    link.track_focus(focus).focus(|style| {
                                        style.border_b_1().border_color(Tokens::ring())
                                    })
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(target) = &this.map.skip_target {
                                        target.focus(window, cx);
                                    }
                                }))
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, window, cx| {
                                        if matches!(
                                            event.keystroke.key.as_str(),
                                            "enter" | "space"
                                        ) && let Some(target) = &this.map.skip_target
                                        {
                                            target.focus(window, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                        .child(ui::muted(ui::text(".", 11., 16.)))
                        .into_any_element(),
                    self.map_viewport(view, cx),
                    // Focus target after the graph. Tab skips it, so only "skip graph" lands here.
                    match &self.map.skip_target {
                        Some(target) => div()
                            .id("map-graph-end")
                            .test_support()
                            .track_focus(target)
                            .into_any_element(),
                        None => div().into_any_element(),
                    },
                ];
                if !warnings.is_empty() {
                    body.push(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .px(px(12.))
                            .py(px(8.))
                            .border_t_1()
                            .border_color(Tokens::border())
                            .child(ui::semibold(ui::text("Topology warnings", 11., 16.)))
                            .children(warnings.iter().take(3).map(|warning| {
                                ui::muted(ui::text(warning.clone(), 11., 16.)).truncate().id(SharedString::from(format!("map-warning:{warning}"))).test_support().aria_label(warning.clone())
                            }))
                            .when(warnings.len() > 3, |list| {
                                list.child(ui::muted(ui::text(
                                    format!("{} more warnings", warnings.len() - 3),
                                    11.,
                                    16.,
                                )))
                            })
                            .into_any_element(),
                    );
                }
                body
            }
            _ => vec![
                centered(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(4.))
                        .child(ui::medium(ui::sm(if self.map.network { "No network flow" } else { "No ownership graph" })).id("map-empty").test_support().aria_label(if self.map.network { "No network flow" } else { "No ownership graph" }))
                        .child(ui::muted(ui::xs(
                            if self.map.network { "No ingress, service, or pod traffic relationships were found in this scope." } else { "No workload ownership relationships were found in this scope." },
                        ))),
                )
                .into_any_element(),
            ],
        };
        ui::card()
            .size_full()
            .min_h(px(400.))
            .shadow_sm()
            .child(header)
            .children(body)
    }
}
