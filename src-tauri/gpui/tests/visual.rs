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
mod visual {
    use crate::common::Fixture;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, px,
        size,
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
        let backend = Arc::new(Fixture::load(Path::new(&fixture)));
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
