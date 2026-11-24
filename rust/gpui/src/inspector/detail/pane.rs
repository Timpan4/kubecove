//! The detail pane: its resize separator, header, tab strip and open tab.
use super::*;

impl Inspector {
    /// Moves the pane's edge with the pointer while the separator is held.
    pub(in crate::inspector) fn drag_detail_pane(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_dragging {
            return;
        }
        if !event.dragging() {
            self.detail_dragging = false;
            return;
        }
        let percent = self.detail_percent_at(window, f32::from(event.position.x));
        self.set_detail_percent(percent);
        cx.notify();
    }

    /// `DetailPanelFrame.svelte`: the `w-2` resize separator (a centered
    /// `w-px bg-border/70` line under an `h-8 w-1` grip), then the `aside`.
    /// Pointer drag and the arrow, Home and End keys resize it.
    pub(in crate::inspector) fn detail_pane(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> [AnyElement; 2] {
        let handle = div()
            .id("detail-resize")
            .test_support()
            .track_focus(&self.detail_resize)
            .tab_stop(true)
            .role(gpui_kit::Role::Splitter)
            .cursor_col_resize()
            .aria_label("Resize details panel")
            .aria_numeric_value(f64::from(
                super::super::resources::detail_percent(self).round(),
            ))
            // Mirrors the numeric value as text, the only value the parity driver can read.
            .aria_value(format!(
                "{}",
                super::super::resources::detail_percent(self).round()
            ))
            .focus_visible(|style| style.bg(Tokens::border().opacity(0.5)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.detail_dragging = true),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.resize_detail_key(event.keystroke.key.as_str()) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(8.))
            .h_full()
            .child(div().w(px(1.)).h_full().bg(Tokens::border().opacity(0.7)))
            .child(
                div()
                    .absolute()
                    .w(px(4.))
                    .h(px(32.))
                    .rounded_full()
                    .bg(Tokens::border())
                    .opacity(0.7),
            );
        let aside = div()
            .flex_none()
            .w(px(super::super::resources::detail_width(self, window)))
            .h_full()
            .border_l_1()
            .border_color(Tokens::border())
            // `ResourceInspector.svelte`: `bg-surface-1` (= card).
            .bg(Tokens::card())
            .child(self.detail_view(cx));
        // The pane renders only while a resource is open, so the track starts
        // with the pane and selecting another row does not replay the enter.
        let aside = match self
            .overlay_motion
            .sample(super::super::overlay_motion::Overlay::Detail, true)
        {
            Some(shown) => shown.surface(aside).into_any_element(),
            None => aside.into_any_element(),
        };
        [
            handle.into_any_element(),
            crate::frame_profile::element("detail", aside),
        ]
    }

    pub(in crate::inspector) fn detail_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let (name, subtitle) = match &self.detail {
            Detail::Closed => return div().into_any_element(),
            Detail::Loading(loading) => {
                (loading.resource.name.clone(), loading.resource.kind.clone())
            }
            Detail::Ready(detail) => {
                let summary = &detail.data.summary;
                (
                    summary.name.clone(),
                    format!(
                        "{} / {}",
                        summary.kind,
                        summary.namespace.as_deref().unwrap_or("Cluster")
                    ),
                )
            }
            Detail::Failed(_) => ("Resource".into(), String::new()),
        };
        let active = match &self.detail {
            Detail::Ready(detail) => Some(detail.tab),
            Detail::Loading(loading) => loading.tab,
            _ => None,
        };
        let resource = self.detail.resource().cloned();
        // With several workspace contexts the header names the row's context.
        let context_badge = resource
            .as_ref()
            .filter(|_| self.workspace_contexts().len() > 1)
            .map(|resource| {
                ui::outline_badge(resource.cluster.clone())
                    .flex_none()
                    .id("detail-context")
                    .test_support()
                    .aria_label(format!("Context: {}", resource.cluster))
            });
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(8.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .min_w_0()
                    .child(ui::semibold(ui::sm(name)).truncate())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .min_w_0()
                            .child(ui::muted(ui::xs(subtitle)).truncate())
                            .children(context_badge),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(ui::ghost_icon("pin", 24., 14.))
                    .child(
                        ui::ghost_icon("x", 24., 14.)
                            .id("close-detail")
                            .test_support()
                            .aria_label("Close resource details")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_detail(cx))),
                    ),
            );
        // `TabsList variant="line" class="flex-wrap"`: an `h-8 p-[3px] gap-1` row of
        // 25px `px-1.5 text-xs font-medium` triggers; narrow panes wrap tabs.
        // `TabsTrigger.svelte`: one line under the row slides to the active tab.
        let line = control_motion::Slide::new();
        let tabs = div()
            .relative()
            .flex()
            .flex_wrap()
            .items_center()
            .min_h(px(32.))
            .p(px(3.))
            .gap(px(4.))
            .child(
                line.indicator(
                    "tab-line",
                    TABS.iter()
                        .position(|(tab, ..)| active == Some(*tab))
                        .unwrap_or_default(),
                    Tokens::foreground(),
                )
                .line(px(2.), px(2.)),
            )
            .children(
                TABS.iter()
                    .filter(|(tab, ..)| {
                        resource
                            .as_ref()
                            .is_none_or(|resource| available(*tab, resource))
                    })
                    .map(|(tab, id, label)| {
                        let selected = active == Some(*tab);
                        let tab = *tab;
                        let trigger = div()
                            .id(*id)
                            .test_support()
                            .role(gpui_kit::Role::Tab)
                            .aria_selected(selected)
                            .relative()
                            .flex()
                            .items_center()
                            .h(px(25.))
                            .px(px(6.))
                            .cursor_pointer()
                            .child(ui::medium(ui::xs(*label)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_tab(tab, window, cx)
                            }));
                        // The text follows `transition-[color,…]`.
                        let trigger = control_motion::eased(
                            gpui_kit::SharedString::from(format!("tab:{id}")),
                            selected,
                            move |on| {
                                let foreground = Tokens::foreground();
                                trigger.text_color(control_motion::mix(
                                    foreground.opacity(0.6),
                                    foreground,
                                    on,
                                ))
                            },
                        );
                        line.option(selected, trigger)
                    }),
            );
        let loading = self
            .loading
            .phase("detail", matches!(self.detail, Detail::Loading(_)));
        let body: AnyElement =
            match &self.detail {
                Detail::Closed => unreachable!("closed detail returned early"),
                _ if loading.shown() => {
                    ui::muted(ui::xs("Loading resource details...")).into_any_element()
                }
                Detail::Loading(_) => div().into_any_element(),
                _ if !loading.ready() => div().into_any_element(),
                Detail::Failed(failed) => {
                    let permission = friendly_error::resource_permission(&failed.resource, "get")
                        .map(|target| feedback::Permission {
                            cluster: failed.resource.cluster.clone(),
                            target,
                        });
                    self.friendly_failure(
                        "detail-error",
                        Some(failed.error.kind),
                        &failed.error.message,
                        friendly_error::Context {
                            operation: Some(friendly_error::Operation::Details),
                            fallback_title: Some("Failed to load details"),
                            ..Default::default()
                        },
                        permission,
                        cx,
                    )
                }
                Detail::Ready(detail) => {
                    let summary = &detail.data.summary;
                    let identity = format!(
                        "{} {} {}",
                        summary.kind,
                        summary.namespace.as_deref().unwrap_or("Cluster"),
                        summary.name
                    );
                    let content: AnyElement = match detail.tab {
                        DetailTab::Details => details_tab(
                            detail,
                            &self.timeline,
                            self.persisted.preferences.timestamp_timezone,
                            &self.loading,
                            cx,
                        )
                        .into_any_element(),
                        DetailTab::Forward => self.forward_tab(cx).into_any_element(),
                        DetailTab::Exec => self.exec_view(cx),
                        DetailTab::Events => events_tab(
                            detail,
                            self.persisted.preferences.timestamp_timezone,
                            &self.loading,
                            cx,
                        ),
                        DetailTab::Logs => logs_tab(
                            detail,
                            self.persisted.preferences.log_layout,
                            &self.loading,
                            &self.overlay_motion,
                            cx,
                        ),
                        DetailTab::Revisions => revisions_tab(detail, &self.loading, cx),
                        DetailTab::Argo => self.argo_detail_tab(detail, cx),
                        DetailTab::Actions => actions_tab(detail, &self.loading, cx),
                        DetailTab::Yaml => yaml_tab(self, detail, &self.persisted.preferences, cx)
                            .into_any_element(),
                    };
                    let content = crate::frame_profile::element(
                        match detail.tab {
                            DetailTab::Details => "detail.details",
                            DetailTab::Forward => "detail.forward",
                            DetailTab::Exec => "detail.exec",
                            DetailTab::Events => "detail.events",
                            DetailTab::Logs => "detail.logs",
                            DetailTab::Revisions => "detail.revisions",
                            DetailTab::Argo => "detail.argo",
                            DetailTab::Actions => "detail.actions",
                            DetailTab::Yaml => "detail.yaml",
                        },
                        content,
                    );
                    // The whole body replaces the detail indicator; a tab's
                    // content replaces that tab's own.
                    let reveal = self.loading.reveal("detail").or_else(|| {
                        self.loading.reveal(match detail.tab {
                            DetailTab::Events => "events",
                            DetailTab::Revisions => "revisions",
                            _ => return None,
                        })
                    });
                    div()
                        .flex()
                        .flex_col()
                        .when_else(
                            detail.tab == DetailTab::Yaml,
                            |body| body.flex_grow_1().flex_shrink_0(),
                            |body| body.flex_1().min_h_0(),
                        )
                        .when_some(reveal, |body, opacity| body.opacity(opacity))
                        .when(
                            matches!(
                                detail.tab,
                                DetailTab::Details | DetailTab::Events | DetailTab::Argo
                            ),
                            |body| body.flex_none(),
                        )
                        .child(
                            div()
                                .id("detail-identity")
                                .test_support()
                                .aria_label(identity)
                                .absolute()
                                .size_0(),
                        )
                        .when(
                            matches!(detail.tab, DetailTab::Details | DetailTab::Events),
                            |body| {
                                body.children(
                                    [
                                        (
                                            "events-watch-warning",
                                            "Events watch failed",
                                            &detail.events.watch_error,
                                        ),
                                        (
                                            "events-refresh-warning",
                                            "Events refresh failed",
                                            &detail.events.refresh_error,
                                        ),
                                    ]
                                    .into_iter()
                                    .filter_map(
                                        |(id, heading, error)| {
                                            let message = SharedString::from(format!(
                                                "{heading}: {}",
                                                error.as_ref()?
                                            ));
                                            Some(
                                                ui::muted(ui::xs(message.clone()))
                                                    .id(id)
                                                    .test_support()
                                                    .aria_label(message),
                                            )
                                        },
                                    ),
                                )
                            },
                        )
                        .child(content)
                        .into_any_element()
                }
            };
        let scroll = match &self.detail {
            Detail::Ready(detail) => Some(&detail.scroll),
            _ => None,
        };
        div()
            .id("detail")
            .flex()
            .flex_col()
            .size_full()
            // `ResourceInspector.svelte`: `section bg-surface-1`.
            .bg(Tokens::card())
            .child(header)
            // Tabs scroll with the body, as inside the web's `p-4` scroller (`Tabs gap-3`).
            .child(
                div()
                    .id("detail-body")
                    .test_support()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when_some(scroll, |body, scroll| body.track_scroll(scroll))
                    // The web capture shows content 8px from the panel's right
                    // edge despite `p-4`; the rest of the inset is the panel frame.
                    .pt(px(16.))
                    .pb(px(16.))
                    .pl(px(16.))
                    .pr(px(8.))
                    .gap(px(12.))
                    .child(tabs)
                    .child(body),
            )
            .into_any_element()
    }
}
