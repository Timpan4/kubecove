//! A focused connected comparison belongs to its current Application tab and selection.
use super::*;

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
        state.comparison.take();
        state.comparison_error = None;
        state.comparison_request = Rc::new(());
        let current = state.comparison_request.clone();
        cx.notify();
        let Some(index) = index else {
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
            .any(|c| crate::argo_app::identity_key(&c.resource).as_ref() == Some(&change.key))
        {
            return;
        }
        let Some(resource) = snapshot
            .resources
            .iter()
            .find(|r| crate::argo_app::identity_key(r).as_ref() == Some(&change.key))
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
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Detail::Ready(detail) = &mut this.detail else {
                    return;
                };
                let Some(state) = detail.argo.as_mut() else {
                    return;
                };
                if !Rc::ptr_eq(&current, &state.comparison_request) {
                    return;
                }
                match result {
                    Ok(comparison)
                        if crate::argo_app::identity_key(&comparison.resource)
                            == crate::argo_app::identity_key(&resource) =>
                    {
                        let change =
                            crate::argo_app::change(&resource, std::slice::from_ref(&comparison));
                        if let ArgoLoad::Ready(view) = &mut state.load {
                            use gpui_kit::{AppContext, component::input::EditorState};
                            state.editors[index] = (
                                cx.new(|cx| {
                                    EditorState::new(window, cx)
                                        .language("yaml")
                                        .line_number(true)
                                        .default_value(change.desired.clone())
                                }),
                                cx.new(|cx| {
                                    EditorState::new(window, cx)
                                        .language("yaml")
                                        .line_number(true)
                                        .default_value(change.live.clone())
                                }),
                            );
                            state.highlights[index] = super::super::highlight::lines(
                                &mut this.argo_highlighter,
                                change.diff.as_deref().unwrap_or_default(),
                                &gpui_kit::component::Theme::global(cx).highlight_theme,
                            );
                            view.changes[index] = change;
                        }
                        if let Some(snapshot) = &mut state.inspector {
                            snapshot.comparisons.push(comparison);
                        }
                    }
                    Ok(_) => {
                        state.comparison_error =
                            Some("Comparison returned a different resource".into())
                    }
                    Err(error) => state.comparison_error = Some(error.message),
                }
                cx.notify();
            });
        });
        if let Some(state) = self.argo_tab_state() {
            state.comparison = Some(task);
        }
    }
}
