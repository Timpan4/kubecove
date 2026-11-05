//! Inline YAML diagnostics. The editor marks every parser and Kubernetes lint
//! finding, and the lens shows one label per line, as `errorLensField` in
//! `YamlCodeEditor.svelte` does. Parser errors come first, then lint results.
//! `info` findings are marked but never get a lens.
use super::{Inspector, detail::Detail};
use crate::{theme::Tokens, ui};
use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
use gpui_kit::component::input::{EditorState, Position};
use gpui_kit::{Context, Div, TestSupportExt, div, prelude::*, px};
use kubecove_lib::models::{KubernetesYamlLintDiagnostic, KubernetesYamlLintSeverity};

pub(super) struct Lens {
    /// One-based line the label sits on.
    pub line: usize,
    /// `source: message`, as the web lens reads.
    pub label: String,
    severity: KubernetesYamlLintSeverity,
    /// Byte offset of the line end, where the lens anchors.
    end: usize,
}

/// One finding in byte offsets of the draft.
struct Mark {
    from: usize,
    to: usize,
    severity: KubernetesYamlLintSeverity,
    source: String,
    message: String,
}

/// Marks every finding in the editor and returns the lenses to show. The
/// marks stay when lenses are off, as the web linter keeps its underlines.
pub(super) fn update(
    editor: &mut EditorState,
    enabled: bool,
    lint: &[KubernetesYamlLintDiagnostic],
    cx: &mut Context<EditorState>,
) -> Vec<Lens> {
    let text = editor.value().to_string();
    let mut marks: Vec<Mark> = parser_error(&text).into_iter().collect();
    marks.extend(lint.iter().map(|diagnostic| {
        let (from, to) = field_range(&text, diagnostic.field_path.as_deref());
        Mark {
            from,
            to,
            severity: diagnostic.severity.clone(),
            source: diagnostic.source.clone(),
            message: diagnostic.message.clone(),
        }
    }));
    if let Some(diagnostics) = editor.diagnostics_mut() {
        diagnostics.clear();
        for mark in &marks {
            diagnostics.push(
                Diagnostic::new(
                    position(&text, mark.from)..position(&text, mark.to),
                    mark.message.clone(),
                )
                .with_severity(severity(&mark.severity))
                .with_source(mark.source.clone()),
            );
        }
    }
    cx.notify();
    if enabled {
        lenses(&text, &marks)
    } else {
        Vec::new()
    }
}

/// The first serde_yaml error, on the one character at its column.
fn parser_error(text: &str) -> Option<Mark> {
    let error = serde_yaml::from_str::<serde_yaml::Value>(text).err()?;
    let location = error.location();
    let line = location.as_ref().map_or(1, |l| l.line());
    let column = location.as_ref().map_or(1, |l| l.column());
    let start = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let content = text[start..].split('\n').next().unwrap_or("");
    // serde_yaml reports character columns.
    let from = start
        + content
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(content.len(), |(index, _)| index);
    let to = from
        + text[from..]
            .chars()
            .next()
            .filter(|c| *c != '\n')
            .map_or(0, char::len_utf8);
    Some(Mark {
        from,
        to,
        severity: KubernetesYamlLintSeverity::Error,
        source: "YAML".into(),
        message: error.to_string(),
    })
}

/// `findYamlFieldRange`: the first line that declares the field's last key,
/// from the key to its colon. Without a match, the first character.
fn field_range(text: &str, field_path: Option<&str>) -> (usize, usize) {
    let fallback = (0, text.chars().next().map_or(0, char::len_utf8));
    let Some(key) = field_path
        .and_then(|path| path.rsplit('.').next())
        .filter(|key| !key.is_empty())
    else {
        return fallback;
    };
    let quoted = format!("\"{key}\"");
    let mut offset = 0;
    for line in text.split('\n') {
        for (index, _) in line.char_indices() {
            let preceded = index == 0 || line[..index].ends_with(char::is_whitespace);
            let rest = &line[index..];
            let name_len = if !preceded {
                continue;
            } else if rest.starts_with(&quoted) {
                quoted.len()
            } else if rest.starts_with(key) {
                key.len()
            } else {
                continue;
            };
            let after = &rest[name_len..];
            let trimmed = after.trim_start_matches(char::is_whitespace);
            if trimmed.starts_with(':') {
                let colon = offset + index + name_len + (after.len() - trimmed.len());
                return (offset + index, colon + 1);
            }
        }
        offset += line.len() + 1;
    }
    fallback
}

/// The editor position of a byte offset: line and UTF-16 column.
fn position(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = before[line_start..].encode_utf16().count();
    Position::new(line as u32, column as u32)
}

fn severity(severity: &KubernetesYamlLintSeverity) -> DiagnosticSeverity {
    match severity {
        KubernetesYamlLintSeverity::Error => DiagnosticSeverity::Error,
        KubernetesYamlLintSeverity::Warning => DiagnosticSeverity::Warning,
        KubernetesYamlLintSeverity::Info => DiagnosticSeverity::Info,
    }
}

/// `forEachYamlErrorLensDiagnostic`: the first non-info finding on each line,
/// in line order.
fn lenses(text: &str, marks: &[Mark]) -> Vec<Lens> {
    let mut chosen: Vec<(usize, &Mark)> = Vec::new();
    for mark in marks {
        if mark.severity == KubernetesYamlLintSeverity::Info {
            continue;
        }
        let end = text[mark.from..]
            .find('\n')
            .map_or(text.len(), |index| mark.from + index);
        if chosen.iter().any(|(line_end, _)| *line_end == end) {
            continue;
        }
        chosen.push((end, mark));
    }
    chosen.sort_by_key(|(end, _)| *end);
    chosen
        .into_iter()
        .map(|(end, mark)| Lens {
            line: text[..mark.from].matches('\n').count() + 1,
            label: format!("{}: {}", mark.source, mark.message),
            severity: mark.severity.clone(),
            end,
        })
        .collect()
}

pub(super) fn overlay(editor: &EditorState, lens: &Lens) -> Div {
    let input = editor.input_bounds();
    let bounds = editor.range_to_bounds(&(lens.end..lens.end));
    let mut view = div();
    if let Some(bounds) = bounds {
        let error = lens.severity == KubernetesYamlLintSeverity::Error;
        // Native lens sits after the faulty line, without changing the document.
        view = view.child(
            ui::xs(lens.label.clone())
                .id("yaml-lens")
                .test_support()
                .aria_label(format!(
                    "YAML {} at line {}",
                    if error { "error" } else { "warning" },
                    lens.line
                ))
                .aria_description(lens.label.clone())
                .absolute()
                .top(bounds.origin.y - input.origin.y)
                .left(bounds.right() - input.origin.x + px(8.))
                .text_color(if error {
                    Tokens::destructive()
                } else {
                    Tokens::muted_foreground()
                })
                .bg(Tokens::card()),
        );
    }
    view
}

impl Inspector {
    pub(super) fn refresh_yaml_lens(&mut self, cx: &mut Context<Self>) {
        let enabled = self.persisted.preferences.yaml_error_lens_enabled;
        if let Detail::Ready(detail) = &mut self.detail {
            detail.lenses = detail.editor.update(cx, |editor, cx| {
                update(editor, enabled, &detail.lint.diagnostics, cx)
            });
        }
    }
}
