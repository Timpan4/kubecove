//! Actions tab session (ADR 0004, ADR 0024), ported from
//! `resourceActionSession.ts`: the permission baseline, the replicas draft,
//! the server dry-run review and the guarded execution that follows it. Each
//! load and review owns its task, so replacing or closing one drops its result.

use super::Inspector;
use super::detail::Detail;
use crate::actions::{self, ActionRow, RowId};
use crate::unified_diff::{self, DiffLine};
use gpui_kit::component::{
    WindowExt,
    input::{InputEvent, InputState},
};
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Task, Window};
use kubecove_lib::models::{
    ResourceActionKind, ResourceActionPreview, ResourceActionRequest, ResourceActionState,
    ResourceSummary,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewStatus {
    Checking,
    Ready,
    Executing,
    Failed,
}

pub(super) struct Review {
    pub action: ResourceActionKind,
    pub name: String,
    pub status: ReviewStatus,
    request: ResourceActionRequest,
    pub preview: Option<ResourceActionPreview>,
    pub diff: Vec<DiffLine>,
    pub diff_text: SharedString,
    pub error: Option<String>,
    /// The API server may have received the failed execution.
    pub execution_attempted: bool,
    pub confirmation: Entity<InputState>,
    _task: Option<Task<()>>,
    _subscription: Subscription,
}

impl Review {
    pub fn busy(&self) -> bool {
        matches!(
            self.status,
            ReviewStatus::Checking | ReviewStatus::Executing
        )
    }

    /// `canExecute`: a ready preview, and the typed name for a deletion.
    pub fn can_execute(&self, cx: &App) -> bool {
        self.status == ReviewStatus::Ready
            && self.preview.is_some()
            && self.error.is_none()
            && (self.action != ResourceActionKind::Delete
                || self.confirmation.read(cx).value().as_ref() == self.name)
    }
}

pub(super) struct ActionsState {
    resource: ResourceSummary,
    pub rows: Vec<ActionRow>,
    pub filter: Entity<InputState>,
    /// The trimmed, lowercased filter.
    pub query: String,
    pub state: Option<ResourceActionState>,
    pub loading: bool,
    pub load_error: Option<String>,
    /// `edits.replicas`, the only draft the native client edits.
    pub replicas: Option<i32>,
    pub expanded: Option<RowId>,
    pub replicas_input: Entity<InputState>,
    pub review: Option<Review>,
    /// Effect of the last successful action.
    pub result: Option<String>,
    load_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ActionsState {
    pub fn resource(&self) -> &ResourceSummary {
        &self.resource
    }

    pub fn pending(&self) -> usize {
        usize::from(self.replicas.is_some())
    }
}

/// `setReplicas(Number(value))`: an empty field reads as zero; anything that
/// is not a non-negative integer leaves the draft unchanged.
fn parse_replicas(value: &str) -> Option<i32> {
    let number = match value.trim() {
        "" => 0.,
        text => text.parse::<f64>().ok()?,
    };
    (number.fract() == 0. && number >= 0. && number <= f64::from(i32::MAX)).then_some(number as i32)
}

impl Inspector {
    pub(super) fn actions_state(&mut self) -> Option<&mut ActionsState> {
        match &mut self.detail {
            Detail::Ready(detail) => detail.actions.as_mut(),
            _ => None,
        }
    }

    /// Creates the session and loads the baseline the first time the tab opens.
    pub(super) fn open_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail.actions.is_some() {
            return;
        }
        let resource = detail.data.summary.clone();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter actions"));
        let replicas_input = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&filter, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = state.read(cx).value().trim().to_lowercase();
                    if let Some(actions) = this.actions_state() {
                        actions.query = query;
                    }
                    cx.notify();
                }
            }),
            cx.subscribe_in(&replicas_input, window, |this, state, event, _, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = parse_replicas(&state.read(cx).value());
                if let Some(actions) = this.actions_state()
                    && actions.state.is_some()
                    && !actions.loading
                    && let Some(value) = value
                {
                    actions.replicas = Some(value);
                }
                cx.notify();
            }),
        ];
        let rows = actions::rows(&resource);
        detail.actions = Some(ActionsState {
            loading: !rows.is_empty(),
            rows,
            resource,
            filter,
            query: String::new(),
            state: None,
            load_error: None,
            replicas: None,
            expanded: None,
            replicas_input,
            review: None,
            result: None,
            load_task: None,
            _subscriptions: subscriptions,
        });
        self.load_action_state(window, cx);
    }

    /// `loadBaseline`; `retry` calls it again after a failure.
    pub(super) fn load_action_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Some(actions) = self.actions_state() else {
            return;
        };
        if actions.rows.is_empty() {
            return;
        }
        actions.loading = true;
        actions.load_error = None;
        let read = backend.action_state(kubecove_lib::models::ResourceActionStateRequest {
            target: actions::target(&actions.resource),
            kubeconfig_env_var: None,
        });
        actions.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(actions) = this.actions_state() {
                    match result {
                        Ok(state) => {
                            actions.state = Some(state);
                            actions.load_error = None;
                        }
                        Err(error) => {
                            actions.state = None;
                            actions.load_error = Some(error.message);
                        }
                    }
                    actions.loading = false;
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Opens the replicas editor with the draft or the loaded value.
    pub(super) fn toggle_action_row(
        &mut self,
        row: RowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(actions) = self.actions_state() else {
            return;
        };
        if actions.expanded == Some(row) {
            actions.expanded = None;
        } else {
            actions.expanded = Some(row);
            let value = actions
                .replicas
                .or(actions.state.as_ref().and_then(|state| state.replicas))
                .unwrap_or(0)
                .to_string();
            let input = actions.replicas_input.clone();
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        cx.notify();
    }

    /// `review`: a server dry run of the exact target, shown in the dialog.
    pub(super) fn review_action(
        &mut self,
        action: ResourceActionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backend = self.backend.clone();
        let Some(actions) = self.actions_state() else {
            return;
        };
        if actions.loading || !actions::allowed(actions.state.as_ref(), action) {
            return;
        }
        if action == ResourceActionKind::Edit && actions.replicas.is_none() {
            return;
        }
        // A running execution keeps its result and refresh.
        if actions
            .review
            .as_ref()
            .is_some_and(|review| review.status == ReviewStatus::Executing)
        {
            return;
        }
        let replicas = (action == ResourceActionKind::Edit)
            .then_some(actions.replicas)
            .flatten();
        let request = actions::review_request(&actions.resource, action, replicas);
        let read = backend.preview_action(request.clone());
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(review) = this.actions_state().and_then(|a| a.review.as_mut()) {
                    match result {
                        Ok(preview) => {
                            review.diff =
                                unified_diff::compact(&preview.before_yaml, &preview.after_yaml);
                            review.diff_text = review
                                .diff
                                .iter()
                                .map(|line| line.text.as_ref())
                                .collect::<Vec<_>>()
                                .join("\n")
                                .into();
                            review.preview = Some(preview);
                            review.status = ReviewStatus::Ready;
                        }
                        // A failed dry run shows its error and never executes.
                        Err(error) => {
                            review.status = ReviewStatus::Failed;
                            review.error = Some(error.message);
                        }
                    }
                }
                cx.notify();
                window.refresh();
            });
        });
        let confirmation = cx.new(|cx| InputState::new(window, cx));
        let subscription = cx.subscribe_in(&confirmation, window, |_, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
                window.refresh();
            }
        });
        let Some(actions) = self.actions_state() else {
            return;
        };
        actions.result = None;
        actions.review = Some(Review {
            action,
            name: actions.resource.name.clone(),
            status: ReviewStatus::Checking,
            request,
            preview: None,
            diff: Vec::new(),
            diff_text: SharedString::default(),
            error: None,
            execution_attempted: false,
            confirmation,
            _task: Some(task),
            _subscription: subscription,
        });
        self.open_review_dialog(window, cx);
        cx.notify();
    }

    /// `execute`: the reviewed request, guarded by the previewed identity.
    pub(super) fn execute_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Some(actions) = self.actions_state() else {
            return;
        };
        let Some(review) = actions.review.as_mut() else {
            return;
        };
        if !review.can_execute(cx) {
            return;
        }
        let Some(preview) = review.preview.as_ref() else {
            return;
        };
        let request = actions::execution_request(&review.request, preview);
        review.status = ReviewStatus::Executing;
        review.error = None;
        let reviewed_replicas = request.edits.as_ref().and_then(|edits| edits.replicas);
        let edit = request.action == ResourceActionKind::Edit;
        let read = backend.execute_action(request);
        review._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(actions) = this.actions_state() else {
                    return;
                };
                match result {
                    Ok(result) => {
                        // Drafts edited while this one executed stay pending.
                        if edit && actions.replicas == reviewed_replicas {
                            actions.replicas = None;
                            actions.expanded = None;
                        }
                        actions.review = None;
                        actions.result = Some(result.effect);
                        window.defer(cx, |window, cx| {
                            if window.has_active_dialog(cx) {
                                window.close_dialog(cx);
                            }
                        });
                        // Later reviews wait for the reloaded baseline (ADR 0024).
                        this.load_action_state(window, cx);
                    }
                    Err(error) => {
                        if let Some(review) = actions.review.as_mut() {
                            review.status = ReviewStatus::Failed;
                            review.error = Some(error.message);
                            review.execution_attempted = true;
                        }
                    }
                }
                cx.notify();
                window.refresh();
            });
        }));
        cx.notify();
        window.refresh();
    }

    /// `closeReview`: an executing review stays open.
    pub(super) fn close_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(actions) = self.actions_state() else {
            return;
        };
        if actions
            .review
            .as_ref()
            .is_some_and(|review| review.status == ReviewStatus::Executing)
        {
            return;
        }
        actions.review = None;
        // Deferred, so a dialog close callback never re-enters this update.
        window.defer(cx, |window, cx| {
            if window.has_active_dialog(cx) {
                window.close_dialog(cx);
            }
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::parse_replicas;

    // Failure modes: an empty field must read as zero like `Number("")`;
    // fractions, negatives and text must not replace the draft.
    #[test]
    fn replicas_follow_number_coercion() {
        assert_eq!(parse_replicas(""), Some(0));
        assert_eq!(parse_replicas(" 3 "), Some(3));
        assert_eq!(parse_replicas("2.5"), None);
        assert_eq!(parse_replicas("-1"), None);
        assert_eq!(parse_replicas("abc"), None);
    }
}
