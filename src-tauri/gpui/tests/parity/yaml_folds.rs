//! Shared steps that fold a YAML block from the editor gutter and read which
//! document lines stay visible. Lines are 1-based, as the gutter numbers them.

use super::*;
use gpui_kit::ElementId;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
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
