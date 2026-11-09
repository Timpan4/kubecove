//! App updates: the check, the install and the relaunch, as `features/app-updates/store.ts`
//! runs them on the web. Settings and the header popover both show this one state.
use super::request::InFlight;
use super::settings_integrations::action;
use super::{Inspector, settings_page::settings_row, surface::secondary_badge};
use crate::backend::{DownloadProgress, Relaunch, UpdateStatus};
use futures_util::future::{Either, select};
use gpui_kit::{Context, Div, Window, div, prelude::*, px};

#[derive(Default)]
pub(super) struct Updates {
    pub(super) status: UpdateStatus,
    pub(super) checked_at: Option<String>,
    /// The last failed check, install or relaunch. The next check clears it.
    pub(super) error: Option<String>,
    /// The update check in flight. Check is disabled while it runs.
    pub(super) check: Option<InFlight>,
    pub(super) install: Install,
}

/// Where the available update's installation stands.
#[derive(Default)]
pub(super) enum Install {
    #[default]
    NotStarted,
    /// The download and install in flight. Dropping the run cancels the download.
    Downloading {
        progress: DownloadProgress,
        _run: InFlight,
    },
    /// The new version is in place and `Relaunch` starts it.
    Installed(Relaunch),
}

impl Updates {
    /// `canCheckForUpdates`: a running check, a download, or an install waiting for its
    /// relaunch owns the pending update.
    pub(super) fn can_check(&self) -> bool {
        self.check.is_none() && matches!(self.install, Install::NotStarted)
    }

    /// The web's `available` status: an update to offer, with nothing failed or under way.
    pub(super) fn offered(&self) -> Option<&crate::backend::AvailableUpdate> {
        match (&self.status, &self.install, &self.error) {
            (UpdateStatus::Available(update), Install::NotStarted, None) => Some(update),
            _ => None,
        }
    }
}

