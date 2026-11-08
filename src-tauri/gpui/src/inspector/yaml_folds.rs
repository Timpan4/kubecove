//! The YAML editor's highlighter. It highlights with GPUI Kit's tree-sitter
//! `SyntaxHighlighter` and folds whole blocks, as the web editor's
//! `@codemirror/lang-yaml` fold service does.
//!
//! GPUI Kit's own adapter ends a fold on the node's last row, and its editor
//! keeps the first and last line of a fold visible, so a folded block kept its
//! last line on screen. A fold here ends on the line after the block, or one
//! past the last line for a block at the end of the document.

use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope, RopeExt as _,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Task, Window};
use std::{cell::RefCell, cmp::Reverse, ops::Range, rc::Rc, time::Duration};
use tree_sitter::{Node, Point, Tree};

// The parse budget of GPUI Kit 0.9.0's tree-sitter input adapter
// (`gpui-component/src/highlighter/input_adapter.rs`), so a large document
// still parses off the UI thread.
const SYNC_PARSE_TIMEOUT: Duration = Duration::from_millis(2);
const SYNC_PARSE_MAX_BYTES: usize = 256 * 1024;
const PARSE_DEBOUNCE: Duration = Duration::from_millis(150);

const LANGUAGE: &str = "yaml";

pub(super) fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        (language == LANGUAGE).then(|| {
            Box::new(YamlHighlighter {
                inner: Rc::new(RefCell::new(SyntaxHighlighter::new(LANGUAGE))),
                parse_task: None,
            }) as Box<dyn InputHighlighter>
        })
    })
}

struct YamlHighlighter {
    inner: Rc<RefCell<SyntaxHighlighter>>,
    parse_task: Option<Task<()>>,
}

impl YamlHighlighter {
    /// Drops a pending background parse when the synchronous one `completed`,
    /// or replaces it with a parse of `text`, as GPUI Kit's adapter does.
    fn finish_update(
        &mut self,
        completed: bool,
        text: &Rope,
        folding: bool,
        window: &mut Window,
        cx: &mut Context<EditorState>,
    ) {
        if completed {
            self.parse_task = None;
            return;
        }
        let inner = self.inner.clone();
        let text = text.clone();
        self.parse_task = Some(cx.spawn_in(window, async move |editor, cx| {
            cx.background_executor().timer(PARSE_DEBOUNCE).await;
            let (parsed, folds) = cx
                .background_executor()
                .spawn(async move {
                    let mut parsed = SyntaxHighlighter::new(LANGUAGE);
                    parsed.update(None, &text, None);
                    let folds = match parsed.tree() {
                        Some(tree) if folding => fold_ranges(tree, &text, 0..usize::MAX),
                        _ => Vec::new(),
                    };
                    (parsed, folds)
                })
                .await;
            *inner.borrow_mut() = parsed;
            let _ = editor.update(cx, |state, cx| {
                state.apply_highlighter_fold_candidates(folds, cx);
            });
        }));
    }
}

impl InputHighlighter for YamlHighlighter {
    fn language(&self) -> SharedString {
        self.inner.borrow().language().clone()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        folding: bool,
        window: &mut Window,
        cx: &mut Context<EditorState>,
    ) {
        let edit = edit.map(tree_sitter_edit);
        let completed = {
            let mut inner = self.inner.borrow_mut();
            if text.len() > SYNC_PARSE_MAX_BYTES {
                inner.edit_tree(edit, text);
                false
            } else {
                inner.update(edit, text, Some(SYNC_PARSE_TIMEOUT))
            }
        };
        self.finish_update(completed, text, folding, window, cx);
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        self.inner.borrow().styles(range, resolver)
    }

    fn fold_ranges(&self, text: &Rope) -> Vec<FoldRange> {
        self.inner
            .borrow()
            .tree()
            .map(|tree| fold_ranges(tree, text, 0..usize::MAX))
            .unwrap_or_default()
    }

    fn fold_ranges_for_edit(&self, range: Range<usize>, text: &Rope) -> Vec<FoldRange> {
        // The editor drops every fold whose last line reaches the edited
        // lines. A fold here ends on the line after its block, so a block
        // that ends on the line before the edit has to be found again.
        let row = text.offset_to_point(range.start.min(text.len())).row;
        let start = text.line_start_offset(row.saturating_sub(1));
        self.inner
            .borrow()
            .tree()
            .map(|tree| fold_ranges(tree, text, start..range.end))
            .unwrap_or_default()
    }
}

