//! Header app-update control, mirroring `UpdateStatusButton.svelte`. It shows
//! the same typed update check that Settings uses. Installation stays disabled,
//! so this panel never downloads or installs anything.

use super::Inspector;
use super::controls::{Variant, button};
use crate::backend::{INSTALL_UNAVAILABLE, UpdateStatus};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, SharedString, deferred, div, prelude::*, px};

/// Whether the popover is open, and the version it last opened for. As on the
/// web, a version opens the popover once, and closing it keeps it closed.
#[derive(Default)]
pub(super) struct PanelState {
    pub(super) open: bool,
    auto_opened: Option<String>,
}

impl Inspector {
    /// Opens the popover for a newly available version that the user has not dismissed.
    pub(super) fn offer_available_update(&mut self) {
        let Some(version) = self.available_version() else {
            return;
        };
        let dismissed =
            self.persisted.updates.dismissed_version.as_deref() == Some(version.as_str());
        let auto_opened = self.update_panel.auto_opened.as_deref() == Some(version.as_str());
        if !dismissed && !auto_opened {
            self.update_panel.open = true;
            self.update_panel.auto_opened = Some(version);
        }
    }

    fn available_version(&self) -> Option<String> {
        match &self.settings.integrations.update {
            UpdateStatus::Available(update) => Some(update.version.clone()),
            _ => None,
        }
    }

    pub(super) fn close_update_panel(&mut self) {
        self.update_panel.open = false;
        if let Some(version) = self.available_version() {
            self.update_panel.auto_opened = Some(version);
        }
    }

    /// `dismissUpdate`: remembers the version across restarts and closes the popover.
    fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        if let Some(version) = self.available_version() {
            self.persisted.updates.dismissed_version = Some(version);
            self.save_preferences(cx);
        }
        self.close_update_panel();
        cx.notify();
    }

    fn update_checking(&self) -> super::loading::Phase {
        self.loading.phase(
            "update-check",
            self.settings.integrations.update_check.is_some(),
        )
    }

    fn update_tooltip(&self) -> &'static str {
        let state = &self.settings.integrations;
        if state.update_check.is_some() && self.update_checking().shown() {
            "Checking for updates"
        } else if state.update_error.is_some() {
            "Update check failed"
        } else if self.available_version().is_some() {
            "Update available"
        } else {
            "Check for updates"
        }
    }

    /// The header button and its popover. Hidden when update checks are
    /// unavailable, as the web hides the button outside stable builds.
    pub(super) fn update_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.backend.updates_enabled() {
            return None;
        }
        let open = self.update_panel.open;
        let trigger = ui::ghost_icon("refresh-cw", 28., 14.)
            .id("header-update")
            .test_support()
            .aria_label(self.update_tooltip())
            .cursor_pointer()
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if open {
                        this.close_update_panel();
                    } else {
                        this.update_panel.open = true;
                    }
                    cx.notify();
                }),
            );
        let mut wrapper = div().relative().mr(px(4.)).child(trigger);
        if self.available_version().is_some() {
            wrapper = wrapper.child(
                div()
                    .absolute()
                    .top(px(4.))
                    .right(px(4.))
                    .size(px(8.))
                    .rounded_full()
                    .bg(Tokens::primary()),
            );
        }
        if self.update_panel.open {
            wrapper = wrapper.child(
                deferred(
                    self.update_panel_card(cx)
                        .id("update-status-popover")
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.close_update_panel();
                            cx.notify();
                        }))
                        .occlude()
                        .absolute()
                        .top(px(36.))
                        .right_0()
                        .w(px(320.))
                        .shadow_lg(),
                )
                .with_priority(1),
            );
        }
        Some(wrapper.into_any_element())
    }

    fn update_panel_card(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.integrations;
        let mut header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .min_w_0()
                    .child(ui::semibold(ui::sm("App updates")))
                    .child(
                        ui::muted(ui::xs(format!(
                            "Current version {}",
                            env!("CARGO_PKG_VERSION")
                        )))
                        .mt(px(4.)),
                    ),
            );
        if let Some(version) = self.available_version() {
            header = header.child(ui::outline_badge(version));
        }
        let last_checked = state
            .checked_at
            .as_deref()
            .unwrap_or("Not checked yet")
            .to_owned();
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .pt(px(12.))
            .border_t_1()
            .border_color(Tokens::border())
            .child(ui::muted(ui::xs(format!("Last checked: {last_checked}"))))
            .child(
                button(Variant::Outline, 24., state.update_check.is_some())
                    .id("header-update-check")
                    .test_support()
                    .aria_label("Check")
                    .child(ui::icon("refresh-cw", 14., Tokens::foreground()))
                    .child("Check")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.check_settings_updates(window, cx)),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(header)
            .child(self.update_body(cx))
            .child(footer)
    }

    fn update_body(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.integrations;
        let body = div().flex().flex_col().gap(px(8.));
        if let Some(error) = &state.update_error {
            return body
                .child(labelled(
                    ui::sm("Update check failed.").text_color(Tokens::destructive()),
                    "Update check failed.",
                ))
                .child(labelled(ui::xs(error.clone()), error));
        }
        if state.update_check.is_some() {
            if !self.update_checking().shown() {
                return body;
            }
            return body.child(labelled(
                ui::muted(ui::sm("Checking for updates...")),
                "Checking for updates...",
            ));
        }
        match &state.update {
            UpdateStatus::Available(update) => {
                let title = format!("KubeCove {} is available.", update.version);
                let mut body = body.child(labelled(ui::sm(title.clone()), &title));
                if let Some(notes) = &update.notes {
                    body = body.child(
                        div()
                            .id("header-update-notes")
                            .max_h(px(112.))
                            .overflow_y_scroll()
                            .p(px(8.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::muted().opacity(0.4))
                            .child(ui::muted(ui::xs(notes.clone())).line_height(px(19.5)))
                            .test_support()
                            .aria_label(notes.clone()),
                    );
                }
                body.child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            button(Variant::Ghost, 28., false)
                                .id("header-update-later")
                                .test_support()
                                .aria_label("Later")
                                .child(ui::icon("x", 14., Tokens::foreground()))
                                .child("Later")
                                .on_click(cx.listener(|this, _, _, cx| this.dismiss_update(cx))),
                        )
                        .child(
                            button(Variant::Default, 28., true)
                                .id("header-update-install")
                                .test_support()
                                .aria_label("Install")
                                .child(ui::icon("download", 14., Tokens::primary_foreground()))
                                .child("Install")
                                .aria_description(INSTALL_UNAVAILABLE),
                        ),
                )
            }
            UpdateStatus::UpToDate => body.child(labelled(
                ui::muted(ui::sm("KubeCove is up to date.")),
                "KubeCove is up to date.",
            )),
            UpdateStatus::Idle => body.child(labelled(
                ui::muted(ui::sm("Ready to check.")),
                "Ready to check.",
            )),
        }
    }
}

/// Gives a status line a test label with its visible text.
fn labelled(element: Div, text: &str) -> AnyElement {
    element
        .id(SharedString::from(format!("header-update-text:{text}")))
        .test_support()
        .aria_label(text.to_owned())
        .into_any_element()
}
