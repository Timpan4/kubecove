//! The saved Service forward prompt above an open workspace, mirroring
//! `SavedPortForwardRestore.svelte`. It shows while the workspace has saved
//! presets and auto-start is off.
use super::controls::{Variant, button};
use super::{Inspector, Page};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{AnyElement, Context, TestSupportExt, Window, div, prelude::*, px};

#[derive(Default)]
pub(super) struct RestorePrompt {
    /// The workspace whose prompt Skip dismissed.
    dismissed: Option<u64>,
    /// The workspace whose Start saved is in flight. A successful start
    /// dismisses its prompt; a failed one keeps it with `error`.
    starting: Option<u64>,
    error: Option<String>,
}

impl RestorePrompt {
    /// Called when a forward action finishes: `startSavedPortForwards` results
    /// decide whether the prompt that started it closes.
    pub(super) fn finish_start(&mut self, failure: Option<String>) {
        let Some(workspace) = self.starting.take() else {
            return;
        };
        match failure {
            None => self.dismissed = Some(workspace),
            Some(error) => self.error = Some(error),
        }
    }
}

impl Inspector {
    /// `shouldShowSavedPortForwardRestorePrompt`: the open workspace, and its
    /// saved preset count when the prompt shows.
    fn restore_prompt_count(&self) -> Option<usize> {
        let id = self.launcher.active?;
        if self.persisted.preferences.auto_start_saved_port_forwards
            || self.restore_prompt.dismissed == Some(id)
        {
            return None;
        }
        let count = self
            .persisted
            .saved_forward_presets
            .get(&self.preset_key())
            .map_or(0, Vec::len);
        (count > 0).then_some(count)
    }

    pub(super) fn restore_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let count = self.restore_prompt_count()?;
        let noun = if count == 1 { "forward" } else { "forwards" };
        let text = format!("This workspace has {count} saved Service {noun} ready to start.");
        let controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .id("restore-prompt-auto-start")
                    .test_support()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .aria_label("Auto-start saved port forwards")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let auto_start =
                            &mut this.persisted.preferences.auto_start_saved_port_forwards;
                        *auto_start = !*auto_start;
                        this.save_preferences(cx);
                        if this.persisted.preferences.auto_start_saved_port_forwards {
                            this.start_restore_prompt(window, cx);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(16.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(Tokens::input())
                            .bg(Tokens::input().opacity(0.3)),
                    )
                    .child(ui::muted(ui::xs("Auto-start"))),
            )
            .child(
                button(Variant::Default, 28., self.sessions.busy)
                    .id("restore-prompt-start")
                    .test_support()
                    .aria_label("Start saved")
                    .child(ui::icon("play", 12., Tokens::primary_foreground()))
                    .child(ui::xs("Start saved"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.start_restore_prompt(window, cx);
                    })),
            )
            .child(
                button(Variant::Outline, 28., false)
                    .id("restore-prompt-review")
                    .test_support()
                    .aria_label("Review")
                    .child(ui::icon("eye", 12., Tokens::foreground()))
                    .child(ui::xs("Review"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_page(Page::PortForwards, cx);
                    })),
            )
            .child(
                button(Variant::Ghost, 28., false)
                    .id("restore-prompt-skip")
                    .test_support()
                    .aria_label("Skip")
                    .child(ui::icon("x", 12., Tokens::foreground()))
                    .child(ui::xs("Skip"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(id) = this.launcher.active {
                            this.restore_prompt.dismissed = Some(id);
                        }
                        this.restore_prompt.error = None;
                        cx.notify();
                    })),
            );
        Some(
            div()
                .id("restore-prompt")
                .test_support()
                .role(gpui_kit::Role::Alert)
                .aria_label(text.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .px(px(16.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::icon("cable", 14., Tokens::foreground()))
                        .child(ui::semibold(ui::xs("Saved port forwards available")))
                        .child(ui::muted(ui::xs(text))),
                )
                .child(controls)
                .children(self.restore_prompt.error.clone().map(|error| {
                    div()
                        .id("restore-prompt-error")
                        .test_support()
                        .aria_label(error.clone())
                        .w_full()
                        .child(ui::xs(error).text_color(Tokens::destructive()))
                }))
                .into_any_element(),
        )
    }

    /// Start saved from the prompt: starts every saved preset of the workspace.
    fn start_restore_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.launcher.active else {
            return;
        };
        if self.sessions.busy {
            return;
        }
        self.restore_prompt.error = None;
        self.restore_prompt.starting = Some(id);
        self.restore_forwards(window, cx);
        cx.notify();
    }
}
