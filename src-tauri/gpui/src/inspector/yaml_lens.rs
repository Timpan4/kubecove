//! Syntax diagnostics use the editor's byte ranges and layout, including scrolling.
use super::{Inspector, detail::Detail};
use crate::{theme::Tokens, ui};
use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::{EditorState, Position};
use gpui_kit::{Context, Div, TestSupportExt, div, prelude::*, px};

pub(super) struct Lens {
    pub line: usize,
    pub message: String,
    end: usize,
}

pub(super) fn update(
    editor: &mut EditorState,
    enabled: bool,
    cx: &mut Context<EditorState>,
) -> Option<Lens> {
    if let Some(diagnostics) = editor.diagnostics_mut() {
        diagnostics.clear();
    }
    if !enabled {
        cx.notify();
        return None;
    }
    let text = editor.value();
    let error = serde_yaml::from_str::<serde_yaml::Value>(&text).err()?;
    let location = error.location();
    let line = location.as_ref().map_or(1, |l| l.line());
    let column = location.as_ref().map_or(1, |l| l.column());
    let start = text
        .split_inclusive('\n')
        .take(line - 1)
        .map(str::len)
        .sum::<usize>();
    let content = text[start..].split('\n').next().unwrap_or("");
    let end = start + content.len();
    // yaml_serde reports character columns; GPUI diagnostics use UTF-16 positions.
    let utf16 = content
        .chars()
        .take(column - 1)
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    let message = error.to_string();
    if let Some(diagnostics) = editor.diagnostics_mut() {
        diagnostics.push(
            Diagnostic::new(
                Position::new((line - 1) as u32, utf16)
                    ..Position::new((line - 1) as u32, utf16 + 1),
                message.clone(),
            )
            .with_severity(DiagnosticSeverity::Error)
            .with_source("YAML"),
        );
    }
    cx.notify();
    Some(Lens { line, message, end })
}

pub(super) fn overlay(editor: &EditorState, lens: &Lens) -> Div {
    let input = editor.input_bounds();
    let bounds = editor.range_to_bounds(&(lens.end..lens.end));
    let mut view = div();
    if let Some(bounds) = bounds {
        // Native lens sits after the faulty line, without changing the document.
        view = view.child(
            ui::xs(format!("YAML: {}", lens.message))
                .id("yaml-lens")
                .test_support()
                .aria_label(format!("YAML error at line {}", lens.line))
                .aria_description(lens.message.clone())
                .absolute()
                .top(bounds.origin.y - input.origin.y)
                .left(bounds.right() - input.origin.x + px(8.))
                .text_color(Tokens::destructive())
                .bg(Tokens::card()),
        );
    }
    view
}

impl Inspector {
    pub(super) fn refresh_yaml_lens(&mut self, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            detail.lens = detail.editor.update(cx, |editor, cx| {
                update(
                    editor,
                    self.persisted.preferences.yaml_error_lens_enabled,
                    cx,
                )
            });
        }
    }
}
