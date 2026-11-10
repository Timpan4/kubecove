//! `ResourceLiveStatusMenu.svelte`: the live-updates control and its popover,
//! and ResourceBrowser.svelte's stale-data notice and expandable refresh error.
use super::{
    Inspector, Page, surface,
    watch_status::{Connection, LiveStatus, WatchLink},
};
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, MouseButton, deferred, div, prelude::*, px};

/// The notice for watches that open, recover or catch up.
const STALE: &str =
    "Showing last observed state. Activity indicators are paused until live updates recover.";

const REFRESH_HINT: &str =
    "Clear cached data for this view and fetch it again. Filters and selection are preserved.";

/// One watch, as the status menu reads it.
pub(super) struct WatchReport<'a> {
    pub context: &'a str,
    pub link: WatchLink,
    /// The stream's last error.
    pub error: Option<&'a str>,
    /// Why watch access is denied.
    pub denied: Option<&'a str>,
    /// Why the watch's last automatic reload failed.
    pub reload_error: Option<&'a str>,
}

/// What the status menu lists for a view, as `ResourceLiveStatusMenu.svelte`
/// reads it from `LiveViewState`.
#[derive(Default)]
struct LiveDetails {
    message: String,
    /// The first stream error.
    connection_error: Option<String>,
    /// Each denied watch, once.
    warnings: Vec<String>,
    /// The failure of the menu's own refresh.
    refresh_error: Option<String>,
    /// The failure of an automatic reload, which retries.
    reload_error: Option<String>,
}

