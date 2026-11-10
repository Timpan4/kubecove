//! Text-fit checks that need real glyph widths. The parity driver shapes text with GPUI's
//! `NoopTextSystem`, where every glyph is 0.6 em wide, so these checks render the
//! browser-mock fixture with the platform text system and the bundled fonts, as
//! `tests/visual.rs` does, in the parity driver's 1440 by 900 window.
//!
//! ```sh
//! bun e2e/parity/visual/export-fixture.ts e2e/artifacts/text-fit/fixture.json
//! KUBECOVE_PARITY_FIXTURE="$PWD/e2e/artifacts/text-fit/fixture.json" \
//!   cargo test --manifest-path src-tauri/gpui/Cargo.toml --test text_fit
//! ```

fn main() {
    #[cfg(target_os = "linux")]
    text_fit::run();
    #[cfg(not(target_os = "linux"))]
    println!("text_fit: skipped; the headless renderer runs on Linux");
}

// This target uses only the fixture loader, not the live-data and timing controls.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod common;

#[cfg(target_os = "linux")]
mod text_fit {
    use crate::common::Fixture;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, Pixels, TextRun, WindowBounds,
        WindowOptions, px, size,
    };
    use kubecove_gpui::{
        Inspector,
        assets::AppAssets,
        theme::{SANS, SM_FONT_SIZE, XS_FONT_SIZE},
    };
    use std::{path::Path, sync::Arc};

    /// The resources page of a new client on the mock workspace, with the bundled fonts.
    fn open(fixture: &Path) -> (HeadlessAppContext, AnyWindowHandle) {
        let backend = Arc::new(Fixture::load(fixture));
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(AppAssets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::init(cx);
            kubecove_gpui::init(cx).expect("fonts load");
        });
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
                            Inspector::new(backend, window, cx).with_saved_workspace(
                                "Parity mock workspace",
                                "mock-dev",
                                cx,
                            )
                        })
                    },
                )
            })
            .unwrap();
        cx.run_until_parked();
        act(&mut cx, handle, |window, cx| {
            window.click("open-resources", cx)
        });
        (cx, handle)
    }

    fn act<T>(
        cx: &mut HeadlessAppContext,
        handle: AnyWindowHandle,
        action: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App) -> T,
    ) -> T {
        let result = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                action(window, cx)
            })
            .unwrap();
        cx.run_until_parked();
        result
    }

    fn search(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, value: &str) {
        act(cx, handle, |window, cx| {
            window.click("search", cx);
            window.input(value, cx);
        });
    }

    /// The width of `text` in the regular sans font, as `ui::xs` and `ui::sm` render it.
    fn shaped_width(window: &mut gpui_kit::Window, text: &str, font_size: f32) -> Pixels {
        let run = TextRun {
            len: text.len(),
            font: gpui_kit::font(SANS),
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(text.to_owned().into(), px(font_size), &[run], None)
            .width
    }

    /// Whether `text` at `font_size` shows in full in a text box `box_width` wide.
    ///
    /// GPUI truncates text only when its shaped width exceeds the width the box is measured
    /// at. Layout then snaps box edges to device pixels, which can make the reported box up
    /// to one device pixel narrower than that measured width (see the pixel-snapping notes in
    /// `gpui-fast`'s `taffy.rs`), so the check allows that one pixel.
    fn shows_in_full(
        window: &mut gpui_kit::Window,
        text: &str,
        font_size: f32,
        box_width: Pixels,
    ) -> Result<(), String> {
        let width = shaped_width(window, text, font_size);
        let snapping = px(1. / window.scale_factor());
        if width <= box_width + snapping {
            Ok(())
        } else {
            Err(format!(
                "{text:?} is {width:?} wide in a {box_width:?} box, so it ends in an ellipsis"
            ))
        }
    }

    /// The Kind column fits "Deployment" without an ellipsis.
    fn deployment_kind_fits(fixture: &Path) {
        let (mut cx, handle) = open(fixture);
        search(&mut cx, handle, "argocd-server");
        act(&mut cx, handle, |window, _| {
            let value = window
                .find("value:Kind:resource:Deployment:argocd:argocd-server:mock-dev")
                .bounds();
            if let Err(problem) =
                shows_in_full(window, "Deployment", SM_FONT_SIZE, value.size.width)
            {
                panic!("Kind column: {problem}");
            }
        });
    }

    /// Clicks the element labeled `label` inside `scope`.
    fn click_label(
        window: &mut gpui_kit::Window,
        scope: &'static str,
        label: &str,
        cx: &mut gpui_kit::App,
    ) {
        let target = window.within(scope).find_by_label(label);
        let id = target
            .path()
            .last()
            .expect("labeled element has an ID")
            .clone();
        window.click(id, cx);
    }

    /// With admin@solid-k8s added to the workspace, the Context column fits
    /// "admin@solid-k8s", the longest fixture context name, without an ellipsis.
    fn context_column_fits(fixture: &Path) {
        let (mut cx, handle) = open(fixture);
        act(&mut cx, handle, |window, cx| {
            window.click("header-workspaces", cx)
        });
        act(&mut cx, handle, |window, cx| {
            click_label(window, "launcher-body", "Edit Parity mock workspace", cx)
        });
        act(&mut cx, handle, |window, cx| {
            window.click("workspace-group:admin@solid-k8s", cx)
        });
        act(&mut cx, handle, |window, cx| {
            window.click("workspace-create", cx)
        });
        act(&mut cx, handle, |window, cx| {
            click_label(window, "launcher-body", "Open Parity mock workspace", cx)
        });
        act(&mut cx, handle, |window, cx| {
            if window.try_find("resources").is_none() {
                window.click("open-resources", cx);
            }
        });
        search(&mut cx, handle, "edge-relay");
        act(&mut cx, handle, |window, _| {
            let value = window
                .find("value:Context:resource:Deployment:platform:edge-relay:admin@solid-k8s")
                .bounds();
            if let Err(problem) =
                shows_in_full(window, "admin@solid-k8s", SM_FONT_SIZE, value.size.width)
            {
                panic!("Context column: {problem}");
            }
        });
    }

    /// With the detail panel open on a multi-container Pod, the live Logs status reads in full
    /// beside the container picker.
    fn live_log_status_shows_in_full(fixture: &Path) {
        let (mut cx, handle) = open(fixture);
        search(&mut cx, handle, "argocd-dex-server");
        act(&mut cx, handle, |window, cx| {
            window.click(
                "resource:Pod:argocd:argocd-dex-server-6d7f9c5b8-q4w2n:mock-dev",
                cx,
            )
        });
        act(&mut cx, handle, |window, cx| window.click("show-logs", cx));
        act(&mut cx, handle, |window, _| {
            let status = window.find("log-status");
            let label = status.label().unwrap_or_default().to_owned();
            assert!(label.starts_with("Live · "), "log status is {label:?}");
            for id in ["log-status-state", "log-status-count"] {
                let part = window.find(id);
                let text = part.label().unwrap_or_default().to_owned();
                if let Err(problem) =
                    shows_in_full(window, &text, XS_FONT_SIZE, part.bounds().size.width)
                {
                    panic!("Log status {label:?}: {problem}");
                }
            }
        });
    }

    pub fn run() {
        let Some(fixture) = std::env::var_os("KUBECOVE_PARITY_FIXTURE") else {
            println!(
                "text_fit: skipped; export a fixture with `bun e2e/parity/visual/export-fixture.ts <path>` and set KUBECOVE_PARITY_FIXTURE"
            );
            return;
        };
        let fixture = Path::new(&fixture);
        deployment_kind_fits(fixture);
        println!("text_fit deployment-kind-fits: passed");
        context_column_fits(fixture);
        println!("text_fit context-column-fits: passed");
        live_log_status_shows_in_full(fixture);
        println!("text_fit live-log-status-shows-in-full: passed");
    }
}
