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
mod common;

#[cfg(target_os = "linux")]
#[path = "parity/scroll.rs"]
mod parity_scroll;

#[cfg(target_os = "linux")]
mod visual {
    use crate::common::Fixture;
    use crate::parity_scroll;
    use gpui_kit::test::{TestEventExt, TestWindowExt};
    use gpui_kit::{
        AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, InputEvent, ScrollDelta,
        ScrollEvent, WindowBounds, WindowOptions, point, px, size,
    };
    use kubecove_gpui::{Inspector, assets::AppAssets};
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
    };

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
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let views: Vec<String> = std::env::var("KUBECOVE_VISUAL_VIEWS")
            .unwrap_or_else(|_| "overview,resources,detail,yaml,cluster-overview".into())
            .split(',')
            .map(str::to_owned)
            .collect();
        for view in views {
            let backend = Arc::new(Fixture::load(Path::new(&fixture)));
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
                                Inspector::new(backend.clone(), window, cx)
                                    .with_workspace_name("Parity mock workspace")
                            })
                        },
                    )
                })
                .unwrap();
            cx.run_until_parked();
            match view.as_str() {
                "settings-remote" | "settings-updates" | "settings-kubeconfig" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-settings", cx)
                    });
                    let category = match view.as_str() {
                        "settings-remote" => "Remote server",
                        "settings-updates" => "Updates",
                        _ => "Kubeconfig",
                    };
                    act(&mut cx, handle, |window, cx| {
                        window.click(format!("settings-section:{category}"), cx)
                    });
                }
                "forward-start" | "forward-active" | "exec-session" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-resources", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("search", cx);
                        window.input("payments-api-7d9c9b7f8d-k2r9p", cx);
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("resource:Pod:payments:payments-api-7d9c9b7f8d-k2r9p", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click(
                            if view == "exec-session" {
                                "show-exec"
                            } else {
                                "show-forward"
                            },
                            cx,
                        )
                    });
                    if view == "exec-session" {
                        act(&mut cx, handle, |window, cx| window.click("exec-start", cx));
                    } else {
                        act(&mut cx, handle, |window, cx| {
                            window.click("forward-remote", cx);
                            window.input("9090", cx);
                            window.click("forward-local", cx);
                            window.input("19090", cx);
                        });
                        if view == "forward-active" {
                            act(&mut cx, handle, |window, cx| {
                                window.click("forward-start", cx)
                            });
                        }
                    }
                }
                "overview" => {}
                "live-limited" | "live-failed" | "live-reconnecting" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-resources", cx)
                    });
                    backend.replay_live(if view == "live-limited" {
                        "forbidden"
                    } else if view == "live-failed" {
                        "failed"
                    } else {
                        "reconnecting"
                    });
                }
                "custom-resources" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("nav:Custom Resources", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("custom-group:kustomize.toolkit.fluxcd.io", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click(
                            "custom-kind:kustomize.toolkit.fluxcd.io/v1:Kustomization",
                            cx,
                        )
                    });
                }
                "usage-footer" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-settings", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("setting:Show CPU and memory footer", cx)
                    });
                }
                "incidents" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Incidents", cx)
                }),
                "port-forwards" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Port Forwards", cx)
                }),
                "resources" | "metrics" => act(&mut cx, handle, |window, cx| {
                    window.click("open-resources", cx)
                }),
                "network-flow" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-resources", cx)
                    });
                    act(&mut cx, handle, |window, cx| {
                        window.click("map-mode:Network Flow", cx)
                    });
                }
                "detail"
                | "yaml"
                | "events"
                | "logs"
                | "revisions"
                | "actions"
                | "actions-image"
                | "actions-environment"
                | "actions-resources"
                | "actions-labels"
                | "actions-annotations"
                | "actions-rollback"
                | "yaml-edit"
                | "yaml-kyaml" => {
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
                        let tab = format!("show-{}", view.split('-').next().unwrap());
                        act(&mut cx, handle, |window, cx| window.click(tab, cx));
                    }
                    if let Some(editor) = view.strip_prefix("actions-") {
                        let label = match editor {
                            "image" => "Image",
                            "environment" => "Environment",
                            "resources" => "Requests and limits",
                            "labels" => "Labels",
                            "annotations" => "Annotations",
                            "rollback" => "Revision",
                            _ => unreachable!(),
                        };
                        act(&mut cx, handle, |window, cx| {
                            window.click("action-filter", cx);
                            window.input(label, cx);
                        });
                        let id = format!("action-{editor}");
                        act(&mut cx, handle, |window, cx| {
                            parity_scroll::reveal(window, "actions-scroll", &id, cx);
                            window.click(id, cx);
                        });
                        act(&mut cx, handle, |window, cx| {
                            parity_scroll::reveal(
                                window,
                                "actions-scroll",
                                &format!("action-row-{editor}"),
                                cx,
                            )
                        });
                    }
                    if view == "yaml-kyaml" {
                        act(&mut cx, handle, |window, cx| {
                            window.click("yaml-encoding", cx)
                        });
                        act(&mut cx, handle, |window, cx| {
                            window.press("down", cx);
                            window.press("enter", cx);
                        });
                    }
                    if view.starts_with("yaml-edit") {
                        act(&mut cx, handle, |window, cx| window.click("edit-draft", cx));
                    }
                }
                "cluster-overview" => act(&mut cx, handle, |window, cx| {
                    window.click("nav:Cluster Overview", cx)
                }),
                "rbac" => act(&mut cx, handle, |window, cx| window.click("nav:RBAC", cx)),
                "workspaces" => act(&mut cx, handle, |window, cx| {
                    window.click("header-workspaces", cx)
                }),
                "settings" => act(&mut cx, handle, |window, cx| {
                    window.click("open-settings", cx)
                }),
                "palette" => {
                    act(&mut cx, handle, |window, cx| {
                        window.click("open-palette", cx)
                    });
                    act(&mut cx, handle, |window, cx| window.input("payments", cx));
                }
                "gitops" | "gitops-list" | "flux" => {
                    act(&mut cx, handle, |window, cx| window.click("nav:GitOps", cx));
                    if view == "gitops-list" {
                        act(&mut cx, handle, |window, cx| {
                            window.click("gitops-view:List", cx)
                        });
                    }
                    if view == "flux" {
                        act(&mut cx, handle, |window, cx| {
                            window.click("nav-toggle:GitOps", cx)
                        });
                        act(&mut cx, handle, |window, cx| window.click("nav:Flux", cx));
                    }
                }
                "argo-app" | "yaml-edit-kyaml" => {
                    act(&mut cx, handle, |window, cx| window.click("nav:GitOps", cx));
                    act(&mut cx, handle, |window, cx| {
                        window.click("gitops-details:Application:argocd:root-application", cx)
                    });
                    if view == "yaml-edit-kyaml" {
                        act(&mut cx, handle, |window, cx| window.click("show-yaml", cx));
                        for picker in ["yaml-shape", "yaml-encoding"] {
                            act(&mut cx, handle, |window, cx| window.click(picker, cx));
                            act(&mut cx, handle, |window, cx| {
                                window.press("down", cx);
                                window.press("enter", cx);
                            });
                        }
                        act(&mut cx, handle, |window, cx| window.click("edit-draft", cx));
                    }
                }
                "argo-workspace" => {
                    act(&mut cx, handle, |window, cx| window.click("nav:GitOps", cx));
                    act(&mut cx, handle, |window, cx| {
                        window.click(
                            "gitops-resources:argoApp:mock-dev:argocd:platform-argocd",
                            cx,
                        )
                    });
                }
                "helm" | "helm-list" | "helm-release" => {
                    act(&mut cx, handle, |window, cx| window.click("nav:Helm", cx));
                    if view == "helm-list" {
                        act(&mut cx, handle, |window, cx| {
                            window.click("helm-view:List", cx)
                        });
                    }
                    if view == "helm-release" {
                        act(&mut cx, handle, |window, cx| {
                            window.click("helm-details:platform:metrics-gateway", cx)
                        });
                        act(&mut cx, handle, |window, cx| {
                            assert!(parity_scroll::seek(
                                window,
                                cx,
                                "helm-list",
                                "helm-release-details"
                            ));
                            let list = window.find("helm-list").bounds();
                            let details = window.find("helm-release-details").bounds();
                            let position = list.origin + point(px(4.), px(4.));
                            window.pointer_move(position, None, cx);
                            window.dispatch_event(
                                ScrollEvent {
                                    position,
                                    delta: ScrollDelta::Pixels(point(
                                        px(0.),
                                        list.origin.y - details.origin.y,
                                    )),
                                    ..Default::default()
                                }
                                .to_platform_input(),
                                cx,
                            );
                        });
                    }
                }
                other => panic!("unknown view: {other}"),
            }
            if view == "metrics" {
                act(&mut cx, handle, |window, cx| {
                    window.click("search", cx);
                    window.input("payments-api", cx);
                });
            }
            capture(&mut cx, handle, &out, &view);
        }
    }
}