/// The menu's content for the watches of `contexts`, in rank order. With
/// several contexts the message is the worst context's and every line names
/// its context, as `combineLiveViewStates` does.
fn details(contexts: &[String], reports: &[WatchReport<'_>]) -> LiveDetails {
    let several = contexts.len() > 1;
    let named = |context: &str, text: &str| {
        if several {
            format!("{context}: {text}")
        } else {
            text.to_owned()
        }
    };
    let mut details = LiveDetails::default();
    let mut worst: Option<(Connection, &str)> = None;
    for context in contexts {
        let watches = || reports.iter().filter(|watch| watch.context == context);
        let connection = Connection::of(watches().map(|watch| watch.link));
        if connection != Connection::Idle
            && worst.is_none_or(|(other, _)| connection.severity() < other.severity())
        {
            worst = Some((connection, context));
        }
        for watch in watches() {
            if details.connection_error.is_none() {
                details.connection_error = watch.error.map(|error| named(context, error));
            }
            if let Some(denied) = watch.denied.map(|denied| named(context, denied))
                && !details.warnings.contains(&denied)
            {
                details.warnings.push(denied);
            }
            if details.reload_error.is_none() {
                details.reload_error = watch.reload_error.map(|error| named(context, error));
            }
        }
    }
    details.message = match worst {
        Some((connection, context)) => named(context, connection.message()),
        None => Connection::Idle.message().to_owned(),
    };
    details
}

impl Inspector {
    fn live_details(&self) -> LiveDetails {
        if self.page == Page::GitOps {
            let context = self.scope.context.as_str();
            let reports: Vec<_> = self
                .page_refresh
                .links()
                .map(|link| WatchReport {
                    context,
                    link,
                    error: None,
                    denied: None,
                    reload_error: None,
                })
                .collect();
            return LiveDetails {
                reload_error: self.gitops.error().map(|error| error.message.clone()),
                ..details(std::slice::from_ref(&self.scope.context), &reports)
            };
        }
        LiveDetails {
            refresh_error: self.live.refresh_error.clone(),
            ..details(&self.watch_contexts(), &self.live.reports())
        }
    }

    pub(super) fn live_control(&self, cx: &mut Context<Self>) -> Div {
        let status = self.live_status();
        let open = self.live_menu_open;
        let trigger = ui::outline_button()
            .id("live-status")
            .test_support()
            .aria_label("Live updates and refresh options")
            .cursor_pointer()
            // `ResourceLiveStatusMenu.svelte`: `h-8 gap-2 bg-background/50 px-2.5`.
            .h(px(32.))
            .px(px(10.))
            .gap(px(8.))
            .bg(Tokens::background().opacity(0.5))
            .child(div().size(px(6.)).rounded_full().bg(match status {
                LiveStatus::Live => super::events::tw::emerald400(1.),
                LiveStatus::RefreshFailed => Tokens::destructive(),
                LiveStatus::CatchingUp => super::events::tw::sky300(),
                LiveStatus::Connecting | LiveStatus::NotConnected => Tokens::muted_foreground(),
                LiveStatus::Reconnecting
                | LiveStatus::LiveLimited
                | LiveStatus::PollingLimited
                | LiveStatus::Refreshing => super::events::tw::amber400(1.),
            }))
            .child(
                ui::xs(status.label())
                    .text_color(match status {
                        LiveStatus::RefreshFailed => Tokens::destructive(),
                        LiveStatus::Reconnecting
                        | LiveStatus::PollingLimited
                        | LiveStatus::LiveLimited => Palette::amber300(),
                        _ => Tokens::muted_foreground(),
                    })
                    .id("live-label")
                    .test_support()
                    .aria_label(status.label()),
            )
            .child(ui::icon("chevron-down", 12., Tokens::muted_foreground()))
            // The popover closes on a press outside it, the trigger included,
            // so the trigger sets the state it had when it rendered.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.live_menu_open = !open;
                    cx.notify();
                }),
            );
        let mut control = div().relative().child(trigger);
        if open {
            control = control.child(deferred(self.live_menu(status, cx)).with_priority(1));
        }
        control
    }

    /// `PopoverContent align="end" class="w-72 space-y-3 p-3"`, 4 px below the trigger.
    fn live_menu(&self, status: LiveStatus, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let details = self.live_details();
        let amber = |text: String| ui::xs(text).text_color(Palette::amber300());
        let alert = |id: &'static str, text: String| {
            ui::xs(text.clone())
                .text_color(Tokens::destructive())
                .id(id)
                .test_support()
                .role(gpui_kit::Role::Alert)
                .aria_label(text)
        };
        // The delayed indicator, as the web button shows it; the read itself
        // disables the button at once.
        let refreshing = status == LiveStatus::Refreshing;
        let busy = self.scope_load.refreshing();
        let (icon, label) = if refreshing {
            ("loader-circle", "Refreshing…")
        } else {
            ("refresh-cw", "Refresh (clear cache)")
        };
        let button = surface::small_button(Some(icon), label)
            .id("refresh")
            .test_support()
            .aria_label(label)
            .aria_description(if busy { "disabled" } else { "enabled" })
        .w_full()
        .justify_center();
        let button = if busy {
            surface::disabled(button)
        } else {
            button
                .cursor_pointer()
                .hover(|button| button.bg(Tokens::muted()))
                .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx)))
        };
        div()
            .id("live-menu")
            .test_support()
            .occlude()
            .absolute()
            .top(px(36.))
            .right_0()
            .w(px(288.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::surface2())
            .text_color(Tokens::foreground())
            .shadow_xl()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.live_menu_open = false;
                cx.notify();
            }))
            .child(
                ui::medium(ui::xs("Current view"))
                    .id("live-menu-title")
                    .test_support()
                    .aria_label("Current view"),
            )
            .child(
                ui::muted(ui::text(details.message.clone(), 12., 19.5))
                    .id("live-message")
                    .test_support()
                    .role(gpui_kit::Role::Status)
                    .aria_label(details.message),
            )
            .children(details.connection_error.map(|error| {
                amber(error.clone())
                    .id("live-connection-error")
                    .test_support()
                    .aria_label(error)
            }))
            .when(!details.warnings.is_empty(), |menu| {
                menu.child(amber(
                    "Some data or live updates are unavailable with this account's permissions. Permitted resources remain usable."
                        .into(),
                ))
                .child(div().flex().flex_col().gap(px(4.)).children(
                    details.warnings.into_iter().enumerate().map(|(index, warning)| {
                        amber(warning.clone())
                            .id(("live-warning", index))
                            .test_support()
                            .aria_label(warning)
                    }),
                ))
            })
            .children(
                details
                    .refresh_error
                    .map(|error| alert("live-refresh-error", format!("Refresh failed: {error}"))),
            )
            .children(details.reload_error.map(|error| {
                alert(
                    "live-reload-error",
                    format!("Automatic reload failed: {error}. Retrying."),
                )
            }))
            .child(
                ui::muted(ui::text(REFRESH_HINT, 12., 19.5))
                    .id("live-refresh-hint")
                    .test_support()
                    .aria_label(REFRESH_HINT),
            )
            .child(button)
    }

    pub(super) fn live_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let status = self.resource_live_status();
        // Watches that connect or recover within the show delay show no notice,
        // and a shown notice stays for the minimum visible time, as loading
        // indicators do. Failed and limited states are not transient.
        let stale = self.loading.phase(
            "live-notice",
            matches!(
                status,
                LiveStatus::Connecting | LiveStatus::Reconnecting | LiveStatus::CatchingUp
            ),
        );
        let failed = status == LiveStatus::RefreshFailed;
        let label = match status {
            LiveStatus::RefreshFailed => {
                "Refresh failed. Showing last observed state. Activity indicators are paused until live updates recover."
            }
            LiveStatus::PollingLimited => {
                "Live updates are unavailable. Readable resources refresh periodically. Activity indicators are paused."
            }
            LiveStatus::LiveLimited => {
                "Live updates are limited. Readable resources without watch access refresh periodically. Activity indicators are paused."
            }
            _ if stale.shown() => STALE,
            _ => return None,
        };
        Some(
            div()
                .id("live-notice")
                .test_support()
                .aria_label(label)
                .role(gpui_kit::Role::Status)
                .flex()
                .flex_col()
                .flex_none()
                .rounded(px(6.))
                .border_1()
                .border_color(Palette::amber500(0.3))
                .text_color(Palette::amber300())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(12.))
                        .py(px(8.))
                        .when(failed, |row| {
                            row.child(ui::icon(
                                if self.live.error_open {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                },
                                12.,
                                Palette::amber300(),
                            ))
                        })
                        .child(ui::xs(label)),
                )
                .when(failed, |notice| {
                    notice
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.live.error_open = !this.live.error_open;
                            cx.notify();
                        }))
                })
                .when(failed && self.live.error_open, |notice| {
                    notice.child(
                        div().px(px(12.)).pb(px(12.)).children(
                            self.live
                                .messages()
                                .next()
                                .map(|message| ui::xs(message.clone())),
                        ),
                    )
                })
                .into_any_element(),
        )
    }
}
