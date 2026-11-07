//! Navigation for frame views follows the controlled offscreen visual driver.
use super::super::Driver;
use gpui_kit::test::{TestEventExt, TestWindowExt};
use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};
#[path = "../../parity/scroll.rs"]
pub(super) mod parity_scroll;

pub(super) fn reveal_detail(window: &mut gpui_kit::Window, id: &str, cx: &mut gpui_kit::App) {
    parity_scroll::reveal(window, "detail-body", id, cx);
}
pub(super) fn setup(driver: &mut Driver, view: &str) {
    match view {
        "argo-connected" | "argo-review" | "argo-desired" => {
            driver.act(|w, cx| w.click("nav:GitOps", cx));
            driver.act(|w, cx| {
                assert!(parity_scroll::seek(
                    w,
                    cx,
                    "gitops-list",
                    "gitops-details:Application:argocd:tenant-ledger"
                ));
                w.click("gitops-details:Application:argocd:tenant-ledger", cx)
            });
            driver.act(|w, cx| {
                parity_scroll::reveal(w, "detail-body", "argo-connect-open", cx);
                w.click("argo-connect-open", cx);
                w.render_frame(cx);
                parity_scroll::reveal(w, "detail-body", "argo-url", cx);
                w.click("argo-url", cx);
                w.input("https://argocd.example.test", cx);
                parity_scroll::reveal(w, "detail-body", "argo-token", cx);
                w.click("argo-token", cx);
                w.input("parity-fixture-token", cx);
            });
            driver.act(|w, cx| {
                parity_scroll::reveal(w, "detail-body", "argo-connect-submit", cx);
                w.click("argo-connect-submit", cx);
            });
            if view == "argo-review" {
                driver.act(|w, cx| {
                    reveal_detail(w, "argo-operation:Sync", cx);
                    w.click("argo-operation:Sync", cx);
                });
            }
            if view == "argo-desired" {
                driver.act(|w, cx| {
                    reveal_detail(w, "argo-view:Desired", cx);
                    w.click("argo-view:Desired", cx);
                });
            }
            if view == "argo-connected" {
                driver.act(|w, cx| {
                    reveal_detail(w, "argo-view:Changes", cx);
                    w.click("argo-view:Changes", cx);
                    w.render_frame(cx);
                    reveal_detail(w, "argo-diff:0", cx);
                });
            }
        }
        "settings-agents"
        | "settings-appearance"
        | "settings-sessions"
        | "settings-yaml"
        | "settings-diagnostics"
        | "settings-advanced"
        | "settings-remote"
        | "settings-updates"
        | "settings-kubeconfig" => {
            driver.act(|window, cx| window.click("open-settings", cx));
            let category = match view {
                "settings-agents" => "AI agents",
                "settings-appearance" => "Appearance",
                "settings-sessions" => "Live sessions",
                "settings-yaml" => "YAML",
                "settings-diagnostics" => "Diagnostics",
                "settings-advanced" => "Advanced",
                "settings-remote" => "Remote server",
                "settings-updates" => "Updates",
                _ => "Kubeconfig",
            };
            driver.act(|window, cx| window.click(format!("settings-section:{category}"), cx));
        }
        "forward-start" | "forward-active" | "exec-session" => {
            driver.act(|window, cx| window.click("open-resources", cx));
            driver.act(|window, cx| window.click("namespace:payments", cx));
            driver.act(|window, cx| {
                window.click("search", cx);
                window.input("payments-api-7d9c9b7f8d-k2r9p", cx);
            });
            driver.act(|window, cx| {
                window.click("resource:Pod:payments:payments-api-7d9c9b7f8d-k2r9p", cx)
            });
            driver.act(|window, cx| {
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
                driver.act(|window, cx| window.click("exec-start", cx));
            } else {
                driver.act(|window, cx| {
                    window.click("forward-remote", cx);
                    window.input("9090", cx);
                    window.click("forward-local", cx);
                    window.input("19090", cx);
                });
                if view == "forward-active" {
                    driver.act(|window, cx| window.click("forward-start", cx));
                }
            }
        }
        "overview" => {}
        "live-limited" | "live-failed" | "live-reconnecting" => {
            driver.act(|window, cx| window.click("open-resources", cx));
            driver.backend.replay_live(if view == "live-limited" {
                "forbidden"
            } else if view == "live-failed" {
                "failed"
            } else {
                "reconnecting"
            });
        }
        "kind-picker" => {
            driver.act(|window, cx| window.click("open-resources", cx));
            driver.act(|window, cx| window.click("kinds-picker", cx));
            driver.act(|window, cx| {
                window.click("kind-search", cx);
                window.input("Service", cx);
            });
        }
        "custom-search" => {
            driver.act(|window, cx| window.click("nav:Custom Resources", cx));
            driver.act(|window, cx| {
                window.click("custom-resource-search", cx);
                window.input("kustom", cx);
            });
        }
        "custom-resources" => {
            driver.act(|window, cx| window.click("nav:Custom Resources", cx));
            driver.act(|window, cx| window.click("custom-group:kustomize.toolkit.fluxcd.io", cx));
            driver.act(|window, cx| {
                window.click(
                    "custom-kind:kustomize.toolkit.fluxcd.io/v1:Kustomization",
                    cx,
                )
            });
        }
        "usage-footer" => {
            driver.act(|window, cx| window.click("open-settings", cx));
            driver.act(|window, cx| window.click("setting:Show CPU and memory footer", cx));
        }
        "incidents" => driver.act(|window, cx| window.click("nav:Incidents", cx)),
        "port-forwards" => driver.act(|window, cx| window.click("nav:Port Forwards", cx)),
        "resources" | "metrics" => driver.act(|window, cx| window.click("open-resources", cx)),
        "logs.pod" | "logs.pod-paused" => {
            driver.act(|window, cx| window.click("open-resources", cx));
            driver.act(|window, cx| {
                window.click("search", cx);
                window.input("argocd-dex-server", cx);
            });
            driver.act(|window, cx| {
                window.click("resource:Pod:argocd:argocd-dex-server-6d7f9c5b8-q4w2n", cx)
            });
            driver.act(|window, cx| window.click("show-logs", cx));
            if view == "logs.pod-paused" {
                driver.act(|window, cx| window.click("log-pause", cx));
            }
        }
        "network-flow" => {
            driver.act(|window, cx| window.click("open-resources", cx));
            driver.act(|window, cx| window.click("map-mode:Network Flow", cx));
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
            super::open_detail(driver);
            if view != "detail" {
                let tab = format!("show-{}", view.split('-').next().unwrap());
                driver.act(|window, cx| window.click(tab, cx));
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
                driver.act(|window, cx| {
                    window.click("action-filter", cx);
                    window.input(label, cx);
                });
                let id = format!("action-{editor}");
                driver.act(|window, cx| {
                    parity_scroll::reveal(window, "actions-scroll", &id, cx);
                    window.click(id, cx);
                });
                driver.act(|window, cx| {
                    parity_scroll::reveal(
                        window,
                        "actions-scroll",
                        &format!("action-row-{editor}"),
                        cx,
                    )
                });
            }
            if view == "yaml-kyaml" {
                driver.act(|window, cx| window.click("yaml-encoding", cx));
                driver.act(|window, cx| {
                    window.press("down", cx);
                    window.press("enter", cx);
                });
            }
            if view.starts_with("yaml-edit") {
                driver.act(|window, cx| window.click("edit-draft", cx));
            }
        }
        "cluster-overview" => driver.act(|window, cx| window.click("nav:Cluster Overview", cx)),
        "rbac" => driver.act(|window, cx| window.click("nav:RBAC", cx)),
        "workspaces" => driver.act(|window, cx| window.click("header-workspaces", cx)),
        "workspace-editor" => {
            driver.act(|window, cx| window.click("header-workspaces", cx));
            driver.act(|window, cx| window.click(("workspace-edit", 1_usize), cx));
        }
        "settings" => driver.act(|window, cx| window.click("open-settings", cx)),
        "palette" => {
            driver.act(|window, cx| window.click("open-palette", cx));
            driver.act(|window, cx| window.input("payments", cx));
        }
        "gitops" | "gitops-list" | "flux" => {
            driver.act(|window, cx| window.click("nav:GitOps", cx));
            if view == "gitops-list" {
                driver.act(|window, cx| window.click("gitops-view:List", cx));
            }
            if view == "flux" {
                driver.act(|window, cx| window.click("nav-toggle:GitOps", cx));
                driver.act(|window, cx| window.click("nav:Flux", cx));
            }
        }
        "argo-app" | "yaml-edit-kyaml" => {
            driver.act(|window, cx| window.click("nav:GitOps", cx));
            driver.act(|window, cx| {
                window.click("gitops-details:Application:argocd:root-application", cx)
            });
            if view == "yaml-edit-kyaml" {
                driver.act(|window, cx| window.click("show-yaml", cx));
                for picker in ["yaml-shape", "yaml-encoding"] {
                    driver.act(|window, cx| window.click(picker, cx));
                    driver.act(|window, cx| {
                        window.press("down", cx);
                        window.press("enter", cx);
                    });
                }
                driver.act(|window, cx| window.click("edit-draft", cx));
            }
        }
        "argo-workspace" => {
            driver.act(|window, cx| window.click("nav:GitOps", cx));
            driver.act(|window, cx| {
                window.click(
                    "gitops-resources:argoApp:mock-dev:argocd:platform-argocd",
                    cx,
                )
            });
        }
        "helm" | "helm-list" | "helm-release" => {
            driver.act(|window, cx| window.click("nav:Helm", cx));
            if view == "helm-list" {
                driver.act(|window, cx| window.click("helm-view:List", cx));
            }
            if view == "helm-release" {
                driver.act(|window, cx| window.click("helm-details:platform:metrics-gateway", cx));
                driver.act(|window, cx| {
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
        driver.act(|window, cx| {
            window.click("search", cx);
            window.input("payments-api", cx);
        });
    }
}
