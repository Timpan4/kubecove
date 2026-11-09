//! Resource toolbar filter state from `ResourceBrowserTopBar.svelte` and
//! `resourceBrowserModel.ts`: namespace multi-select, health buttons, the
//! GitOps owner and the historical ReplicaSet toggle. Filters run as one pass
//! over the rows; the owner list and health counts come from that same pass.

use crate::model::HealthSummary;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext, Context, Entity, Subscription, Window};
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

use super::Inspector;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HealthFilter {
    #[default]
    All,
    Healthy,
    Unknown,
    NotEvaluated,
    Attention,
    Degraded,
    Restarted,
}

impl HealthFilter {
    /// `filterResourcesByHealth` in `src/features/resources/helpers.ts`.
    pub(super) fn matches(self, row: &ResourceSummary) -> bool {
        let state = row.health_assessment.state;
        match self {
            Self::All => true,
            Self::Healthy => state == HealthAssessmentState::Healthy,
            Self::Unknown => state == HealthAssessmentState::Unknown,
            Self::NotEvaluated => state == HealthAssessmentState::NotEvaluated,
            Self::Attention => state == HealthAssessmentState::NeedsAttention,
            Self::Degraded => state == HealthAssessmentState::Degraded,
            Self::Restarted => row.restarts.unwrap_or(0) > 0,
        }
    }
}

pub(super) struct ResourceFilters {
    /// Selected namespaces, sorted. Empty means every namespace.
    pub namespaces: Vec<String>,
    /// Selected workspace contexts (`context_filter.rs`). Empty means every context.
    pub contexts: Vec<String>,
    pub health: HealthFilter,
    /// The selected GitOps owner's `filterKey`.
    pub gitops_owner: Option<String>,
    /// The web's toggle. It hides historical ReplicaSets from the map, the
    /// topology view of these rows, using each node's `deploymentRevision`.
    pub hide_historical_replicasets: bool,
    /// Health counts over the rows that pass every filter except health.
    pub summary: HealthSummary,
    /// GitOps owner choices as (`filterKey`, label), sorted by label.
    pub owners: Vec<(String, String)>,
    pub namespace_open: bool,
    pub context_open: bool,
    pub owner_open: bool,
    pub namespace_search: Entity<InputState>,
    _subscription: Subscription,
}

impl ResourceFilters {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let namespace_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search namespaces..."));
        let subscription = cx.subscribe(&namespace_search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            namespaces: Vec::new(),
            contexts: Vec::new(),
            health: HealthFilter::All,
            gitops_owner: None,
            hide_historical_replicasets: false,
            summary: HealthSummary::default(),
            owners: Vec::new(),
            namespace_open: false,
            context_open: false,
            owner_open: false,
            namespace_search,
            _subscription: subscription,
        }
    }

    /// Whether a row's namespace is selected. Cluster-scoped rows always pass,
    /// as the web fetches cluster-scoped kinds without a namespace.
    pub fn in_namespace(&self, row: &ResourceSummary) -> bool {
        self.namespaces.is_empty()
            || row
                .namespace
                .as_ref()
                .is_none_or(|namespace| self.namespaces.contains(namespace))
    }

    /// Whether a row's GitOps owner key passes the owner filter.
    pub fn in_owner(&self, owner: Option<&str>) -> bool {
        self.gitops_owner
            .as_deref()
            .is_none_or(|selected| owner == Some(selected))
    }

    pub fn clear_owner_and_health(&mut self) {
        self.health = HealthFilter::All;
        self.gitops_owner = None;
    }
}

/// `nextNamespaceSelection` in `resourceBrowserModel.ts`. Unchecking from
/// "All" (an empty selection) removes the namespace from every available one.
pub(super) fn next_namespace_selection(
    selected: &[String],
    available: &[String],
    namespace: &str,
    checked: bool,
) -> Vec<String> {
    if checked {
        let mut next = selected.to_vec();
        if !next.iter().any(|item| item == namespace) {
            next.push(namespace.to_owned());
        }
        next.sort();
        return next;
    }
    let current = if selected.is_empty() {
        available
    } else {
        selected
    };
    current
        .iter()
        .filter(|item| item.as_str() != namespace)
        .cloned()
        .collect()
}

/// `namespaceLabel` in `ResourceBrowserTopBar.svelte`.
pub(super) fn namespace_label(selected: &[String]) -> String {
    match selected.len() {
        0 => "All".to_owned(),
        1 | 2 => selected.join(", "),
        count => format!("{} +{}", selected[..2].join(", "), count - 2),
    }
}