fn tree_sitter_edit(edit: InputEdit) -> tree_sitter::InputEdit {
    tree_sitter::InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: Point::new(edit.start_position.row, edit.start_position.column),
        old_end_position: Point::new(edit.old_end_position.row, edit.old_end_position.column),
        new_end_position: Point::new(edit.new_end_position.row, edit.new_end_position.column),
    }
}

/// One fold per start line for the nodes that touch `bytes`. CodeMirror folds
/// the outermost node that starts on a line, which is the one that ends last.
fn fold_ranges(tree: &Tree, text: &Rope, bytes: Range<usize>) -> Vec<FoldRange> {
    fn collect(node: Node, text: &Rope, bytes: &Range<usize>, ranges: &mut Vec<FoldRange>) {
        if node.end_byte() <= bytes.start || node.start_byte() >= bytes.end {
            return;
        }
        ranges.extend(fold_range(node, text));
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, text, bytes, ranges);
        }
    }

    let mut ranges = Vec::new();
    collect(tree.root_node(), text, &bytes, &mut ranges);
    ranges.sort_by_key(|range| (range.start_line, Reverse(range.end_line)));
    ranges.dedup_by_key(|range| range.start_line);
    ranges
}

/// The fold `@codemirror/lang-yaml` makes for `node`, if any. `Item`, `Pair`
/// and `BlockLiteral` fold from the end of their first line to their end;
/// `FlowMapping` and `FlowSequence` fold inside their brackets. The editor
/// hides lines `start_line + 1 ..= end_line - 1` of a fold.
fn fold_range(node: Node, text: &Rope) -> Option<FoldRange> {
    let start_line = node.start_position().row;
    // Lezer names every entry of a flow collection `Pair` or `Item`.
    let flow_entry = node
        .parent()
        .is_some_and(|parent| matches!(parent.kind(), "flow_mapping" | "flow_sequence"));
    let end_line = match node.kind() {
        _ if flow_entry => last_row(node, text) + 1,
        "block_mapping_pair" | "block_sequence_item" | "block_scalar" => last_row(node, text) + 1,
        "flow_mapping" | "flow_sequence" => {
            let closer = node.children(&mut node.walk()).last()?;
            if closer.is_named() || closer.is_missing() {
                // `foldInside` folds to the node's end when the closer is missing.
                last_row(node, text) + 1
            } else if opens_line(closer, text) {
                closer.start_position().row
            } else {
                // Content before the closer folds into the first line with it.
                closer.start_position().row + 1
            }
        }
        _ => return None,
    };
    (end_line > start_line + 1).then(|| FoldRange::new(start_line, end_line))
}

/// The row of the node's last content. Tree-sitter keeps blank lines and
/// comments after a block inside it, while Lezer ends the block at its last
/// value, also before the blank lines a `|+` block scalar keeps.
fn last_row(node: Node, text: &Rope) -> usize {
    let mut last = node;
    while let Some(child) = content_child(last, text) {
        last = child;
    }
    let token = text.slice(last.start_byte()..last.end_byte());
    let blank: usize = token
        .chars_at(token.len())
        .reversed()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum();
    text.offset_to_point(last.end_byte() - blank).row
}

/// The child that holds the end of the node's content: its last child that is
/// neither a comment nor a parser-inserted missing token. `None` when the node
/// has its own text after its children, as a block scalar has after its `|`.
fn content_child<'tree>(node: Node<'tree>, text: &Rope) -> Option<Node<'tree>> {
    let last = node.child(u32::try_from(node.child_count().checked_sub(1)?).ok()?)?;
    let mut child = last;
    while child.is_extra() || child.is_missing() {
        child = child.prev_sibling()?;
    }
    let tail_is_blank = text
        .slice(last.end_byte()..node.end_byte())
        .chars()
        .all(char::is_whitespace);
    tail_is_blank.then_some(child)
}

/// Whether only indentation precedes `node` on its line.
fn opens_line(node: Node, text: &Rope) -> bool {
    let start = node.start_byte();
    text.slice(start - node.start_position().column..start)
        .chars()
        .all(char::is_whitespace)
}
