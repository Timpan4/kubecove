//! Table order under a non-default sort, as `sortedResourceRows` and
//! `valueForSort` in `src/features/resources/resourceTableModel.ts` order it.
//! Each visible row gets one packed key: the sorted column's value in the high
//! bits and the row's tie rank, by name and then context rank, in the low 32.
//! One unstable sort of plain integers orders the rows, and the low bits name
//! each row.

use super::{PreparedRows, Visible, order::Lanes};
use crate::collate::Key;
use crate::table_sort::{Column, Sort, Usage, health_state};
use kubecove_lib::models::ResourceSummary;
use std::collections::HashMap;

/// Per-row sort values fixed at preparation. They read only name, namespace,
/// kind and context, which a watch patch never changes in place: a patch that
/// adds, deletes or renames a row prepares every row again
/// (`PreparedRows::rebuild`).
#[derive(Default)]
pub(super) struct SortColumns {
    /// Rank of the name's numeric collation primary; equal primaries share one.
    name: Vec<u32>,
    /// Unique rank by full non-numeric name collation, then context rank, then
    /// row index: the web's `nameCollator` and context-rank tie-breaks, then
    /// its stable sort. A watch patch appends new rows, so row index alone
    /// would put a new row after a same-named row of a later context.
    ties: Vec<u32>,
    /// The row at each tie rank.
    by_tie: Vec<u32>,
    namespace: Vec<u32>,
    /// Collation rank of each prepared kind.
    kind: Vec<u32>,
    /// Collation rank of each prepared context name.
    context: Vec<u32>,
}

fn index(value: usize) -> u32 {
    u32::try_from(value).expect("row count fits u32")
}

