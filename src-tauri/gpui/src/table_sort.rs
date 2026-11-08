//! Resource table sort order, mirroring `sortedResourceRows` and
//! `valueForSort` in `src/features/resources/resourceTableModel.ts`.
//! Collation keys are built once per call, and rows are ordered by index.

use crate::collate::Key;
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Name,
    Status,
    Namespace,
    Kind,
    Ready,
    Restarts,
    Cpu,
    Memory,
    Age,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sort {
    pub column: Column,
    pub desc: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            column: Column::Name,
            desc: false,
        }
    }
}

/// Usage numbers for the CPU and Memory sorts. A missing metric sorts first, as `-1` does on the web.
#[derive(Clone, Copy, Default)]
pub struct Usage {
    pub cpu_millicores: Option<f64>,
    pub memory_bytes: Option<i64>,
}

impl Column {
    /// The header label in `ResourceBrowser.svelte`'s `SortButton` calls.
    pub fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Status => "Status",
            Column::Namespace => "Namespace",
            Column::Kind => "Kind",
            Column::Ready => "Ready",
            Column::Restarts => "Restarts",
            Column::Cpu => "CPU",
            Column::Memory => "Memory",
            Column::Age => "Age",
        }
    }

    /// `CPU_USAGE_DESCRIPTION` and `MEMORY_USAGE_DESCRIPTION` in `operational-data.ts`.
    pub fn description(self) -> Option<&'static str> {
        match self {
            Column::Cpu => Some("CPU usage in millicores; 1000m equals one CPU core."),
            Column::Memory => Some("Memory usage in binary units such as Ki, Mi, and Gi."),
            _ => None,
        }
    }
}

impl Sort {
    /// `toggleSort`: the active column flips direction, and a new column sorts ascending.
    pub fn toggled(self, column: Column) -> Self {
        if self.column == column {
            Self {
                column,
                desc: !self.desc,
            }
        } else {
            Self {
                column,
                desc: false,
            }
        }
    }
}

enum Value<'a> {
    Text(Key<'a>),
    Number(f64),
}

fn text(value: &str) -> Value<'_> {
    Value::Text(Key::new(value, true))
}

/// The `state` strings of `HealthAssessmentState` in `src/lib/health-types.ts`.
fn health_state(row: &ResourceSummary) -> &'static str {
    match row.health_assessment.state {
        HealthAssessmentState::Healthy => "healthy",
        HealthAssessmentState::NeedsAttention => "needsAttention",
        HealthAssessmentState::Degraded => "degraded",
        HealthAssessmentState::Unknown => "unknown",
        HealthAssessmentState::NotEvaluated => "notEvaluated",
    }
}

fn value(row: &ResourceSummary, column: Column, usage: Usage) -> Value<'_> {
    match column {
        Column::Name => text(&row.name),
        Column::Status => text(health_state(row)),
        Column::Namespace => text(row.namespace.as_deref().unwrap_or("")),
        Column::Kind => text(&row.kind),
        Column::Ready => text(row.ready.as_deref().unwrap_or("")),
        Column::Restarts => Value::Number(row.restarts.map_or(-1., f64::from)),
        Column::Cpu => Value::Number(usage.cpu_millicores.unwrap_or(-1.)),
        Column::Memory => Value::Number(usage.memory_bytes.map_or(-1., |bytes| bytes as f64)),
        Column::Age => text(row.created_at.as_deref().unwrap_or(&row.age)),
    }
}

fn compare(left: &Value<'_>, right: &Value<'_>) -> Ordering {
    match (left, right) {
        (Value::Text(left), Value::Text(right)) => left.base_cmp(right),
        (Value::Number(left), Value::Number(right)) => left.total_cmp(right),
        // Each column produces one kind of value, so mixed kinds do not occur.
        _ => Ordering::Equal,
    }
}

