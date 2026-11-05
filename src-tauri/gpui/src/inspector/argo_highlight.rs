//! Parse each YAML diff fragment once, as highlightedArgoDiffSegments does.
use crate::unified_diff::{DiffLine, DiffLineKind};
use gpui_kit::{HighlightStyle, component::highlighter::SyntaxHighlighter};
use std::ops::Range;
pub(super) type Highlights = Vec<(Range<usize>, HighlightStyle)>;

pub(super) fn lines(
    lines: &[DiffLine],
    theme: &gpui_kit::component::highlighter::HighlightTheme,
) -> Vec<Highlights> {
    let mut parser = SyntaxHighlighter::new("yaml");
    lines
        .iter()
        .map(|line| {
            if !matches!(
                line.kind,
                DiffLineKind::Add | DiffLineKind::Remove | DiffLineKind::Context
            ) {
                return Vec::new();
            }
            let source = line.text.get(1..).unwrap_or_default();
            parser.update(None, &source.into(), None);
            parser
                .styles(&(0..source.len()), theme)
                .into_iter()
                .map(|(r, style)| ((r.start + 1)..(r.end + 1), style))
                .collect()
        })
        .collect()
}