impl Inspector {
    pub(super) fn check_for_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.updates.can_check() || !self.backend.updates_enabled() {
            return;
        }
        let read = self.backend.check_updates();
        let read = self.timings.measure("check_updates", read);
        let check = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.updates;
                state.check = None;
                state.checked_at = Some(
                    chrono::Local::now()
                        .format("%b %-d, %Y, %-H:%M")
                        .to_string(),
                );
                this.persisted.updates.last_checked_at = state.checked_at.clone();
                match result {
                    Ok(status) => state.status = status,
                    Err(error) => {
                        state.status = UpdateStatus::Idle;
                        state.error = Some(error.message);
                    }
                }
                this.save_preferences(cx);
                this.offer_available_update();
                cx.notify();
            });
        });
        self.updates.error = None;
        self.updates.check = Some(check.into());
        cx.notify();
    }

    /// `installUpdate`: downloads, verifies and installs the offered update. A failure shows
    /// its message and leaves the installed version as it was.
    pub(super) fn install_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(update) = self.updates.offered().cloned() else {
            return;
        };
        let (sender, mut progress) = tokio::sync::watch::channel(DownloadProgress::default());
        let mut read = self.backend.install_update(update, sender);
        let run = cx.spawn_in(window, async move |this, cx| {
            let result = loop {
                let changed = {
                    let changed = std::pin::pin!(progress.changed());
                    match select(&mut read, changed).await {
                        Either::Left((result, _)) => break result,
                        Either::Right((changed, _)) => changed,
                    }
                };
                if changed.is_err() {
                    // The backend dropped its sender, so only the result is left.
                    break read.await;
                }
                let latest = *progress.borrow_and_update();
                let _ = this.update(cx, |this, cx| {
                    let Install::Downloading { progress, .. } = &mut this.updates.install else {
                        return;
                    };
                    // A chunk arrives far more often than the shown percentage changes.
                    let shown = progress.percent();
                    *progress = latest;
                    if shown != latest.percent() {
                        cx.notify();
                    }
                });
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(relaunch) => this.updates.install = Install::Installed(relaunch),
                    Err(error) => {
                        this.updates.install = Install::NotStarted;
                        this.updates.error = Some(error.message);
                    }
                }
                cx.notify();
            });
        });
        self.updates.install = Install::Downloading {
            progress: DownloadProgress::default(),
            _run: run.into(),
        };
        cx.notify();
    }

    /// `relaunchApp`: starts the installed version and quits this one. Like closing the
    /// window, it stays while an unsaved YAML draft would be lost.
    pub(super) fn relaunch_after_update(&mut self, cx: &mut Context<Self>) {
        let Install::Installed(relaunch) = &self.updates.install else {
            return;
        };
        let relaunch = relaunch.clone();
        if !self.can_leave(cx) {
            return;
        }
        match self.backend.relaunch(&relaunch) {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.updates.install = Install::NotStarted;
                self.updates.error = Some(error.message);
                cx.notify();
            }
        }
    }

    pub(super) fn updates_settings_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.updates;
        let enabled = self.backend.updates_enabled();
        let description = if enabled {
            format!(
                "Current version {}. Last checked: {}.",
                env!("CARGO_PKG_VERSION"),
                self.persisted
                    .updates
                    .last_checked_at
                    .as_deref()
                    .unwrap_or("Not checked yet")
            )
        } else {
            format!(
                "Current version {}. Development build; update checks disabled.",
                env!("CARGO_PKG_VERSION")
            )
        };
        let mut control = div().flex().items_center().gap(px(8.));
        if enabled {
            if let UpdateStatus::Available(update) = &state.status {
                control = control.child(crate::ui::outline_badge(format!(
                    "Update {}",
                    update.version
                )));
            }
            control =
                control.child(action("Check", !state.can_check()).on_click(
                    cx.listener(|this, _, window, cx| this.check_for_updates(window, cx)),
                ));
        } else {
            control = control.child(crate::ui::outline_badge("Dev build"));
        }
        let mut section = div().flex().flex_col().gap(px(16.)).child(
            settings_row("KubeCove version", description, control.into_any_element())
                .aria_label("KubeCove version"),
        );
        if !enabled {
            return section;
        }
        if let Some(update) = state.offered() {
            section = section.child(
                settings_row(
                    "Update available",
                    format!(
                        "KubeCove {} can be downloaded and installed now.",
                        update.version
                    ),
                    action("Install", false)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.install_update(window, cx)),
                        )
                        .into_any_element(),
                )
                .aria_label("Update available"),
            );
        }
        match &state.install {
            Install::NotStarted => {}
            Install::Downloading { progress, .. } => {
                let description = match progress.percent() {
                    Some(percent) => format!("Download progress: {percent}%."),
                    None => "Preparing installer.".to_owned(),
                };
                section = section.child(described_row(
                    "Installing update",
                    description,
                    secondary_badge("Working").into_any_element(),
                ));
            }
            Install::Installed(_) => {
                section = section.child(described_row(
                    "Relaunch required",
                    "Update is installed. Relaunch KubeCove to finish.".to_owned(),
                    action("Relaunch", false)
                        .on_click(cx.listener(|this, _, _, cx| this.relaunch_after_update(cx)))
                        .into_any_element(),
                ));
            }
        }
        if let Some(error) = &state.error {
            section = section.child(
                settings_row(
                    "Update check failed",
                    error.clone(),
                    action("Retry", false)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.check_for_updates(window, cx)),
                        )
                        .into_any_element(),
                )
                .aria_label(error.clone()),
            );
        }
        section
    }
}

/// A settings row named by its title and description, so a reader hears both.
fn described_row(
    title: &str,
    description: String,
    control: gpui_kit::AnyElement,
) -> super::settings_page::Observed {
    let label = format!("{title}. {description}");
    settings_row(title, description, control).aria_label(label)
}
