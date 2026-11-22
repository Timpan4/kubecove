//! `ResourceLiveStatusMenu.svelte`: the live-updates control and its popover,
//! and ResourceBrowser.svelte's stale-data notice and expandable refresh error.
use super::{
    Inspector, Page,
    overlay_motion::Overlay,
    resource_observation::NoticeFreshness,
    surface,
    watch_status::{Connection, LiveStatus, WatchLink},
};
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{
    AnyElement, Context, Div, MouseButton, SharedString, deferred, div, prelude::*, px,
};
use std::time::Duration;

const STALE_NOTICE_DELAY: Duration = Duration::from_millis(500);
/// The notice for watches that open, recover or catch up.
const STALE: &str =
    "Showing last observed state. Activity indicators are paused until live updates recover.";
const MAP_STALE: &str =
    "Showing last observed graph. Map activity indicators are paused until the map refreshes.";

fn notice_text(status: LiveStatus, stale: bool) -> Option<&'static str> {
    match status {
        LiveStatus::RefreshFailed => Some(
            "Refresh failed. Showing last observed state. Activity indicators are paused until live updates recover.",
        ),
        LiveStatus::PollingLimited => Some(
            "Live updates are unavailable. Readable resources refresh periodically. Activity indicators are paused.",
        ),
        LiveStatus::LiveLimited => Some(
            "Live updates are limited. Readable resources without watch access refresh periodically. Activity indicators are paused.",
        ),
        _ if stale => Some(STALE),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NoticeSurface {
    Table,
    Map,
}

fn waiting_for_watch(status: Option<LiveStatus>) -> bool {
    matches!(
        status,
        Some(LiveStatus::Connecting | LiveStatus::Reconnecting | LiveStatus::CatchingUp)
    )
}

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
            refresh_error: if self.page == Page::Resources {
                self.visible_refresh_error().map(str::to_owned)
            } else {
                self.live.refresh_error.clone()
            },
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
        if let Some(shown) = self.overlay_motion.sample(Overlay::LiveStatus, open) {
            control =
                control.child(deferred(shown.surface(self.live_menu(status, cx))).with_priority(1));
        }
        control
    }

    /// `PopoverContent align="end" class="w-72 space-y-3 p-3"`, 4 px below the trigger.
    fn live_menu(
        &self,
        status: LiveStatus,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + Styled + use<> {
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
        let busy = self.observation.refreshing();
        let (icon, label) = if refreshing {
            (ui::SPINNER, "Refreshing…")
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

    pub(super) fn live_notice(
        &self,
        freshness: NoticeFreshness,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let (table_error, map_error) = self.notice_read_errors(&freshness);
        let shared_error = self.shared_read_error(table_error, map_error);
        let table_pending = waiting_for_watch(freshness.table);
        let map_pending = if shared_error {
            matches!(
                freshness.map_watch,
                Some(LiveStatus::Connecting | LiveStatus::Reconnecting)
            )
        } else {
            map_error.is_none() && waiting_for_watch(freshness.map)
        };
        let table_shown = self
            .loading
            .phase_after("table-live-notice", table_pending, STALE_NOTICE_DELAY)
            .shown();
        let map_shown = self
            .loading
            .phase_after("map-live-notice", map_pending, STALE_NOTICE_DELAY)
            .shown();
        let only_table_stale = table_pending && table_shown && !map_pending;
        let only_map_stale = map_pending && map_shown && !table_pending;
        let table = freshness
            .table
            .and_then(|status| notice_text(status, table_shown && !only_map_stale));
        let map_stale = map_shown && !only_table_stale;
        let map = freshness.map.and_then(|status| {
            if status == LiveStatus::RefreshFailed {
                Some(MAP_STALE)
            } else if map_error.is_some() && !shared_error {
                Some("Refresh failed. Showing last observed state. Map activity indicators are paused until live updates recover.")
            } else if shared_error {
                freshness.map_watch.and_then(|watch| {
                    notice_text(watch, map_stale)
                }).filter(|_| freshness.map_watch != Some(LiveStatus::RefreshFailed))
            } else {
                notice_text(status, map_stale)
            }
        });
        let mut notices = Vec::new();
        if let Some(label) = table {
            let label = if freshness.map.is_some() && !shared_error && map != table {
                SharedString::from(
                    label.replace("Activity indicators", "Table activity indicators"),
                )
            } else {
                SharedString::from(label)
            };
            notices.push(self.live_notice_panel(
                NoticeSurface::Table,
                "live-notice",
                label,
                table_error,
                cx,
            ));
        }
        if let Some(label) = map.filter(|_| map != table) {
            let label = if freshness.table.is_some() {
                SharedString::from(label.replace("Activity indicators", "Map activity indicators"))
            } else {
                SharedString::from(label)
            };
            let id = if table.is_some() {
                "map-live-notice"
            } else {
                "live-notice"
            };
            let error = map_error
                .filter(|_| !shared_error && freshness.map != Some(LiveStatus::RefreshFailed));
            notices.push(self.live_notice_panel(NoticeSurface::Map, id, label, error, cx));
        }
        notices
    }

    fn shared_read_error(&self, table: Option<&str>, map: Option<&str>) -> bool {
        table.zip(map).is_some_and(|(table, map)| {
            std::ptr::eq(table, map)
                || (self
                    .live
                    .refresh_error
                    .as_deref()
                    .is_some_and(|error| std::ptr::eq(error, table))
                    && self
                        .observation
                        .context_scope_error(self.map_context())
                        .is_some_and(|error| std::ptr::eq(error, map)))
        })
    }

    pub(super) fn reset_hidden_diagnostic(&mut self, freshness: &NoticeFreshness) {
        if let Some(surface) = self.live.error_open {
            let (table, map) = self.notice_read_errors(freshness);
            let shown = match surface {
                NoticeSurface::Table => table.is_some(),
                NoticeSurface::Map => {
                    map.is_some()
                        && freshness.map != Some(LiveStatus::RefreshFailed)
                        && !self.shared_read_error(table, map)
                }
            };
            if !shown {
                self.live.error_open = None;
            }
        }
    }

    fn live_notice_panel(
        &self,
        surface: NoticeSurface,
        id: &'static str,
        label: SharedString,
        error: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = self.live.error_open == Some(surface);
        div()
            .id(id)
            .test_support()
            .aria_label(label.clone())
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
                    .when(error.is_some(), |row| {
                        row.child(ui::icon(
                            if expanded {
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
            .when(error.is_some(), |notice| {
                notice
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.live.error_open =
                            (this.live.error_open != Some(surface)).then_some(surface);
                        cx.notify();
                    }))
            })
            .when(error.is_some() && expanded, |notice| {
                notice.child(
                    div()
                        .px(px(12.))
                        .pb(px(12.))
                        .children(error.map(|message| ui::xs(message.to_owned()))),
                )
            })
            .into_any_element()
    }
}
impl Inspector {
    /// The notice for workspace contexts still reading their rows while other
    /// contexts' rows show, delayed like other loading indicators, as
    /// `contextsLoadingNotice` in `contextResults.ts` words it.
    pub(super) fn contexts_waiting_notice(&self) -> Option<AnyElement> {
        let waiting = self.observation.waiting_contexts();
        let phase = self.loading.phase("contexts-waiting", !waiting.is_empty());
        if !phase.shown() || waiting.is_empty() {
            return None;
        }
        let names = waiting
            .iter()
            .map(|context| format!("\"{context}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let plural = if waiting.len() > 1 { "s" } else { "" };
        let label = format!(
            "Waiting for context{plural} {names}. Showing resources from the contexts that answered."
        );
        Some(
            div()
                .id("contexts-waiting")
                .test_support()
                .role(gpui_kit::Role::Status)
                .aria_label(label.clone())
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::spinner(14., Tokens::muted_foreground()))
                .child(ui::muted(ui::xs(label)))
                .into_any_element(),
        )
    }
}
