//! Renders the native client offscreen with the browser-mock fixture so its
//! screenshots can be compared with the web frontend's. Driven by
//! `scripts/gpui-visual.ts`; without its environment the test only reports a skip.

fn main() {
    #[cfg(target_os = "linux")]
    visual::run();
    #[cfg(not(target_os = "linux"))]
    println!("visual: skipped; the capture driver runs on Linux");
}

#[cfg(target_os = "linux")]
mod visual {
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, px,
        size,
    };
    use kubecove_gpui::{
        Inspector,
        assets::AppAssets,
        backend::{Backend, CLUSTER_KINDS, LiveSessions, Read, Scope, ScopeResult},
        gitops::GitOpsInventory,
    };
    use kubecove_lib::models::{
        AppError, AppErrorKind, ArgoApplicationSummary, ClusterContext, FluxResourceSummary,
        IncidentCockpitSummary, NamespaceSummary, PodExecSessionSummary, PortForwardSessionSummary,
        ResourceDetailsFull, ResourceEventSummary, ResourceSummary, ResourceTopology,
    };
    use serde::Deserialize;
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
    };

    #[derive(Deserialize)]
    struct Fixture {
        #[serde(rename = "contexts")]
        cluster_contexts: Vec<ClusterContext>,
        namespaces: Vec<NamespaceSummary>,
        resources: Vec<ResourceSummary>,
        details: Vec<ResourceDetailsFull>,
        events: Vec<ResourceEvents>,
        topology: ResourceTopology,
        gitops: GitOps,
        /// Per-namespace graphs, which the incident guide reads for Pods.
        #[serde(rename = "namespaceTopologies")]
        namespace_topologies: std::collections::HashMap<String, ResourceTopology>,
        incidents: IncidentCockpitSummary,
        #[serde(rename = "liveSessions")]
        live_sessions: FixtureSessions,
    }

    #[derive(Deserialize)]
    struct GitOps {
        argo: Option<Vec<ArgoApplicationSummary>>,
        flux: Option<Vec<FluxResourceSummary>>,
    }

    #[derive(Deserialize)]
    struct ResourceEvents {
        kind: String,
        name: String,
        namespace: Option<String>,
        events: Vec<ResourceEventSummary>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureSessions {
        port_forwards: Vec<PortForwardSessionSummary>,
        exec_sessions: Vec<PodExecSessionSummary>,
        source_key: Option<String>,
        show_source_labels: bool,
    }

    impl Backend for Fixture {
        fn contexts(&self) -> Read<Vec<ClusterContext>> {
            let contexts = self.cluster_contexts.clone();
            Box::pin(async move { Ok(contexts) })
        }
        fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
            // Cluster-scoped kinds come from `cluster_resources`, as in the native backend.
            let resources = self
                .resources
                .iter()
                .filter(|row| !CLUSTER_KINDS.contains(&row.kind.as_str()))
                .filter(|row| {
                    scope.namespace.is_none()
                        || row.namespace.as_deref() == scope.namespace.as_deref()
                })
                .cloned()
                .collect();
            let namespaces = self.namespaces.clone();
            Box::pin(async move {
                Ok(ScopeResult {
                    namespaces,
                    resources,
                    warnings: vec![],
                })
            })
        }
        fn details(&self, resource: ResourceSummary) -> Read<ResourceDetailsFull> {
            let found = self
                .details
                .iter()
                .find(|detail| {
                    detail.summary.kind == resource.kind
                        && detail.summary.name == resource.name
                        && detail.summary.namespace == resource.namespace
                })
                .cloned();
            Box::pin(async move {
                found.ok_or_else(|| AppError::new("No fixture details", AppErrorKind::NotFound))
            })
        }
        /// The all-namespaces graph, or a namespace's graph the fixture exported.
        fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
            let topology = scope
                .namespace
                .and_then(|namespace| self.namespace_topologies.get(&namespace))
                .unwrap_or(&self.topology)
                .clone();
            Box::pin(async move { Ok(topology) })
        }
        fn gitops(&self, _: String) -> Read<GitOpsInventory> {
            let inventory = GitOpsInventory {
                argo: self.gitops.argo.clone(),
                flux: self.gitops.flux.clone(),
            };
            Box::pin(async move { Ok(inventory) })
        }
        fn cluster_resources(&self, _: String, _: bool) -> Read<ScopeResult> {
            let resources = self
                .resources
                .iter()
                .filter(|row| CLUSTER_KINDS.contains(&row.kind.as_str()))
                .cloned()
                .collect();
            Box::pin(async move {
                Ok(ScopeResult {
                    namespaces: vec![],
                    resources,
                    warnings: vec![],
                })
            })
        }
        fn events(
            &self,
            resource: ResourceSummary,
            _: Option<String>,
        ) -> Read<Vec<ResourceEventSummary>> {
            let found = self
                .events
                .iter()
                .find(|row| {
                    row.kind == resource.kind
                        && row.name == resource.name
                        && row.namespace == resource.namespace
                })
                .map_or_else(Vec::new, |row| row.events.clone());
            Box::pin(async move { Ok(found) })
        }
        fn incidents(&self, _: String) -> Read<IncidentCockpitSummary> {
            let incidents = self.incidents.clone();
            Box::pin(async move { Ok(incidents) })
        }
        fn live_sessions(&self) -> Read<LiveSessions> {
            let sessions = &self.live_sessions;
            let sessions = LiveSessions {
                port_forwards: sessions.port_forwards.clone(),
                exec_sessions: sessions.exec_sessions.clone(),
                source_key: sessions.source_key.clone(),
                show_source_labels: sessions.show_source_labels,
            };
            Box::pin(async move { Ok(sessions) })
        }
    }

    fn capture(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, out: &Path, view: &str) {
        cx.run_until_parked();
        // The first frame measures the ownership map's viewport; the second fits the graph to it.
        for _ in 0..2 {
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        let image = cx
            .capture_screenshot(handle)
            .expect("offscreen rendering must be available");
        let path = out.join(format!("{view}.png"));
        image.save(&path).unwrap();
        println!(
            "gpui {view} {}x{} -> {}",
            image.width(),
            image.height(),
            path.display()
        );
    }

    fn act(
        cx: &mut HeadlessAppContext,
        handle: AnyWindowHandle,
        action: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
    ) {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            action(window, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    pub fn run() {
        let (Some(fixture), Some(out)) = (
            std::env::var_os("KUBECOVE_VISUAL_FIXTURE"),
            std::env::var_os("KUBECOVE_VISUAL_OUT"),
        ) else {
            println!("visual: skipped; run `bun scripts/gpui-visual.ts`");
            return;
        };
        let fixture: Fixture =
            serde_json::from_slice(&std::fs::read(fixture).expect("fixture must be readable"))
                .expect("fixture must match the Rust models");
        let backend = Arc::new(fixture);
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let views: Vec<String> = std::env::var("KUBECOVE_VISUAL_VIEWS")
            .unwrap_or_else(|_| "overview,resources,detail,yaml,cluster-overview".into())
            .split(',')
            .map(str::to_owned)
            .collect();
        for view in views {
            let mut cx = HeadlessAppContext::with_platform(
                gpui_kit::platform::current_platform(true).text_system(),
                Arc::new(AppAssets),
                gpui_kit::platform::current_headless_renderer,
            );
            cx.update(|cx| {
                gpui_kit::init(cx);
                kubecove_gpui::init(cx).expect("fonts load");
            });
            let backend = backend.clone();
            let (handle, _) = cx
                .update(|cx| {
                    gpui_kit::open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds {
                                origin: Default::default(),
                                size: size(px(1440.), px(900.)),
                            })),
                            focus: false,
                            show: false,
                            ..Default::default()
                        },
                        cx,
                        |window, cx| {
                            cx.new(|cx| {
                                Inspector::new(backend, window, cx)
                                    .with_workspace_name("Parity mock workspace")
                            })
                        },
                    )
                })
                .unwrap();
            cx.run_until_parked();
            match view.as_str() {
                "overview" => {}
                "incidents" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Incidents", cx)
                }),
                "port-forwards" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Port Forwards", cx)
                }),
                "resources" => act(&mut cx, handle, |window, cx| {
                    window.click("open-resources", cx)
                }),
                "detail" | "yaml" | "events" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-resources", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("search", cx);
                        window.input("payments-api", cx);
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("resource:Deployment:payments:payments-api", cx)
                    });
                    if view != "detail" {
                        let tab = if view == "yaml" {
                            "show-yaml"
                        } else {
                            "show-events"
                        };
                        act(&mut cx, handle, |window, cx| window.click(tab, cx));
                    }
                }
                "cluster-overview" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Cluster Overview", cx)
                }),
                other => panic!("unknown view: {other}"),
            }
            capture(&mut cx, handle, &out, &view);
        }
    }
}
