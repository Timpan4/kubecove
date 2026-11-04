//! Indent tints for the YAML editor, from `addIndentDecorations` and the
//! `.kubecove-yaml-indent-N` styles in `src/components/yamlCodeViewerExtensions.ts`.

use super::Inspector;
use crate::theme::oklch;
use gpui_kit::component::input::{
    EditorState, InputEvent, RangeDecoration, RangeDecorationCollection, RangeDecorationStyle,
};
use gpui_kit::{App, Context, Entity, Subscription};

/// `--kubecove-yaml-indent-color` per depth (`depth % 6`), drawn at
/// `color-mix(in oklch, color 11%, transparent)`.
const DEPTH_COLORS: [(f32, f32, f32); 6] = [
    (0.72, 0.14, 28.),
    (0.79, 0.13, 78.),
    (0.76, 0.12, 145.),
    (0.74, 0.12, 210.),
    (0.75, 0.12, 285.),
    (0.78, 0.12, 335.),
];

/// One fill per two-space indent step, tinted by its depth.
fn indent_marks(text: &str) -> Vec<RangeDecoration> {
    let mut marks = Vec::new();
    let mut line_start = 0;
    for line in text.split('\n') {
        // `text.search(/\S/)`: lines without content or indent get no tint.
        if let Some(indent) = line.find(|c: char| !c.is_whitespace()) {
            for (depth, start) in (0..indent).step_by(2).enumerate() {
                let (l, c, h) = DEPTH_COLORS[depth % DEPTH_COLORS.len()];
                let end = (start + 2).min(indent);
                marks.push(
                    RangeDecoration::new(line_start + start..line_start + end)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(oklch(l, c, h, 0.11)),
                );
            }
        }
        line_start += line.len() + 1;
    }
    marks
}

pub(super) struct YamlMarks {
    collection: RangeDecorationCollection,
    _changed: Subscription,
}

impl YamlMarks {
    pub fn new(editor: &Entity<EditorState>, cx: &mut Context<Inspector>) -> Self {
        let collection = editor.update(cx, |state, cx| {
            let marks = indent_marks(&state.value());
            state.create_range_decorations_collection(marks, cx)
        });
        let marks = collection.clone();
        // CodeMirror rebuilds the decorations on every document change.
        let _changed = cx.subscribe(editor, move |_, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = editor.read(cx).value();
                marks.set(indent_marks(&text), cx);
            }
        });
        Self {
            collection,
            _changed,
        }
    }

    /// `set_value` replaces the document without a change event.
    pub fn refresh(&self, editor: &Entity<EditorState>, cx: &mut App) {
        let text = editor.read(cx).value();
        self.collection.set(indent_marks(&text), cx);
    }
}
