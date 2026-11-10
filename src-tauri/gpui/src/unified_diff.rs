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

fn hunks(old: &str, new: &str, context: usize) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old, new);
    diff.grouped_ops(context)
        .iter()
        .map(|group| {
            let (first, last) = (&group[0], &group[group.len() - 1]);
            let old_range = first.old_range().start..last.old_range().end;
            let new_range = first.new_range().start..last.new_range().end;
            let header = format!(
                "@@ -{},{} +{},{} @@",
                old_range.start + 1,
                old_range.len(),
                new_range.start + 1,
                new_range.len()
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
    let (old_name, new_name, empty) = labels;
    let mut lines = vec![
        line(DiffLineKind::Header, format!("--- {old_name}")),
        line(DiffLineKind::Header, format!("+++ {new_name}")),
    ];
    let hunks = hunks(old, new, context);
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

fn context(current: &str, dry_run: &str, full: bool) -> usize {
    if full {
        current
            .split('\n')
            .count()
            .max(dry_run.split('\n').count())
            .max(CONTEXT_LINES)
    } else {
        CONTEXT_LINES
    }
}

/// `buildYamlDryRunDiff` with the default `clean` diff style.
pub fn dry_run(current: &str, dry_run: &str, full: bool, force_conflicts: bool) -> Vec<DiffLine> {
    let hunks = hunks(current, dry_run, context(current, dry_run, full));
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

pub fn dry_run_git(current: &str, dry_run: &str, full: bool) -> Vec<DiffLine> {
    unified(
        current,
        dry_run,
        context(current, dry_run, full),
        ("current", "dry-run", "No server-side dry-run changes."),
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
        let clean = dry_run("a\n", "a\n", false, false);
        assert_eq!(clean[0].text.as_ref(), "No server-side dry-run changes.");
        assert_eq!(clean[2].text.as_ref(), "  force-conflicts: false");
    }

    #[test]
    fn missing_final_newline_is_marked() {
        let lines = compact("a\n", "a\nb");
        assert_eq!(lines.last().unwrap().kind, DiffLineKind::Empty);
    }
}