/// Ranks `keys` by collation primary. Equal primaries share a rank.
fn base_ranks(keys: &[Key<'_>]) -> Vec<u32> {
    let mut sorted: Vec<usize> = (0..keys.len()).collect();
    sorted.sort_unstable_by(|&a, &b| keys[a].base_cmp(&keys[b]));
    let mut ranks = vec![0; keys.len()];
    let mut rank = 0;
    for pair in 0..sorted.len() {
        if pair > 0 && keys[sorted[pair - 1]].base_cmp(&keys[sorted[pair]]).is_ne() {
            rank += 1;
        }
        ranks[sorted[pair]] = rank;
    }
    ranks
}

impl SortColumns {
    /// `by_name` lists the rows sorted by full numeric name collation.
    /// `context` holds each row's rank in `contexts`.
    pub(super) fn new(
        rows: &[ResourceSummary],
        names: &[Key<'_>],
        by_name: &[usize],
        kinds: &[(String, usize)],
        context: &[u32],
        contexts: &[String],
    ) -> Self {
        let mut name = vec![0; rows.len()];
        let mut rank = 0;
        for pair in 0..by_name.len() {
            if pair > 0
                && names[by_name[pair - 1]]
                    .base_cmp(&names[by_name[pair]])
                    .is_ne()
            {
                rank += 1;
            }
            name[by_name[pair]] = rank;
        }
        let mut by_tie: Vec<u32> = (0..index(rows.len())).collect();
        {
            let plain: Vec<Key<'_>> = rows.iter().map(|row| Key::new(&row.name, false)).collect();
            // Stable, so equal names in one context keep row order.
            by_tie.sort_by(|&a, &b| {
                let (a, b) = (a as usize, b as usize);
                plain[a]
                    .full_cmp(&plain[b])
                    .then_with(|| context[a].cmp(&context[b]))
            });
        }
        let mut ties = vec![0; rows.len()];
        for (rank, &row) in by_tie.iter().enumerate() {
            ties[row as usize] = index(rank);
        }
        let mut namespace_ids: HashMap<&str, usize> = HashMap::new();
        let mut namespace_keys = Vec::new();
        let namespace: Vec<usize> = rows
            .iter()
            .map(|row| {
                let text = row.namespace.as_deref().unwrap_or("");
                *namespace_ids.entry(text).or_insert_with(|| {
                    namespace_keys.push(Key::new(text, true));
                    namespace_keys.len() - 1
                })
            })
            .collect();
        let namespace_ranks = base_ranks(&namespace_keys);
        let kind_keys: Vec<Key<'_>> = kinds.iter().map(|(kind, _)| Key::new(kind, true)).collect();
        let context_keys: Vec<Key<'_>> = contexts
            .iter()
            .map(|context| Key::new(context, true))
            .collect();
        Self {
            name,
            ties,
            by_tie,
            namespace: namespace
                .into_iter()
                .map(|id| namespace_ranks[id])
                .collect(),
            kind: base_ranks(&kind_keys),
            context: base_ranks(&context_keys),
        }
    }
}

/// `value.total_cmp` order as an unsigned integer.
fn number(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 == 1 {
        !bits
    } else {
        bits | 1 << 63
    }
}

/// Collation ranks of text values that a patch can change, ranked when sorted.
/// Each distinct value is collated once.
fn text_ranks<'a>(values: impl Iterator<Item = &'a str>) -> Vec<u64> {
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut distinct = Vec::new();
    let values: Vec<usize> = values
        .map(|value| {
            *ids.entry(value).or_insert_with(|| {
                distinct.push(Key::new(value, true));
                distinct.len() - 1
            })
        })
        .collect();
    let ranks = base_ranks(&distinct);
    values.into_iter().map(|id| u64::from(ranks[id])).collect()
}

/// A Kubernetes timestamp, `YYYY-MM-DDTHH:MM:SSZ`, as its digits. Numeric
/// collation compares each digit run by value, so for this fixed layout the
/// digits in order compare as collation does.
fn timestamp(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    let layout = b"dddd-dd-ddTdd:dd:ddZ";
    if bytes.len() != layout.len() {
        return None;
    }
    let mut digits = 0u64;
    for (&byte, &expected) in bytes.iter().zip(layout) {
        if expected == b'd' {
            if !byte.is_ascii_digit() {
                return None;
            }
            digits = digits * 10 + u64::from(byte - b'0');
        } else if byte != expected {
            return None;
        }
    }
    Some(digits)
}

fn age(row: &ResourceSummary) -> &str {
    row.created_at.as_deref().unwrap_or(&row.age)
}

/// The sorted column's value for each row in `rows`, ordered as the column sorts.
fn values(
    source: &PreparedRows,
    rows: &[usize],
    column: Column,
    usage: impl Fn(&ResourceSummary) -> Usage,
) -> Vec<u64> {
    let columns = &source.sort;
    let row = |index: usize| &source.rows[index];
    match column {
        Column::Name => rows.iter().map(|&i| u64::from(columns.name[i])).collect(),
        Column::Namespace => rows
            .iter()
            .map(|&i| u64::from(columns.namespace[i]))
            .collect(),
        Column::Kind => rows
            .iter()
            .map(|&i| u64::from(columns.kind[source.keys[i].kind]))
            .collect(),
        Column::Status => text_ranks(rows.iter().map(|&i| health_state(row(i)))),
        Column::Ready => text_ranks(rows.iter().map(|&i| row(i).ready.as_deref().unwrap_or(""))),
        Column::Restarts => rows
            .iter()
            .map(|&i| number(row(i).restarts.map_or(-1., f64::from)))
            .collect(),
        Column::Cpu => rows
            .iter()
            .map(|&i| number(usage(row(i)).cpu_millicores.unwrap_or(-1.)))
            .collect(),
        Column::Memory => rows
            .iter()
            .map(|&i| number(usage(row(i)).memory_bytes.map_or(-1., |bytes| bytes as f64)))
            .collect(),
        Column::Age => rows
            .iter()
            .map(|&i| timestamp(age(row(i))))
            .collect::<Option<Vec<u64>>>()
            .unwrap_or_else(|| text_ranks(rows.iter().map(|&i| age(row(i))))),
        Column::Context => rows
            .iter()
            .map(|&i| u64::from(columns.context[source.context[i] as usize]))
            .collect(),
    }
}

impl Visible {
    /// The visible rows in display order under `sort`. Ties keep ascending
    /// name order, as `nameCollator` breaks them on the web, even when the
    /// sort is descending, then context rank.
    pub(crate) fn sorted_order(
        &self,
        sort: Sort,
        usage: impl Fn(&ResourceSummary) -> Usage,
    ) -> Vec<usize> {
        let _span = crate::frame_profile::scope("table", "sort");
        let source = &*self.source;
        let ties = &source.sort.ties;
        let mut keys: Vec<u128> = values(source, &self.indices, sort.column, usage)
            .into_iter()
            .zip(&self.indices)
            .map(|(value, &row)| {
                let value = if sort.desc { !value } else { value };
                u128::from(value) << 32 | u128::from(ties[row])
            })
            .collect();
        keys.sort_unstable();
        let by_tie = &source.sort.by_tie;
        let sequence: Vec<usize> = keys
            .iter()
            .map(|&key| by_tie[(key as u32) as usize] as usize)
            .collect();
        let mut order = Vec::with_capacity(sequence.len());
        super::order::Grouping::default().order(
            &Lanes {
                source,
                grouped: self.grouped_by_gitops,
                preferred: &self.preferred,
                lane_order: &self.lane_order,
            },
            &sequence,
            &mut order,
        );
        order
    }
}
