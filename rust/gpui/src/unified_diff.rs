//! Line diffs for server results, ported from
//! `src/features/resource-detail/yamlTabDiff.ts` (jsdiff `structuredPatch`
//! with three context lines).

use gpui_kit::SharedString;
use similar::{ChangeTag, TextDiff};

const CONTEXT_LINES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Header,
    Hunk,
    Add,
    Remove,
    Context,
    Empty,
    Meta,
}

impl DiffLineKind {
    /// An added or removed line, as a diff's change counts tally them.
    pub fn changed(self) -> bool {
        matches!(self, DiffLineKind::Add | DiffLineKind::Remove)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: SharedString,
}

fn line(kind: DiffLineKind, text: impl Into<SharedString>) -> DiffLine {
    DiffLine {
        kind,
        text: text.into(),
    }
}

struct Hunk {
    header: String,
    lines: Vec<DiffLine>,
}

/// GNU unified format names an empty range by the line before it, so it prints
/// its 0-based start. Other ranges print their 1-based first line.
fn hunk_range(range: std::ops::Range<usize>) -> String {
    let start = if range.is_empty() {
        range.start
    } else {
        range.start + 1
    };
    format!("{start},{}", range.len())
}

fn hunks(diff: &TextDiff<'_, '_, str>, context: usize) -> Vec<Hunk> {
    diff.grouped_ops(context)
        .iter()
        .map(|group| {
            let (first, last) = (&group[0], &group[group.len() - 1]);
            let old_range = first.old_range().start..last.old_range().end;
            let new_range = first.new_range().start..last.new_range().end;
            let header = format!(
                "@@ -{} +{} @@",
                hunk_range(old_range),
                hunk_range(new_range)
            );
            let mut lines = Vec::new();
            for op in group {
                for change in diff.iter_changes(op) {
                    let (kind, sign) = match change.tag() {
                        ChangeTag::Insert => (DiffLineKind::Add, '+'),
                        ChangeTag::Delete => (DiffLineKind::Remove, '-'),
                        ChangeTag::Equal => (DiffLineKind::Context, ' '),
                    };
                    let value = change.value();
                    let text = value.strip_suffix('\n').unwrap_or(value);
                    lines.push(line(kind, format!("{sign}{}", text.trim_end_matches('\r'))));
                    if change.missing_newline() {
                        lines.push(line(DiffLineKind::Empty, "\\ No newline at end of file"));
                    }
                }
            }
            Hunk { header, lines }
        })
        .collect()
}

/// `buildUnifiedDiff` with its default `current` and `dry-run` labels.
fn unified(old: &str, new: &str, context: usize, labels: (&str, &str, &str)) -> Vec<DiffLine> {
    unified_hunks(hunks(&TextDiff::from_lines(old, new), context), labels)
}

fn unified_hunks(hunks: Vec<Hunk>, labels: (&str, &str, &str)) -> Vec<DiffLine> {
    let (old_name, new_name, empty) = labels;
    let mut lines = vec![
        line(DiffLineKind::Header, format!("--- {old_name}")),
        line(DiffLineKind::Header, format!("+++ {new_name}")),
    ];
    if hunks.is_empty() {
        lines.push(line(DiffLineKind::Empty, empty.to_owned()));
    }
    for hunk in hunks {
        lines.push(line(DiffLineKind::Hunk, hunk.header));
        lines.extend(hunk.lines);
    }
    lines
}

/// `buildCompactUnifiedDiff`, used for revision comparisons and action reviews.
pub fn compact(current: &str, dry_run: &str) -> Vec<DiffLine> {
    unified(
        current,
        dry_run,
        CONTEXT_LINES,
        ("current", "dry-run", "No server-side dry-run changes."),
    )
}

/// `buildArgoResourceDiff`: an Argo CD target document against its live state.
pub fn argo(target: &str, live: &str) -> Vec<DiffLine> {
    unified(
        target,
        live,
        CONTEXT_LINES,
        ("target", "live", "Target and live state match."),
    )
}

/// `buildYamlDryRunDiff`, rendered in either its clean or Git style.
fn dry_run_lines(
    diff: &TextDiff<'_, '_, str>,
    context: usize,
    git: bool,
    force_conflicts: bool,
) -> Vec<DiffLine> {
    let hunks = hunks(diff, context);
    if git {
        return unified_hunks(
            hunks,
            ("current", "dry-run", "No server-side dry-run changes."),
        );
    }
    let mut lines: Vec<DiffLine> = Vec::new();
    if hunks.is_empty() {
        lines.push(line(DiffLineKind::Empty, "No server-side dry-run changes."));
    }
    for hunk in hunks {
        lines.extend(hunk.lines);
    }
    lines.push(line(DiffLineKind::Meta, "  apply gate: dry-run succeeded"));
    lines.push(line(
        DiffLineKind::Meta,
        format!("  force-conflicts: {force_conflicts}"),
    ));
    lines
}

pub(crate) fn dry_run_pair(
    current: &str,
    dry_run: &str,
    git: bool,
    force_conflicts: bool,
) -> (Vec<DiffLine>, Vec<DiffLine>) {
    let diff = TextDiff::from_lines(current, dry_run);
    let full = current
        .split('\n')
        .count()
        .max(dry_run.split('\n').count())
        .max(CONTEXT_LINES);
    (
        dry_run_lines(&diff, CONTEXT_LINES, git, force_conflicts),
        dry_run_lines(&diff, full, git, force_conflicts),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Failure modes: a missing hunk header breaks line numbers in review, an
    // identical pair must say so instead of rendering an empty pane, and a
    // missing final newline must be visible.

    #[test]
    fn hunks_carry_both_ranges() {
        let lines = compact("a\nb\nc\n", "a\nB\nc\n");
        let texts: Vec<&str> = lines.iter().map(|line| line.text.as_ref()).collect();
        assert_eq!(
            texts,
            [
                "--- current",
                "+++ dry-run",
                "@@ -1,3 +1,3 @@",
                " a",
                "-b",
                "+B",
                " c"
            ]
        );
    }

    #[test]
    fn identical_input_reports_no_changes() {
        let lines = compact("a\n", "a\n");
        assert_eq!(lines[2].text.as_ref(), "No server-side dry-run changes.");
        let (compact, full) = dry_run_pair("a\n", "a\n", false, false);
        assert_eq!(compact[0].text.as_ref(), "No server-side dry-run changes.");
        assert_eq!(compact[2].text.as_ref(), "  force-conflicts: false");
        assert_eq!(compact, full);
    }

    // Hunk ranges can break in these ways: an empty old side (empty file) or
    // an empty new side prints start+1 instead of the line before the range;
    // both sides empty must still produce no hunk; insertions at the start or
    // end of a non-empty file must keep their 1-based starts. With three
    // context lines an empty side only occurs for an empty file, so its start
    // is 0. Context 0 is the only way to reach an empty range that starts
    // after line 0.

    fn headers(old: &str, new: &str, context: usize) -> Vec<String> {
        hunks(&TextDiff::from_lines(old, new), context)
            .into_iter()
            .map(|hunk| hunk.header)
            .collect()
    }

    #[test]
    fn empty_ranges_name_the_line_before_them() {
        assert_eq!(headers("", "a\nb\n", CONTEXT_LINES), ["@@ -0,0 +1,2 @@"]);
        assert_eq!(headers("a\nb\n", "", CONTEXT_LINES), ["@@ -1,2 +0,0 @@"]);
        assert_eq!(headers("a\nb\n", "a\nx\nb\n", 0), ["@@ -1,0 +2,1 @@"]);
        assert_eq!(headers("a\nx\nb\n", "a\nb\n", 0), ["@@ -2,1 +1,0 @@"]);
        assert_eq!(headers("a\n", "a\nb\n", 0), ["@@ -1,0 +2,1 @@"]);
        assert_eq!(headers("b\n", "a\nb\n", 0), ["@@ -0,0 +1,1 @@"]);
    }

    #[test]
    fn edits_at_either_end_keep_one_based_starts() {
        assert_eq!(
            headers("b\nc\n", "a\nb\nc\n", CONTEXT_LINES),
            ["@@ -1,2 +1,3 @@"]
        );
        assert_eq!(
            headers("a\nb\n", "a\nb\nc\n", CONTEXT_LINES),
            ["@@ -1,2 +1,3 @@"]
        );
        assert!(headers("", "", CONTEXT_LINES).is_empty());
    }

    #[test]
    fn missing_final_newline_is_marked() {
        let lines = compact("a\n", "a\nb");
        assert_eq!(lines.last().unwrap().kind, DiffLineKind::Empty);
    }
}
