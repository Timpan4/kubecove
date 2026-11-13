//! The grouped GitOps list model: `src/features/gitops/surfaceList.ts`.

use super::{Row, Selection, status_label};
use crate::gitops::GitOpsInventory;
use kubecove_lib::models::HealthAssessmentState;

/// `GitOpsListGroup`, in list order: rows that need a look sit on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Attention,
    Healthy,
    NotEvaluated,
}

impl Group {
    const ALL: [Group; 3] = [Group::Attention, Group::Healthy, Group::NotEvaluated];

    pub fn label(self) -> &'static str {
        match self {
            Group::Attention => "Not healthy",
            Group::Healthy => "Healthy",
            Group::NotEvaluated => "Not evaluated",
        }
    }

    fn of(state: HealthAssessmentState) -> Self {
        match state {
            HealthAssessmentState::Healthy => Group::Healthy,
            HealthAssessmentState::NotEvaluated => Group::NotEvaluated,
            _ => Group::Attention,
        }
    }
}

/// The collapsed groups, indexed by `Group`.
pub type Collapsed = [bool; 3];

/// `GitOpsListEntry`: one line of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `position` counts the group headers above this one.
    Group {
        group: Group,
        position: usize,
        count: usize,
        collapsed: bool,
    },
    /// `row` indexes the page's rows. An `expanded` row adds a second line
    /// with the assessment reasons and raw statuses.
    Row { row: usize, expanded: bool },
}

/// Order inside the attention group: the worst state first.
fn attention_rank(state: HealthAssessmentState) -> u8 {
    match state {
        HealthAssessmentState::Degraded => 0,
        HealthAssessmentState::NeedsAttention => 1,
        HealthAssessmentState::Unknown => 2,
        HealthAssessmentState::Healthy | HealthAssessmentState::NotEvaluated => 3,
    }
}

/// `buildGitOpsListEntries`. Empty groups are left out, and a collapsed group
/// keeps its header.
pub fn entries(rows: &[Row], data: &GitOpsInventory, collapsed: Collapsed) -> Vec<Entry> {
    let mut groups: [Vec<(u8, usize)>; 3] = Default::default();
    for (index, row) in rows.iter().enumerate() {
        let Some(selection) = row.resolve(data) else {
            continue;
        };
        let state = selection.health().state;
        groups[Group::of(state) as usize].push((attention_rank(state), index));
    }
    groups[Group::Attention as usize].sort_by_key(|(rank, _)| *rank);

    let mut entries = Vec::with_capacity(rows.len() + Group::ALL.len());
    let mut position = 0;
    for group in Group::ALL {
        let members = &groups[group as usize];
        if members.is_empty() {
            continue;
        }
        let collapsed = collapsed[group as usize];
        entries.push(Entry::Group {
            group,
            position,
            count: members.len(),
            collapsed,
        });
        position += 1;
        if collapsed {
            continue;
        }
        entries.extend(members.iter().map(|(_, row)| Entry::Row {
            row: *row,
            expanded: group == Group::Attention,
        }));
    }
    entries
}

/// The page row shown at `line`, or the first one below it, so a group header
/// stands for its first row.
pub fn row_at(entries: &[Entry], line: usize) -> Option<usize> {
    entries.get(line..)?.iter().find_map(|entry| match entry {
        Entry::Row { row, .. } => Some(*row),
        Entry::Group { .. } => None,
    })
}

/// The list line that shows page row `row`. `None` while its group is collapsed.
pub fn line_of(entries: &[Entry], row: usize) -> Option<usize> {
    entries
        .iter()
        .position(|entry| matches!(entry, Entry::Row { row: shown, .. } if *shown == row))
}

/// `gitOpsListColumns`: the Sync and Destination column labels. Every row of
/// one list has the same type, so the first row decides.
pub fn columns(first: Option<Selection>) -> (&'static str, &'static str) {
    match first {
        Some(Selection::Flux(_)) => ("Ready", "Namespace"),
        Some(Selection::Project(_)) => ("Status", "Namespace"),
        _ => ("Sync", "Destination"),
    }
}

/// `GitOpsListSyncTone`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncTone {
    Quiet,
    Active,
    Attention,
    Failed,
}

/// `gitOpsListSync`: one word for the delivery state of a row. For an
/// Application a running or failed operation wins over the sync status. The
/// web's tracked request label comes from connected operations, which this
/// client does not run.
pub fn sync(selection: Selection) -> (String, SyncTone) {
    let quiet = |label: &str| (label.to_owned(), SyncTone::Quiet);
    let status = match selection {
        Selection::Flux(item) => {
            return match item.ready_status.as_deref() {
                None | Some("") => quiet("-"),
                Some("True") => quiet("Ready"),
                Some("False") => ("Not ready".to_owned(), SyncTone::Failed),
                Some(ready) => (status_label(ready), SyncTone::Attention),
            };
        }
        Selection::Project(item) => {
            return match item.status.as_deref() {
                None | Some("") => quiet("-"),
                Some(status) => (status_label(status), SyncTone::Quiet),
            };
        }
        Selection::App(item) => {
            if item
                .refresh_requested
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                return ("Refreshing".to_owned(), SyncTone::Active);
            }
            match item.operation_phase.as_deref() {
                Some("Running") => return ("Syncing".to_owned(), SyncTone::Active),
                Some("Terminating") => return ("Terminating".to_owned(), SyncTone::Active),
                Some("Failed" | "Error") => return ("Sync failed".to_owned(), SyncTone::Failed),
                _ => item.sync_status.as_deref(),
            }
        }
        Selection::AppSet(item) => item.sync_status.as_deref(),
    };
    match status {
        None | Some("") => quiet("-"),
        Some("Synced") => quiet("Synced"),
        Some("OutOfSync") => ("Out of sync".to_owned(), SyncTone::Attention),
        Some(status) => (status_label(status), SyncTone::Attention),
    }
}
