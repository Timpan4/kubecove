//! The window shell around the page: per-frame motion bookkeeping, the
//! sidebar, the header, notices and the detail pane.
use super::detail::Detail;
use super::{Inspector, Page, sidebar};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{Context, MouseButton, MouseMoveEvent, Window, div, prelude::*, px};

impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::frame_profile::scope("inspector", "render");
        #[cfg(any(debug_assertions, feature = "topology-spike"))]
        if let Some(spike) = self.topology_spike_view(cx) {
            return spike;
        }
        let started = std::time::Instant::now();
        self.loading
            .begin_frame(cx.background_executor().now(), cx.reduce_motion());
        self.overlay_motion
            .begin_frame(cx.background_executor().now(), cx.reduce_motion());
        self.row_changes.begin_frame();
        crate::control_motion::Moving::begin_frame(cx);
        self.sync_argo_scope(window, cx);
        self.sync_scope_reread(window, cx);
        self.sync_remote_status(window, cx);
        self.sync_sidebar_discovery(cx);
        self.sync_live(window, cx);
        self.flush_live_patches(cx);
        // `xl` in the web shell: the sidebar is in flow only at this width or wider.
        let compact = window.viewport_size().width < px(sidebar::COMPACT_WIDTH);
        if !compact {
            self.navigation_open = false;
        }
        self.restore_remembered_detail(window, cx);
        let detail_open = !matches!(self.detail, Detail::Closed);
        // Without an open workspace the web renders the launcher and settings only, so the shell's
        // sidebar, drawer and workspace pages stay out of the tree.
        let workspace = self.workspace_open();
        let page = if workspace || self.page == Page::Settings {
            self.page_view(window, cx)
        } else {
            div().into_any_element()
        };
        let overlays = self.overlays(window, cx);
        let sidebar = (workspace && !compact).then(|| {
            crate::frame_profile::element(
                "sidebar",
                self.sidebar(window, cx, sidebar::WIDE_WIDTH)
                    .into_any_element(),
            )
        });
        let drawer = self.navigation_drawer(workspace && compact, window, cx);
        self.drop_closed_namespace_kinds();
        let probe = self.presentation_frame(started, cx.entity_id());
        let foreground_bar = self.foreground_bar(window, cx);
        let view = div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    this.record_key(event, window, cx);
                    if event.keystroke.key != "escape" {
                        return;
                    }
                    if this.navigation_open {
                        this.navigation_open = false;
                        cx.stop_propagation();
                        cx.notify();
                    } else if this.close_argo_option_popovers(window, cx) {
                        cx.stop_propagation();
                        cx.notify();
                    } else if this.sessions.popover || this.update_panel.open {
                        this.sessions.popover = false;
                        this.close_update_panel();
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .flex()
            .gap(px(8.))
            .px(px(8.))
            // The web's 2px in-flow `ForegroundLoadingBar` sits above its `h-screen p-2`
            // shell, shifting it down 2px and clipping 2px of bottom padding.
            .pt(px(10.))
            .pb(px(6.))
            .bg(Tokens::background())
            .text_color(Tokens::foreground())
            .font_family(crate::theme::SANS)
            .child(foreground_bar)
            .children(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::background())
                    .child(self.header(cx, compact))
                    .when_some(
                        self.persistence_error.clone().or(self.notice.clone()),
                        |main, notice| {
                            main.child(
                                div()
                                    .id("notice")
                                    .test_support()
                                    .aria_label(notice.clone())
                                    .role(gpui_kit::Role::Alert)
                                    .px(px(16.))
                                    .py(px(8.))
                                    .border_b_1()
                                    .border_color(Tokens::border())
                                    .child(ui::xs(notice)),
                            )
                        },
                    )
                    .children(self.restore_prompt(cx))
                    .children(self.session_cleanup_notice(cx))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .id("page")
                                    .test_support()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .overflow_y_scroll()
                                    .child(crate::frame_profile::element("page", page)),
                            )
                            .when(detail_open, |row| {
                                row.on_mouse_move(cx.listener(
                                    |this, event: &MouseMoveEvent, window, cx| {
                                        this.drag_detail_pane(event, window, cx);
                                    },
                                ))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, _| this.detail_dragging = false),
                                )
                                .children(self.detail_pane(window, cx))
                            }),
                    )
                    .children(self.usage_footer(cx)),
            )
            .children(overlays)
            .children(drawer)
            .children(self.copy_toast(cx))
            .children(probe);
        let fading = self.loading.end_frame(cx);
        if self.row_changes.end_frame() || fading {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
        self.end_overlay_motion_frame(window, cx);
        self.record_load_frames(window, started.elapsed());
        view
    }
}
