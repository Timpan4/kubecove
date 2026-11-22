//! The developer topology harness, mirroring
//! `src/features/resources/topology-spike/TopologyHarness.svelte`. It lays out
//! the synthetic graph with the map's Network Flow layout and draws it with
//! the harness's own node and edge styles. Debug builds open it from
//! Diagnostics; it replaces the workspace until closed, as the web harness
//! replaces the app page.

mod graph;
mod run;
mod view;

use crate::topology::Viewport;
use gpui_kit::{Context, Pixels, Point, SharedString, Window};
use graph::Graph;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use super::Inspector;

/// `TOPOLOGY_SPIKE_URL`'s query in `DiagnosticsSettings.svelte`.
const LOD_4K: &str =
    "nodes=4000&viewport=focused&nodeDetail=compact&edgeMode=selected&edgeType=straight";
/// Replaces `LOD_4K`, as editing the harness URL's query does on the web.
const QUERY_ENV: &str = "KUBECOVE_TOPOLOGY_SPIKE";
/// `minZoom` and `maxZoom` on the harness's `SvelteFlow`.
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 1.5;
/// The harness's `initialViewport`, used by the focused viewport.
const INITIAL_VIEWPORT: Viewport = Viewport {
    x: 64.,
    y: 72.,
    zoom: 0.7,
};
/// xyflow's default `fitView` padding, used by the fit viewport.
const FIT_PADDING: f32 = 0.1;

/// The harness's URL parameters and their defaults.
pub(super) struct Options {
    nodes: usize,
    straight: bool,
    compact: bool,
    selected_edges: bool,
    focused: bool,
    autorun: bool,
}

impl Options {
    fn parse(query: &str) -> Self {
        let value = |key: &str| {
            query
                .split('&')
                .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value)
        };
        Self {
            // `Number("abc")` is NaN on the web, which generates no nodes.
            nodes: value("nodes").map_or(1000, |nodes| nodes.parse().unwrap_or(0)),
            straight: value("edgeType") == Some("straight"),
            compact: value("nodeDetail") == Some("compact"),
            selected_edges: value("edgeMode") == Some("selected"),
            focused: value("viewport") == Some("focused"),
            autorun: value("autorun").is_some(),
        }
    }

    /// The header's options, as `TopologyHarness.svelte` prints them.
    fn summary(&self) -> String {
        format!(
            "{} viewport / {} nodes / {} {} edges",
            if self.focused { "focused" } else { "fit" },
            if self.compact { "compact" } else { "full" },
            if self.selected_edges {
                "selected"
            } else {
                "all"
            },
            if self.straight {
                "straight"
            } else {
                "smoothstep"
            },
        )
    }

    /// A node's size in flow units: the harness's compact or full node style.
    fn node_size(&self) -> (f32, f32) {
        if self.compact {
            (48., 22.)
        } else {
            (220., 74.)
        }
    }
}

pub(in crate::inspector) struct Spike {
    options: Options,
    graph: Graph,
    selected: Option<usize>,
    /// xyflow's interactivity toggle: off, nodes do not select.
    interactive: bool,
    /// `None` until the fit viewport has a measured size.
    viewport: Option<Viewport>,
    size: Rc<Cell<(f32, f32)>>,
    origin: Rc<Cell<(f32, f32)>>,
    drag: Option<(Point<Pixels>, Viewport)>,
    pan_moved: bool,
    opened: Instant,
    /// Milliseconds from opening to the first settled frame.
    initial_render_ms: Option<f64>,
    /// Nodes and edges the last frame drew, after viewport culling.
    rendered: Cell<(usize, usize)>,
    run: Option<run::Run>,
    result: Option<SharedString>,
}

impl Spike {
    fn new(query: &str) -> Self {
        let opened = Instant::now();
        let options = Options::parse(query);
        let graph = Graph::build(options.nodes, options.node_size());
        Self {
            viewport: options.focused.then_some(INITIAL_VIEWPORT),
            options,
            graph,
            selected: None,
            interactive: true,
            size: Rc::default(),
            origin: Rc::default(),
            drag: None,
            pan_moved: false,
            opened,
            initial_render_ms: None,
            rendered: Cell::new((0, 0)),
            run: None,
            result: None,
        }
    }

    /// The header line: counts, layout time and options.
    fn counts(&self) -> String {
        format!(
            "{} nodes / {} edges / layout {} ms / {}",
            self.graph.nodes.len(),
            self.graph.edges.len(),
            self.graph.layout_ms.round(),
            self.options.summary()
        )
    }

    /// xyflow's `getViewportForBounds` with a symmetric fractional padding.
    fn fit(&self, padding: f32) -> Option<Viewport> {
        let (width, height) = self.size.get();
        let (x, y, bound_width, bound_height) = self.graph.bounds?;
        if width <= 0. || height <= 0. {
            return None;
        }
        let pad = |length: f32| ((length - length / (1. + padding)) * 0.5).floor();
        let zoom = ((width - 2. * pad(width)) / bound_width)
            .min((height - 2. * pad(height)) / bound_height)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        Some(Viewport {
            x: width / 2. - (x + bound_width / 2.) * zoom,
            y: height / 2. - (y + bound_height / 2.) * zoom,
            zoom,
        })
    }

    /// Zooms by `factor` about `anchor` in canvas coordinates, as d3-zoom's `scaleBy` does.
    fn zoom_about(&mut self, factor: f32, anchor: (f32, f32)) {
        let Some(current) = self.viewport else {
            return;
        };
        let zoom = (current.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let ratio = zoom / current.zoom;
        self.viewport = Some(Viewport {
            x: anchor.0 - (anchor.0 - current.x) * ratio,
            y: anchor.1 - (anchor.1 - current.y) * ratio,
            zoom,
        });
    }
}

impl Inspector {
    /// Diagnostics' "Open 4k LOD", or the query in `KUBECOVE_TOPOLOGY_SPIKE`.
    pub(in crate::inspector) fn open_topology_spike(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = std::env::var(QUERY_ENV).unwrap_or_else(|_| LOD_4K.to_owned());
        self.open_topology_harness(&query, window, cx);
    }

    /// Opens the harness with a query in the web harness's URL form, such as
    /// `nodes=4000&viewport=fit`. Release builds have no Settings entry, so
    /// `tests/frames.rs` opens it here.
    pub fn open_topology_harness(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.spike = Some(Box::new(Spike::new(query)));
        self.measure_spike_first_render(window, cx);
        cx.notify();
    }

    fn close_topology_spike(&mut self, cx: &mut Context<Self>) {
        self.spike = None;
        cx.notify();
    }
}
