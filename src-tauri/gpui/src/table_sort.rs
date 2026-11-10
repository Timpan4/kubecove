//! Resource table sort columns. The table sorts packed keys
//! (`model/rows/sort.rs`); `reference::order` mirrors `sortedResourceRows` and
//! `valueForSort` in `src/features/resources/resourceTableModel.ts` directly,
//! as the tests' reference for that sort.

use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

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
    /// The row's Kubernetes context, shown when a view holds several.
    Context,
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
            Column::Context => "Context",
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

/// The `state` strings of `HealthAssessmentState` in `src/lib/health-types.ts`.
pub(crate) fn health_state(row: &ResourceSummary) -> &'static str {
    match row.health_assessment.state {
        HealthAssessmentState::Healthy => "healthy",
        HealthAssessmentState::NeedsAttention => "needsAttention",
        HealthAssessmentState::Degraded => "degraded",
        HealthAssessmentState::Unknown => "unknown",
        HealthAssessmentState::NotEvaluated => "notEvaluated",
    }
}

#[cfg(test)]
pub(crate) mod reference {
    use super::{Column, Sort, Usage, health_state};
    use crate::collate::Key;
    use kubecove_lib::models::ResourceSummary;
    use std::cmp::Ordering;

    enum Value<'a> {
        Text(Key<'a>),
        Number(f64),
    }

    fn text(value: &str) -> Value<'_> {
        Value::Text(Key::new(value, true))
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
            Column::Context => text(&row.cluster),
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

    /// Indices of `rows` in the table's sort order. Ties keep ascending name
    /// order, as `nameCollator` breaks them on the web, even when the sort is
    /// descending, then the order of the row's context in `contexts`. A
    /// context outside `contexts` ranks after every listed one.
    pub(crate) fn order(
        rows: &[&ResourceSummary],
        contexts: &[String],
        sort: Sort,
        usage: impl Fn(&ResourceSummary) -> Usage,
    ) -> Vec<usize> {
        let values: Vec<Value<'_>> = rows
            .iter()
            .map(|row| value(row, sort.column, usage(row)))
            .collect();
        let names: Vec<Key<'_>> = rows.iter().map(|row| Key::new(&row.name, false)).collect();
        let ranks: Vec<usize> = rows
            .iter()
            .map(|row| {
                contexts
                    .iter()
                    .position(|context| *context == row.cluster)
                    .unwrap_or(contexts.len())
            })
            .collect();
        let mut indices: Vec<usize> = (0..rows.len()).collect();
        indices.sort_by(|&left, &right| {
            let ordering = compare(&values[left], &values[right]);
            let ordering = if sort.desc {
                ordering.reverse()
            } else {
                ordering
            };
            ordering
                .then_with(|| names[left].full_cmp(&names[right]))
                .then_with(|| ranks[left].cmp(&ranks[right]))
        });
        indices
    }
}
