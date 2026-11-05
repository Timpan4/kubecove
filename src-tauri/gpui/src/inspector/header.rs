//! Workspace header, mirroring `WorkspaceHeader.svelte` and `ClusterSelector.svelte`.

use super::{Inspector, Page};
use crate::theme::{MONO, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::select::Select;
use gpui_kit::{Context, FontWeight, div, prelude::*, px};

impl Inspector {
    /// `compact` shows the menu button that opens the navigation drawer.
    pub(super) fn header(&self, cx: &mut Context<Self>, compact: bool) -> impl IntoElement {
        let dirty = self.dirty(cx);
        let location = match self.page {
            Page::Overview => self.workspace_name.clone(),
            Page::Resources | Page::ClusterOverview => "Resources".into(),
            Page::Incidents => "Incident Cockpit".into(),
            Page::PortForwards => "Port Forwards".into(),
            Page::Rbac => self.rbac.location().into(),
            Page::Settings => "Settings".into(),
            Page::GitOps => self.gitops_location().into(),
            Page::Helm => "Helm Releases".into(),
            Page::Namespaces => "Namespaces".into(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.))
            .h(px(48.))
            .px(px(16.))
            .border_b_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .when(compact, |bar| {
                bar.child(
                    ui::ghost_icon("menu", 28., 16.)
                        .id("open-navigation")
                        .test_support()
                        .aria_label("Open workspace navigation")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigation_open = true;
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                ui::muted(ui::xs("CLUSTER CONTEXT:"))
                                    .font_weight(FontWeight::BOLD)
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Select::new(&self.context_picker)
                                    .id("context")
                                    .disabled(dirty)
                                    // `SelectTrigger` default size is `h-7`; the call site adds `text-xs`.
                                    .h(px(28.))
                                    .text_xs()
                                    .w_auto()
                                    .min_w(px(94.))
                                    .bg(Tokens::background().opacity(0.5)),
                            ),
                    )
                    .child(ui::icon("chevron-right", 14., Tokens::muted_foreground()))
                    .child(
                        ui::muted(ui::xs(location))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate(),
                    )
                    .when_some(self.application_workspace_name(), |row, name| {
                        row.child(ui::icon("chevron-right", 14., Tokens::muted_foreground()))
                            .child(ui::semibold(ui::xs(name.to_owned())).truncate())
                    }),
            )
            .when_some(
                self.settings.integrations.remote.as_ref(),
                |header, remote| {
                    let label = format!(
                        "Remote host: {}{}",
                        remote.server_url,
                        if remote.connected {
                            ""
                        } else {
                            " (disconnected)"
                        }
                    );
                    header.child(
                        ui::outline_badge(label.clone())
                            .id("remote-host-scope")
                            .test_support()
                            .aria_label(label)
                            .max_w(px(256.))
                            .truncate(),
                    )
                },
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .justify_end()
                    .children(self.permission_return(cx))
                    .gap(px(4.))
                    .when_some(
                        self.live_session_count().filter(|count| *count > 0),
                        |row, count| row.child(self.live_session_control(count, cx)),
                    )
                    .children(self.update_control(cx))
                    .child(
                        ui::ghost_icon("folder-open", 28., 14.)
                            .id("header-workspaces")
                            .test_support()
                            .aria_label("Open workspaces")
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_launcher(window, cx)),
                            ),
                    )
                    .child(
                        ui::ghost_icon("settings", 28., 14.)
                            .id("open-settings")
                            .test_support()
                            .aria_label("Open settings")
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                            ),
                    )
                    .child(
                        ui::outline_button()
                            .id("open-palette")
                            .test_support()
                            .aria_label("Search views, namespaces, and resources")
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_palette(window, cx)),
                            )
                            .h(px(24.))
                            .px(px(12.))
                            .gap(px(8.))
                            .bg(Tokens::card())
                            .child(ui::icon("search", 14., Tokens::muted_foreground()))
                            .child(ui::muted(ui::xs("Search resources...")))
                            .child(
                                div()
                                    .px(px(4.))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(Tokens::border())
                                    .bg(Tokens::muted())
                                    .child(ui::muted(
                                        ui::xs(super::palette::SHORTCUT_HINT).font_family(MONO),
                                    )),
                            ),
                    ),
            )
    }
}
