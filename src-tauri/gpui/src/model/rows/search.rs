//! Row search text as one buffer and query matches as one bit per row, so a
//! filter over every row is one scan with no allocation per row.

use kubecove_lib::models::ResourceSummary;

/// Every row's lowercase search text, back to back. Row `i` is
/// `text[ends[i - 1]..ends[i]]`, with `0` before the first row.
#[derive(Default)]
pub(super) struct SearchText {
    text: String,
    ends: Vec<usize>,
}

impl SearchText {
    pub(super) fn with_capacity(rows: usize) -> Self {
        Self {
            text: String::new(),
            ends: Vec::with_capacity(rows),
        }
    }

    pub(super) fn push(
        &mut self,
        row: &ResourceSummary,
        owner: Option<crate::model::OwnerRef<'_>>,
    ) {
        crate::resource_search::push_haystack(row, owner, &mut self.text);
        self.ends.push(self.text.len());
    }

    /// Sets the bit of each row whose text contains `query`. A match that runs
    /// past its row's end belongs to no row, so the scan resumes one byte after
    /// its start, where a match inside the next row can begin.
    pub(super) fn matches(&self, query: &str, out: &mut SearchMatches) {
        let words = self.ends.len().div_ceil(64);
        out.0.clear();
        if query.is_empty() {
            out.0.resize(words, u64::MAX);
            return;
        }
        out.0.resize(words, 0);
        let text = self.text.as_bytes();
        let finder = memchr::memmem::Finder::new(query.as_bytes());
        let (mut at, mut row) = (0, 0);
        while let Some(found) = finder.find(&text[at..]) {
            let start = at + found;
            // Matches only move forward, so the row does too.
            while self.ends[row] <= start {
                row += 1;
            }
            let end = self.ends[row];
            if start + query.len() <= end {
                out.0[row / 64] |= 1 << (row % 64);
                at = end;
            } else {
                at = start + 1;
            }
        }
    }
}

/// One bit per row: whether its search text contains the query.
#[derive(Default)]
pub(crate) struct SearchMatches(Vec<u64>);

impl SearchMatches {
    pub(crate) fn contains(&self, row: usize) -> bool {
        self.0[row / 64] >> (row % 64) & 1 == 1
    }
}
