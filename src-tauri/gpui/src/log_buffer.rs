//! The retained log lines, ported from `appendParsedLogBatch` and the
//! filters in `ResourceLogsPane.svelte`. Lines live in arrival order in a ring
//! buffer, so eviction and lookup by arrival index are constant time, and the
//! level counts update with each append instead of rescanning every line.
//! (A `logs/` directory would match `src-tauri/.gitignore`, so this module
//! sits beside `logs.rs`.)

use crate::logs::{MAX_RETAINED_LOG_LINES, ParsedLogLine, parse_line};
use kubecove_lib::models::LogLineSource;
use std::collections::VecDeque;

#[derive(Default)]
pub struct LogBuffer {
    lines: VecDeque<ParsedLogLine>,
    /// The arrival index the next line gets; clearing does not reset it, as
    /// `nextLogLineIndex` survives `resetLogLines` in the web.
    next_index: u64,
    /// `levelCounts`, indexed by `LogLevel::index`.
    counts: [usize; 4],
}

impl LogBuffer {
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The arrival index of the oldest retained line.
    pub fn first_index(&self) -> u64 {
        self.lines
            .front()
            .map_or(self.next_index, |line| line.index)
    }

    pub fn counts(&self) -> [usize; 4] {
        self.counts
    }

    /// `hasLevels`: some retained line has a detected level.
    pub fn has_levels(&self) -> bool {
        self.counts.iter().any(|count| *count > 0)
    }

    /// The retained line with this arrival index.
    pub fn line(&self, index: u64) -> Option<&ParsedLogLine> {
        let offset = index.checked_sub(self.first_index())?;
        self.lines.get(usize::try_from(offset).ok()?)
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.counts = [0; 4];
    }

    /// Parses and retains a batch, evicting the oldest lines past
    /// `MAX_RETAINED_LOG_LINES`. Lines a batch drops before parsing still
    /// consume arrival indices. Returns the number of lines that arrived.
    pub fn append(&mut self, batch: Vec<(String, Option<LogLineSource>)>) -> usize {
        let arrived = batch.len();
        let skipped = arrived.saturating_sub(MAX_RETAINED_LOG_LINES);
        self.next_index += skipped as u64;
        for (line, source) in batch.into_iter().skip(skipped) {
            if self.lines.len() == MAX_RETAINED_LOG_LINES
                && let Some(evicted) = self.lines.pop_front()
                && let Some(level) = evicted.structure.level
            {
                self.counts[level.index()] -= 1;
            }
            let parsed = parse_line(&line, source.as_ref(), self.next_index);
            self.next_index += 1;
            if let Some(level) = parsed.structure.level {
                self.counts[level.index()] += 1;
            }
            self.lines.push_back(parsed);
        }
        arrived
    }

    /// `visibleLogLines` as arrival indices into `out`: text matches, minus
    /// hidden levels. Unleveled lines stay visible, as in the web.
    pub fn visible(&self, term: &str, hidden: [bool; 4], latest_first: bool, out: &mut Vec<u64>) {
        out.clear();
        let shown = |line: &&ParsedLogLine| {
            (term.is_empty() || line.search.contains(term))
                && line
                    .structure
                    .level
                    .is_none_or(|level| !hidden[level.index()])
        };
        if latest_first {
            out.extend(self.lines.iter().rev().filter(shown).map(|line| line.index));
        } else {
            out.extend(self.lines.iter().filter(shown).map(|line| line.index));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Failure modes: eviction must keep level counts and index lookups in
    // step with the retained lines, and an oversized batch must keep only its
    // newest lines.

    fn lines(range: std::ops::Range<usize>, text: &str) -> Vec<(String, Option<LogLineSource>)> {
        range.map(|n| (format!("{text} {n}"), None)).collect()
    }

    #[test]
    fn eviction_keeps_counts_and_lookup_aligned() {
        let mut buffer = LogBuffer::default();
        buffer.append(lines(0..MAX_RETAINED_LOG_LINES, "ERROR boom"));
        buffer.append(lines(0..10, "plain"));
        assert_eq!(buffer.len(), MAX_RETAINED_LOG_LINES);
        assert_eq!(buffer.counts()[0], MAX_RETAINED_LOG_LINES - 10);
        assert_eq!(buffer.first_index(), 10);
        assert!(buffer.line(9).is_none());
        assert_eq!(buffer.line(10).unwrap().index, 10);
        let last = MAX_RETAINED_LOG_LINES as u64 + 9;
        assert_eq!(
            buffer.line(last).unwrap().structure.headline.as_ref(),
            "plain 9"
        );
    }

    #[test]
    fn oversized_batch_keeps_newest_lines() {
        let mut buffer = LogBuffer::default();
        let arrived = buffer.append(lines(0..MAX_RETAINED_LOG_LINES + 5, "line"));
        assert_eq!(arrived, MAX_RETAINED_LOG_LINES + 5);
        assert_eq!(buffer.first_index(), 5);
        assert_eq!(
            buffer.line(5).unwrap().structure.headline.as_ref(),
            "line 5"
        );
    }
}
