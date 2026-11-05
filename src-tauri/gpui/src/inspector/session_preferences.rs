//! User preferences follow `settings-store.ts` and the workspace preset store.
use super::Inspector;
use crate::backend::SessionPreferences;
use gpui_kit::{Context, Task};
use tokio::sync::mpsc;

#[derive(Default)]
pub(super) struct PreferencesState {
    loaded: bool,
    sender: Option<mpsc::UnboundedSender<SessionPreferences>>,
    task: Option<Task<()>>,
}
impl Inspector {
    pub(super) fn preset_key(&self) -> String {
        let source = self
            .sessions
            .load
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|s| s.source_key.as_deref());
        serde_json::to_string(&(source, &self.scope.context, self.workspace_name.as_ref()))
            .expect("string tuple")
    }
    pub(super) fn load_session_preferences(&mut self, cx: &mut Context<Self>) {
        let read = self.backend.session_preferences();
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(preferences) => {
                        this.sessions.preferences.loaded = true;
                        this.sessions.presets = preferences.presets;
                        this.sessions.auto_start = preferences.auto_start;
                        this.sessions.keep = preferences.keep;
                    }
                    Err(error) => this.sessions.error = Some(error.message),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn persist_session_preferences(&mut self, cx: &mut Context<Self>) {
        if !self.sessions.preferences.loaded {
            // A failed read must never replace the user's original with defaults.
            self.sessions.error =
                Some("Preferences have not loaded. These changes are not saved.".into());
            return;
        }
        if self.sessions.preferences.sender.is_none() {
            let (sender, mut receiver) = mpsc::unbounded_channel::<SessionPreferences>();
            let backend = self.backend.clone();
            self.sessions.preferences.sender = Some(sender);
            self.sessions.preferences.task = Some(cx.spawn(async move |this, cx| {
                while let Some(preferences) = receiver.recv().await {
                    if let Err(error) = backend.save_session_preferences(preferences).await {
                        let _ = this.update(cx, |this, cx| {
                            this.sessions.error = Some(error.message);
                            cx.notify();
                        });
                    }
                }
            }));
        }
        let _ = self
            .sessions
            .preferences
            .sender
            .as_ref()
            .unwrap()
            .send(SessionPreferences {
                presets: self.sessions.presets.clone(),
                auto_start: self.sessions.auto_start,
                keep: self.sessions.keep,
            });
    }
}