/// Indices of `rows` in the table's sort order. Ties keep ascending name order,
/// as `nameCollator` breaks them on the web, even when the sort is descending.
pub fn order(
    rows: &[&ResourceSummary],
    sort: Sort,
    usage: impl Fn(&ResourceSummary) -> Usage,
) -> Vec<usize> {
    let values: Vec<Value<'_>> = rows
        .iter()
        .map(|row| value(row, sort.column, usage(row)))
        .collect();
    let names: Vec<Key<'_>> = rows.iter().map(|row| Key::new(&row.name, false)).collect();
    let mut indices: Vec<usize> = (0..rows.len()).collect();
    indices.sort_by(|&left, &right| {
        let ordering = compare(&values[left], &values[right]);
        let ordering = if sort.desc {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| names[left].full_cmp(&names[right]))
    });
    indices
}

/// Moves the rows at `moved` (positions in `rows`, ascending and unique) to
/// where [`order`] would put them after their values changed. Every other
/// row's value must be the one `order` sorted by. Ties fall back to the
/// position, as the stable sort in `order` leaves them.
pub fn reposition(
    order: &mut Vec<usize>,
    rows: &[&ResourceSummary],
    sort: Sort,
    usage: impl Fn(&ResourceSummary) -> Usage,
    moved: &[usize],
) {
    if moved.is_empty() {
        return;
    }
    // Keys are built on first use, so a few moved rows build a few keys.
    let mut keys: Vec<Option<(Value<'_>, Key<'_>)>> = Vec::new();
    keys.resize_with(rows.len(), || None);
    let mut before = |left: usize, right: usize| {
        for index in [left, right] {
            if keys[index].is_none() {
                let row = rows[index];
                keys[index] = Some((
                    value(row, sort.column, usage(row)),
                    Key::new(&row.name, false),
                ));
            }
        }
        let (Some((left_value, left_name)), Some((right_value, right_name))) =
            (&keys[left], &keys[right])
        else {
            unreachable!("both keys were built above");
        };
        let ordering = compare(left_value, right_value);
        let ordering = if sort.desc {
            ordering.reverse()
        } else {
            ordering
        };
        ordering
            .then_with(|| left_name.full_cmp(right_name))
            .then(left.cmp(&right))
    };
    let mut is_moved = crate::bits::Bits::new(rows.len());
    for &position in moved {
        is_moved.insert(position);
    }
    let kept: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&position| !is_moved.contains(position))
        .collect();
    let mut moved = moved.to_vec();
    moved.sort_by(|&left, &right| before(left, right));
    order.clear();
    let mut start = 0;
    for position in moved {
        let at = start + kept[start..].partition_point(|&row| before(row, position).is_lt());
        order.extend_from_slice(&kept[start..at]);
        order.push(position);
        start = at;
    }
    order.extend_from_slice(&kept[start..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(name: &str, restarts: i32) -> ResourceSummary {
        serde_json::from_value(json!({
            "name": name, "kind": "Pod", "cluster": "test", "namespace": "a",
            "age": "1d", "restarts": restarts
        }))
        .unwrap()
    }

    // Failure modes: a moved row lands at its old place; rows tied on the
    // sorted value and the name leave position order; a descending sort
    // inserts in ascending order; several moved rows reorder among themselves;
    // a row moved to either end is dropped or duplicated.
    #[test]
    fn repositioned_rows_match_a_full_sort() {
        let mut rows: Vec<ResourceSummary> = (0..40)
            .map(|index| row(&format!("pod-{}", index % 7), index % 5))
            .collect();
        for desc in [false, true] {
            let sort = Sort {
                column: Column::Restarts,
                desc,
            };
            let refs: Vec<&ResourceSummary> = rows.iter().collect();
            let mut order = super::order(&refs, sort, |_| Usage::default());
            let moved = [0, 3, 17, 18, 39];
            for (step, &position) in moved.iter().enumerate() {
                rows[position].restarts = Some([-1, 9, 2, 2, 0][step]);
            }
            let refs: Vec<&ResourceSummary> = rows.iter().collect();
            reposition(&mut order, &refs, sort, |_| Usage::default(), &moved);
            assert_eq!(order, super::order(&refs, sort, |_| Usage::default()));
        }
    }
}
