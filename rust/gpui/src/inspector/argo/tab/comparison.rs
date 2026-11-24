//! A focused connected comparison belongs to its current Application tab and selection.
use super::*;
use crate::argo_app::{Change, identity_key};
use gpui_kit::component::highlighter::HighlightTheme;
use kubecove_lib::models::{ArgoManagedResource, ArgoResourceComparison};

/// A comparison of the focused resource with its change, accessible label and
/// diff highlights, built off the UI thread.
type Prepared = (
    ArgoResourceComparison,
    Change,
    Arc<[super::super::highlight::Highlights]>,
);

/// Builds the change of `resource` from its comparison, as the snapshot job
/// builds every change, or the message the comparison fails with. The job
/// owns its highlighter, so cancelling it takes nothing from the UI.
fn prepare(
    result: Result<ArgoResourceComparison, AppError>,
    resource: &ArgoManagedResource,
    theme: &HighlightTheme,
) -> Result<Prepared, String> {
    let comparison = match result {
        Ok(comparison) if identity_key(&comparison.resource) == identity_key(resource) => {
            comparison
        }
        Ok(_) => return Err("Comparison returned a different resource".into()),
        Err(error) => return Err(error.message),
    };
    let change = crate::argo_app::change(resource, std::slice::from_ref(&comparison));
    let highlights = Arc::from(super::super::highlight::lines(
        &mut None,
        change.diff.as_deref().unwrap_or_default(),
        theme,
    ));
    Ok((comparison, change, highlights))
}

impl Inspector {
    pub(in crate::inspector) fn focus_argo_resource(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection_id = self.argo_connection_id();
        let source = self.argo_source_key().map(str::to_owned);
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        state.focus = index;
        // Cancels the comparison of the resource that had the focus.
        state.comparison = Fetch::Idle;
        cx.notify();
        // Leaving the focus shows every change again, some perhaps for the first time.
        self.ensure_argo_editors(window, cx);
        let Some(index) = index else {
            return;
        };
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        let ArgoLoad::Ready(view) = &state.load else {
            return;
        };
        let Some(snapshot) = &state.inspector else {
            return;
        };
        if snapshot.transport != "connected" {
            return;
        }
        let Some(change) = view.changes.get(index) else {
            return;
        };
        if snapshot
            .comparisons
            .iter()
            .any(|c| identity_key(&c.resource).as_ref() == Some(&change.key))
        {
            return;
        }
        let Some(resource) = snapshot
            .resources
            .iter()
            .find(|r| identity_key(r).as_ref() == Some(&change.key))
            .cloned()
        else {
            return;
        };
        let request = crate::backend::ArgoInspection {
            application: snapshot.application.clone(),
            connection_id,
            source,
        };
        let read = self.backend.compare_argo(request, resource.clone());
        let theme = gpui_kit::component::Theme::global(cx)
            .highlight_theme
            .clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let prepared = cx
                .background_executor()
                .spawn(async move { prepare(result, &resource, &theme) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Detail::Ready(detail) = &mut this.detail else {
                    return;
                };
                let Some(state) = detail.argo.as_mut() else {
                    return;
                };
                state.comparison = match prepared {
                    Ok((comparison, change, highlights)) => {
                        if let ArgoLoad::Ready(view) = &mut state.load {
                            // `ensure_argo_editors` rebuilds them from the new documents.
                            if let Some(editors) = state.editors.get_mut(index) {
                                *editors = Default::default();
                            }
                            state.highlights[index] = highlights;
                            view.changes[index] = change;
                        }
                        if let Some(snapshot) = &mut state.inspector {
                            snapshot.comparisons.push(comparison);
                        }
                        Fetch::Ready(())
                    }
                    Err(message) => Fetch::Failed(message),
                };
                this.ensure_argo_editors(window, cx);
                cx.notify();
            });
        });
        if let Some(state) = self.argo_tab_state() {
            state.comparison = Fetch::start(task);
        }
    }
}
