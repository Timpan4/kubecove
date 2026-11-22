//! Input loops for native views added after the original frame harness.
use super::*;
pub(super) static CASES: &[Case] = &[
    Case {
        id: "live-watch-burst",
        description: "Replay every row of export-watch-burst.ts through the live watches",
        setup: open_resources,
        prepare: Some(|d, index| {
            // Let the existing watch throttle return to receive before the next burst.
            std::thread::sleep(kubecove_gpui::inspector::RESOURCE_WATCH_COALESCE);
            d.cx.settle();
            d.backend.prepare_live_burst(index);
        }),
        aim: nowhere,
        step: Some(|_, _, _, _| {}),
        fixture: Some("fixture-watch-burst.json"),
        replay: Some(|backend, _, index| {
            backend.replay_live(if index.is_multiple_of(2) {
                "burst"
            } else {
                "burst-reset"
            })
        }),
        probe: Some(|window| {
            assert!(
                window
                    .find("resource-table-summary")
                    .label()
                    .is_some_and(|label| label.starts_with("10000 resources ·")),
                "watch fixture must retain all 10,000 rows"
            );
            String::new()
        }),
    },
    Case {
        id: "exec-output-burst",
        description: "Append the retained 1,000-line log fixture to the active terminal",
        setup: |d| views::setup(d, "exec-session"),
        prepare: None,
        aim: nowhere,
        step: Some(|_, _, _, _| {}),
        fixture: None,
        replay: Some(|backend, text, index| {
            let content = text.strip_suffix("\r\n").expect("log fixture newline");
            backend.terminal_output("app", &format!("{content} burst-{index}\r\n"));
        }),
        probe: Some(|w| {
            w.find("exec-terminal")
                .label()
                .unwrap_or_default()
                .to_string()
        }),
    },
    Case {
        id: "exec-selection-drag",
        description: "Drag a selection through the controlled terminal output",
        setup: |d| {
            views::setup(d, "exec-session");
            d.act(|w, cx| {
                let b = w.find(id("exec-terminal")).bounds();
                let at = b.origin + point(px(8.), px(8.));
                move_to(w, cx, at, None);
                mouse_down(w, cx, at);
            });
        },
        prepare: None,
        aim: |w, _| w.find(id("exec-terminal")).bounds().origin + point(px(8.), px(8.)),
        step: Some(|w, cx, i, at| {
            let phase = i % 20;
            let offset = if phase < 10 { phase } else { 20 - phase } as f32 * 8.;
            move_to(
                w,
                cx,
                at + point(px(offset), px(0.)),
                Some(MouseButton::Left),
            );
        }),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "helm-search-typing",
        description: "Type and erase gateway in the Helm search",
        setup: |d| {
            views::setup(d, "helm");
            d.act(|w, cx| w.click("helm-search", cx));
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "gateway")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "helm-search")),
    },
    Case {
        id: "kind-picker-toggle",
        description: "Toggle the Service kind in the searched picker",
        setup: |d| views::setup(d, "kind-picker"),
        prepare: None,
        aim: |w, _| center(w, "pick-kind:Service"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "gitops-view-toggle",
        description: "Switch between GitOps cards and list",
        setup: |d| views::setup(d, "gitops"),
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
        id: "helm-view-toggle",
        description: "Switch between Helm cards and list",
        setup: |d| views::setup(d, "helm"),
        prepare: None,
        aim: |w, i| {
            center(
                w,
                if i.is_multiple_of(2) {
                    "helm-view:List"
                } else {
                    "helm-view:Cards"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "palette-typing",
        description: "Type and erase deployment in palette-query",
        setup: |d| {
            views::setup(d, "palette");
            d.act(|w, cx| {
                w.click("palette-query", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "deployment")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "palette-query")),
    },
    Case {
        id: "kind-picker-typing",
        description: "Type and erase Pod in kind-search",
        setup: |d| {
            views::setup(d, "kind-picker");
            d.act(|w, cx| {
                w.click("kind-search", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "Pod")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "kind-search")),
    },
    Case {
        id: "custom-search-typing",
        description: "Type and erase helm in custom-resource-search",
        setup: |d| {
            views::setup(d, "custom-search");
            d.act(|w, cx| {
                w.click("custom-resource-search", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "helm")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "custom-resource-search")),
    },
    Case {
        id: "settings-search-typing",
        description: "Type and erase logs in settings-search",
        setup: |d| {
            views::setup(d, "settings");
            d.act(|w, cx| {
                w.click("settings-search", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "logs")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "settings-search")),
    },
    Case {
        id: "network-flow-pan",
        description: "pan the network-flow map",
        setup: |d| {
            views::setup(d, "network-flow");
            d.act(|w, cx| {
                let at = map_point(w, 0);
                move_to(w, cx, at, None);
                mouse_down(w, cx, at);
            });
        },
        prepare: None,
        aim: map_point,
        step: Some(|w, cx, i, at| {
            let phase = i % 20;
            let offset = if phase < 10 { phase } else { 20 - phase } as f32 * 8.;
            move_to(
                w,
                cx,
                at + point(px(offset), px(offset / 2.)),
                Some(MouseButton::Left),
            );
        }),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "network-flow-zoom",
        description: "zoom the network-flow map",
        setup: |d| {
            views::setup(d, "network-flow");
        },
        prepare: None,
        aim: |w, i| {
            center(
                w,
                if i % 6 < 3 {
                    "map-zoom-in"
                } else {
                    "map-zoom-out"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "metrics-pan",
        description: "pan the metrics map",
        setup: |d| {
            views::setup(d, "metrics");
            d.act(|w, cx| {
                let at = map_point(w, 0);
                move_to(w, cx, at, None);
                mouse_down(w, cx, at);
            });
        },
        prepare: None,
        aim: map_point,
        step: Some(|w, cx, i, at| {
            let phase = i % 20;
            let offset = if phase < 10 { phase } else { 20 - phase } as f32 * 8.;
            move_to(
                w,
                cx,
                at + point(px(offset), px(offset / 2.)),
                Some(MouseButton::Left),
            );
        }),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "metrics-zoom",
        description: "zoom the metrics map",
        setup: |d| {
            views::setup(d, "metrics");
        },
        prepare: None,
        aim: |w, i| {
            center(
                w,
                if i % 6 < 3 {
                    "map-zoom-in"
                } else {
                    "map-zoom-out"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "events-scroll",
        description: "Scroll the event detail from its ancestor gutter",
        setup: |d| open_tab(d, "show-events"),
        prepare: None,
        aim: |w, _| {
            let b = w.find(id("detail-body")).bounds();
            point(b.left() + px(4.), b.top() + b.size.height / 2.)
        },
        step: Some(|w, cx, i, at| wheel(w, cx, at, back_and_forth(i, 30, 40.))),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "timeline-scroll",
        description: "Scroll the timeline from its ancestor gutter",
        setup: |d| open_detail(d),
        prepare: None,
        aim: |w, _| {
            let b = w.find(id("detail-body")).bounds();
            point(b.left() + px(4.), b.top() + b.size.height / 2.)
        },
        step: Some(|w, cx, i, at| wheel(w, cx, at, back_and_forth(i, 30, 40.))),
        fixture: None,
        replay: None,
        probe: None,
    },
];
