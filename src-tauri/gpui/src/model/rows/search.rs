//! Row search text as one buffer and query matches as one bit per row, so a
//! filter over every row is one scan with no allocation per row. A row's text
//! mirrors `buildResourceSearchIndex` in `src/features/resources/helpers.ts`.
//! Fields are joined with newlines, so a query cannot match across two fields.

use crate::model::OwnerRef;
use kubecove_lib::models::ResourceSummary;

/// The row's lowercase search text.
#[cfg(test)]
pub(super) fn haystack(row: &ResourceSummary) -> String {
    let mut text = String::new();
    push_haystack(row, crate::model::owner_ref(row), None, &mut text);
    text
}

/// Appends the row's lowercase search text to `text`: identity, API identity,
/// owner reference, Helm release and the GitOps owner's terms (`owner_ref` of
/// the row). `context` is the row's context in a view of several contexts, and
/// `None` otherwise, so a one-context view does not match its context name.
/// Each field is lowercased alone, which matches lowercasing the
/// joined text because no case mapping looks across a newline.
fn push_haystack(
    row: &ResourceSummary,
    owner: Option<OwnerRef<'_>>,
    context: Option<&str>,
    text: &mut String,
) {
    let fields = [
        Some(row.name.as_str()),
        Some(row.kind.as_str()),
        row.namespace.as_deref(),
        row.api_version.as_deref(),
        row.group.as_deref(),
        row.plural.as_deref(),
        row.owner_ref.as_deref(),
        row.helm_release.as_deref(),
        owner.map(|owner| owner.provider),
        owner.map(|owner| owner.kind),
        owner.map(|owner| owner.name),
        owner.and_then(|owner| owner.namespace),
        owner.map(|owner| owner.confidence),
        owner.map(|owner| owner.provenance),
        context,
    ];
    let mut first = true;
    for field in fields
        .into_iter()
        .flatten()
        .filter(|field| !field.is_empty())
    {
        if !first {
            text.push('\n');
        }
        first = false;
        if field.is_ascii() {
            let start = text.len();
            text.push_str(field);
            text[start..].make_ascii_lowercase();
        } else {
            text.push_str(&field.to_lowercase());
        }
    }
}

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

    /// `context` is the row's context when the rows span several contexts.
    pub(super) fn push(
        &mut self,
        row: &ResourceSummary,
        owner: Option<OwnerRef<'_>>,
        context: Option<&str>,
    ) {
        push_haystack(row, owner, context, &mut self.text);
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
