//! Shared YAML editor layout and folding checks.

use super::*;
use gpui_kit::{
    Bounds, DevicePixels, ElementId, Font, FontId, FontMetrics, FontRun, Global, GlyphId,
    LineLayout, NoopTextSystem, Pixels, PlatformTextSystem, RenderGlyphParams, Size,
    TextRenderingMode, px,
};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct WhitespaceGlyphs {
    dot: AtomicBool,
    arrow: AtomicBool,
}

struct WhitespaceLayout(Arc<WhitespaceGlyphs>);

impl Global for WhitespaceLayout {}

/// The detail body and editor viewport heights that the last
/// `expectYamlEditorFillsPanel` measured, for a `grows` check on the next one.
#[derive(Default)]
struct YamlFillHistory(Option<(Pixels, Pixels)>);

impl Global for YamlFillHistory {}

impl PlatformTextSystem for WhitespaceLayout {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> gpui_kit::Result<()> {
        NoopTextSystem.add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        NoopTextSystem.all_font_names()
    }

    fn font_id(&self, descriptor: &Font) -> gpui_kit::Result<FontId> {
        NoopTextSystem.font_id(descriptor)
    }

    fn font_metrics(&self, font: FontId) -> FontMetrics {
        NoopTextSystem.font_metrics(font)
    }

    fn typographic_bounds(&self, font: FontId, glyph: GlyphId) -> gpui_kit::Result<Bounds<f32>> {
        NoopTextSystem.typographic_bounds(font, glyph)
    }

    fn advance(&self, font: FontId, glyph: GlyphId) -> gpui_kit::Result<Size<f32>> {
        NoopTextSystem.advance(font, glyph)
    }

    fn glyph_for_char(&self, font: FontId, ch: char) -> Option<GlyphId> {
        NoopTextSystem.glyph_for_char(font, ch)
    }

    fn glyph_raster_bounds(
        &self,
        glyph: &RenderGlyphParams,
    ) -> gpui_kit::Result<Bounds<DevicePixels>> {
        NoopTextSystem.glyph_raster_bounds(glyph)
    }

    fn rasterize_glyph(
        &self,
        glyph: &RenderGlyphParams,
        bounds: Bounds<DevicePixels>,
    ) -> gpui_kit::Result<(Size<DevicePixels>, Vec<u8>)> {
        NoopTextSystem.rasterize_glyph(glyph, bounds)
    }

    fn layout_line(&self, text: &str, size: Pixels, runs: &[FontRun]) -> LineLayout {
        match text {
            "•" => self.0.dot.store(true, Ordering::Relaxed),
            "→" => self.0.arrow.store(true, Ordering::Relaxed),
            _ => {}
        }
        NoopTextSystem.layout_line(text, size, runs)
    }

    fn recommended_rendering_mode(&self, font: FontId, size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(font, size)
    }
}

pub(super) fn with_marker_recording(
    cx: &TestAppContext,
    scenario: &Value,
) -> Option<TestAppContext> {
    if !scenario["steps"]
        .as_array()?
        .iter()
        .any(|step| step["action"] == "expectYamlEditorFillsPanel")
    {
        return None;
    }
    // GPUI's test window doesn't expose painted text. Record the standalone
    // glyphs its editor shapes for whitespace without changing text metrics.
    let glyphs = Arc::new(WhitespaceGlyphs::default());
    let recording = TestAppContext::build_with_text_system(
        cx.dispatcher.clone(),
        cx.test_function_name(),
        Arc::new(WhitespaceLayout(glyphs.clone())),
    );
    recording.update(|cx| {
        cx.set_global(WhitespaceLayout(glyphs));
        cx.set_global(YamlFillHistory::default());
    });
    Some(recording)
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectYamlEditorFillsPanel" => {
            let glyphs = cx.global::<WhitespaceLayout>().0.clone();
            window.render_frame(cx);
            let body = window.find("detail-body").bounds();
            let frame = window.find("yaml-editor").bounds();
            let (editor, _) = gpui_kit::base::Root::read(window, cx)
                .view()
                .clone()
                .downcast::<Inspector>()
                .expect("the Inspector is the window's content")
                .read(cx)
                .yaml_editor_viewport(cx)
                .expect("YAML editor is open");
            // The detail body has 16px bottom padding. The input has two 1px
            // borders (its own and the YAML frame) plus GPUI editor padding.
            assert_eq!(
                body.bottom() - frame.bottom(),
                px(16.),
                "detail body {body:?}, YAML frame {frame:?}"
            );
            let inset = px(2.) + gpui_kit::component::Size::Medium.input_py();
            assert_eq!(
                frame.bottom() - editor.bottom(),
                inset,
                "YAML frame {frame:?}, editor viewport {editor:?}"
            );
            assert!(
                glyphs.dot.load(Ordering::Relaxed) && glyphs.arrow.load(Ordering::Relaxed),
                "the YAML editor did not shape its whitespace markers in this frame (dot {}, arrow {})",
                glyphs.dot.load(Ordering::Relaxed),
                glyphs.arrow.load(Ordering::Relaxed),
            );
            let previous = cx
                .global_mut::<YamlFillHistory>()
                .0
                .replace((body.size.height, editor.size.height));
            if step["grows"].as_bool() == Some(true) {
                let (panel, viewport) = previous.expect(
                    "an earlier expectYamlEditorFillsPanel records the heights to grow from",
                );
                assert!(
                    body.size.height > panel,
                    "the detail body did not grow: {panel:?} to {:?}",
                    body.size.height
                );
                assert!(
                    editor.size.height > viewport,
                    "the YAML editor viewport did not grow: {viewport:?} to {:?}",
                    editor.size.height
                );
            }
        }
        "foldYamlLine" => fold(window, cx, number(step, "line") as usize),
        "expectYamlVisibleLines" => {
            let expected: Vec<usize> = step["lines"]
                .as_array()
                .expect("missing lines")
                .iter()
                .map(|line| line.as_u64().expect("a line number") as usize)
                .collect();
            assert_eq!(
                visible_lines(window, cx),
                expected,
                "wrong visible YAML lines"
            );
        }
        _ => return false,
    }
    true
}

