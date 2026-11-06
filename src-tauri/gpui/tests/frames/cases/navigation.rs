//! Main navigation and selection interactions in the controlled fixture.
use super::*;
const SETTINGS: &[&str] = &[
    "Appearance",
    "Live sessions",
    "YAML",
    "Diagnostics",
    "AI agents",
    "Advanced",
    "Remote server",
    "Updates",
    "Kubeconfig",
    "General",
];
fn close_detail(d: &mut Driver, _: usize) {
    d.act(|w, cx| {
        if w.try_find("close-detail").is_some() {
            w.click("close-detail", cx);
        }
    });
}
pub(super) static CASES: &[Case] = &[
    Case {
        id: "workspace-editor-open",
        description: "Open the saved workspace editor from the launcher",
        setup: |d| views::setup(d, "workspaces"),
        prepare: Some(|d, _| {
            d.act(|w, cx| {
                if w.try_find("workspace-cancel-edit").is_some() {
                    w.click("workspace-cancel-edit", cx);
                }
            })
        }),
        aim: |w, _| w.find(("workspace-edit", 1_usize)).bounds().center(),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "forward-start-typing",
        description: "Type and erase the local port in the forward start form",
        setup: |d| {
            views::setup(d, "forward-start");
            d.act(|w, cx| {
                w.click("forward-local", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "19090")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "forward-local")),
    },
    Case {
        id: "forward-start-submit",
        description: "Start the controlled forward, with the previous session stopped between samples",
        setup: |d| views::setup(d, "forward-start"),
        prepare: Some(|d, _| {
            d.act(|w, cx| {
                let target = "stop-forward:Pod/payments-api-7d9c9b7f8d-k2r9p:9090";
                if w.try_find(target).is_some() {
                    views::reveal_detail(w, target, cx);
                    w.click(target, cx);
                    w.render_frame(cx);
                }
                views::reveal_detail(w, "forward-local", cx);
                w.click("forward-local", cx);
                w.press("ctrl-a", cx);
                w.input("19090", cx);
                views::reveal_detail(w, "forward-start", cx);
            })
        }),
        aim: |w, _| center(w, "forward-start"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "forward-local")),
    },
    Case {
        id: "forward-stop",
        description: "Stop the controlled active forward",
        setup: |d| views::setup(d, "forward-start"),
        prepare: Some(|d, _| {
            let stop = "stop-forward:Pod/payments-api-7d9c9b7f8d-k2r9p:9090";
            if d.read(|w| w.try_find(stop).is_none()) {
                d.act(|w, cx| {
                    views::reveal_detail(w, "forward-local", cx);
                    w.click("forward-local", cx);
                    w.press("ctrl-a", cx);
                    w.input("19090", cx);
                    views::reveal_detail(w, "forward-start", cx);
                    w.click("forward-start", cx);
                });
            }
            d.act(|w, cx| views::reveal_detail(w, stop, cx));
        }),
        aim: |w, _| center(w, "stop-forward:Pod/payments-api-7d9c9b7f8d-k2r9p:9090"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: Some(|w| {
            w.try_find("stop-forward:Pod/payments-api-7d9c9b7f8d-k2r9p:9090")
                .is_some()
                .to_string()
        }),
    },
    Case {
        id: "helm-release-open",
        description: "Open Helm release details from the list",
        setup: |d| views::setup(d, "helm-list"),
        prepare: Some(|d, _| {
            d.act(|w, cx| {
                if w.try_find("helm-close").is_some() {
                    views::parity_scroll::reveal(w, "helm-list", "helm-close", cx);
                    w.click("helm-close", cx);
                    w.render_frame(cx);
                }
                views::parity_scroll::reveal(
                    w,
                    "helm-list",
                    "helm-details:platform:metrics-gateway",
                    cx,
                );
                let columns = w.find("helm-release:platform:metrics-gateway").bounds();
                let viewport = w.find("helm-list").bounds();
                let right = viewport.right().min(w.viewport_size().width);
                let left = columns.left().max(viewport.left());
                let at = point((left + right) / 2., columns.center().y);
                let target = w.find("helm-details:platform:metrics-gateway").bounds();
                if target.right() > right || target.left() < left {
                    w.dispatch_event(
                        ScrollEvent {
                            position: at,
                            delta: ScrollDelta::Pixels(point(at.x - target.center().x, px(0.))),
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    w.render_frame(cx);
                }
            })
        }),
        aim: |w, _| center(w, "helm-details:platform:metrics-gateway"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "workspace-editor-typing",
        description: "Type and erase payments in workspace-name",
        setup: |d| {
            views::setup(d, "workspace-editor");
            d.act(|w, cx| {
                w.click("workspace-name", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "payments")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "workspace-name")),
    },
    Case {
        id: "rbac-typing",
        description: "Type and erase admin in rbac-search",
        setup: |d| {
            views::setup(d, "rbac");
            d.act(|w, cx| {
                w.click("rbac-search", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "admin")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "rbac-search")),
    },
    Case {
        id: "settings-page-navigation",
        description: "Cycle every settings category including diagnostics and agents",
        setup: |d| views::setup(d, "settings"),
        prepare: None,
        aim: |w, i| {
            center(
                w,
                &format!("settings-section:{}", SETTINGS[i % SETTINGS.len()]),
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "argo-document-navigation",
        description: "Switch the connected Argo view between desired manifests and changes",
        setup: |d| views::setup(d, "argo-connected"),
        prepare: Some(|d, i| {
            d.act(|w, cx| {
                views::reveal_detail(
                    w,
                    if i.is_multiple_of(2) {
                        "argo-view:Desired"
                    } else {
                        "argo-view:Changes"
                    },
                    cx,
                )
            })
        }),
        aim: |w, i| {
            center(
                w,
                if i.is_multiple_of(2) {
                    "argo-view:Desired"
                } else {
                    "argo-view:Changes"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "argo-review-open",
        description: "Prepare the controlled Argo Sync review without executing it",
        setup: |d| views::setup(d, "argo-connected"),
        prepare: Some(|d, _| {
            d.act(|w, cx| {
                if w.try_find("argo-review-cancel").is_some() {
                    views::reveal_detail(w, "argo-review-cancel", cx);
                    w.click("argo-review-cancel", cx);
                    w.render_frame(cx);
                }
                views::reveal_detail(w, "argo-operation:Sync", cx);
            })
        }),
        aim: |w, _| center(w, "argo-operation:Sync"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "flux-view-toggle",
        description: "Switch Flux controllers between cards and list",
        setup: |d| views::setup(d, "flux"),
        prepare: None,
        aim: |w, i| {
            center(
                w,
                if i.is_multiple_of(2) {
                    "gitops-view:List"
                } else {
                    "gitops-view:Cards"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "gitops-application-open",
        description: "Open the root Argo application's inspection panel",
        setup: |d| views::setup(d, "gitops"),
        prepare: Some(|d, index| {
            close_detail(d, index);
            d.act(|w, cx| {
                assert!(views::parity_scroll::seek(
                    w,
                    cx,
                    "gitops-list",
                    "gitops-details:Application:argocd:root-application",
                ));
            });
        }),
        aim: |w, _| center(w, "gitops-details:Application:argocd:root-application"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "network-flow-selection",
        description: "Select the payments Pod from the network-flow map",
        setup: |d| {
            views::setup(d, "network-flow");
            d.act(|w, cx| w.click("namespace:payments", cx));
        },
        prepare: Some(|d, index| {
            close_detail(d, index);
            d.act(|w, cx| {
                views::parity_scroll::reveal(
                    w,
                    "resources",
                    "map-node:mock-dev:v1:Pod:payments:payments-api-7d9c9b7f8d-k2r9p",
                    cx,
                )
            });
        }),
        aim: |w, _| {
            center(
                w,
                "map-node:mock-dev:v1:Pod:payments:payments-api-7d9c9b7f8d-k2r9p",
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "metrics-selection",
        description: "Select the payments Deployment from the metrics map",
        setup: |d| {
            views::setup(d, "metrics");
            d.act(|w, cx| w.click("namespace:payments", cx));
        },
        prepare: Some(|d, index| {
            close_detail(d, index);
            d.act(|w, cx| {
                views::parity_scroll::reveal(
                    w,
                    "resources",
                    "map-node:mock-dev:apps/v1:Deployment:payments:payments-api",
                    cx,
                )
            });
        }),
        aim: |w, _| {
            center(
                w,
                "map-node:mock-dev:apps/v1:Deployment:payments:payments-api",
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
];