/// Focuses the editor with the pointer over its text, away from the gutter,
/// and moves the cursor to the start of the document.
fn focus_start(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    reveal(window, "detail-body", "yaml-editor", cx);
    let editor = locate::id(&window.within("detail-body").find_by_label("Resource YAML"));
    window.within("detail-body").click(editor, cx);
    window.press(
        if cfg!(target_os = "macos") {
            "cmd-up"
        } else {
            "ctrl-home"
        },
        cx,
    );
}

/// The document's line count, as the gutter numbers it.
fn line_count(window: &mut gpui_kit::Window) -> usize {
    window
        .find_by_label("Resource YAML")
        .value()
        .map_or(0, |text| text.split('\n').count())
}

/// Clicks the fold chevron of `line`, which must be below the first line.
/// While the pointer is off the gutter, the editor paints the chevrons of
/// folded lines and of the cursor line, so the chevron that appears when the
/// cursor moves from the first line onto `line` is that line's.
fn fold(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, line: usize) {
    assert!(line > 1, "the first line is where the cursor starts");
    focus_start(window, cx);
    window.render_frame(cx);
    let before = painted_chevrons(window);
    // Down moves by visible line, so count the lines that folds skip.
    let passed = walk(window, cx, Some(line));
    assert_eq!(passed.last(), Some(&line), "line {line} is hidden");
    window.render_frame(cx);
    let appeared: Vec<ElementId> = painted_chevrons(window)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(
        appeared.len(),
        1,
        "line {line} should show its own chevron: {appeared:?}, before {before:?}"
    );
    window.click(appeared[0].clone(), cx);
    window.render_frame(cx);
}

/// GPUI Kit numbers the chevrons from the top, at most one per line.
fn painted_chevrons(window: &mut gpui_kit::Window) -> Vec<ElementId> {
    (0..line_count(window))
        .map(|index| ElementId::from(("fold-icon", index)))
        .filter(|id| {
            window
                .try_find(id.clone())
                .is_some_and(|chevron| chevron.visible())
        })
        .collect()
}

/// Walks the cursor down from the first line. Each Shift+Down selects up to
/// the next visible line, so the copied selection counts the lines it skips.
/// On the last visible line the selection no longer ends at a line start.
fn visible_lines(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) -> Vec<usize> {
    focus_start(window, cx);
    walk(window, cx, None)
}

/// Moves the cursor down one visible line at a time from the first line, until
/// it reaches the start of line `stop`, or the last visible line without one.
/// Returns the visible lines it passed.
fn walk(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, stop: Option<usize>) -> Vec<usize> {
    let lines = line_count(window);
    let mut line = 1;
    let mut visible = vec![line];
    loop {
        assert!(
            line <= lines,
            "the cursor walked past line {lines}: {visible:?}"
        );
        if stop == Some(line) {
            return visible;
        }
        // An empty selection copies nothing, so clear what the last step copied.
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(String::new()));
        window.press("shift-down", cx);
        window.press("secondary-c", cx);
        let copied = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        // Collapses the selection to its end.
        window.press("right", cx);
        if !copied.ends_with('\n') {
            return visible;
        }
        line += copied.matches('\n').count();
        visible.push(line);
    }
}
