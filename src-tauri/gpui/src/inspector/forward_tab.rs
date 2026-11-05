//! `PortForwardTab.svelte`: `flex flex-col gap-3`, guarded target, port fields,
//! explicit Start, Service-only Save preset, and exact-target session cards.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
};
use crate::{theme::Tokens, ui};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    IndexPath,
    select::{SearchableVec, Select, SelectEvent, SelectState},
};
use gpui_kit::{AppContext, Context, Div, Entity, TestSupportExt, Window, div, prelude::*, px};
use kubecove_lib::models::PortForwardRequest;

pub(super) struct ForwardForm {
    pub remote: Entity<InputState>,
    pub local: Entity<InputState>,
    pub service_port: Option<Entity<SelectState<SearchableVec<gpui_kit::SharedString>>>>,
    _subscriptions: Vec<gpui_kit::Subscription>,
}
impl ForwardForm {
    pub fn new(
        yaml: &str,
        service: bool,
        window: &mut Window,
        cx: &mut Context<Inspector>,
    ) -> Self {
        let ports: Vec<gpui_kit::SharedString> = if service {
            serde_yaml::from_str::<serde_json::Value>(yaml)
                .ok()
                .and_then(|v| {
                    v["spec"]["ports"].as_array().map(|ports| {
                        ports
                            .iter()
                            .filter(|p| p["protocol"].as_str().is_none_or(|p| p == "TCP"))
                            .filter_map(|p| p["port"].as_u64())
                            .collect::<Vec<_>>()
                    })
                })
                .map(|mut ports| {
                    ports.sort_unstable();
                    ports.dedup();
                    ports.into_iter().map(|p| p.to_string().into()).collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let port = ports.first().cloned().unwrap_or_default();
        let remote = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("8080")
                .default_value(port)
        });
        let mut subscriptions = Vec::new();
        let service_port = (!ports.is_empty()).then(|| {
            let picker = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(ports),
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            });
            let input = remote.clone();
            subscriptions.push(
                cx.subscribe_in(&picker, window, move |_, _, event, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                    }
                    cx.notify();
                }),
            );
            picker
        });
        Self {
            remote,
            local: cx.new(|cx| InputState::new(window, cx).placeholder("auto")),
            service_port,
            _subscriptions: subscriptions,
        }
    }
}

fn field(label: &str, input: impl IntoElement, help: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::medium(ui::xs(label.to_owned())))
        .child(input)
        .child(ui::muted(ui::xs(help.to_owned())))
}

impl Inspector {
    pub(super) fn forward_request(&self, cx: &gpui_kit::App) -> Result<PortForwardRequest, String> {
        let Detail::Ready(detail) = &self.detail else {
            return Err("Choose an exact target".into());
        };
        let resource = &detail.data.summary;
        let form = &detail.forward;
        // `parsePortForwardForm`: TCP's u16 range and the web's unprivileged local port rule.
        let remote = form
            .remote
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("Remote port must be between 1 and 65535")?;
        let local = form.local.read(cx).value();
        let local = if local.trim().is_empty() {
            None
        } else {
            Some(
                local
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|p| *p >= 1024)
                    .ok_or("Local port must be between 1024 and 65535")?,
            )
        };
        Ok(PortForwardRequest {
            cluster_context: resource.cluster.clone(),
            kubeconfig_env_var: self.session_env_var(),
            namespace: resource.namespace.clone().ok_or("Namespace required")?,
            target_kind: Some(resource.kind.clone()),
            target_name: Some(resource.name.clone()),
            pod_name: None,
            remote_port: remote.into(),
            local_port: local.map(i64::from),
        })
    }

    pub(super) fn forward_tab(&self, cx: &mut Context<Self>) -> Div {
        let Detail::Ready(detail) = &self.detail else {
            return div();
        };
        let resource = &detail.data.summary;
        let service = resource.kind == "Service";
        let sessions: Vec<_> = self
            .sessions
            .load
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|sessions| {
                sessions
                    .for_context(&self.scope.context)
                    .0
                    .into_iter()
                    .filter(|s| {
                        s.target_kind == resource.kind
                            && s.target_name == resource.name
                            && Some(&s.namespace) == resource.namespace.as_ref()
                    })
                    .collect()
            })
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                ui::card()
                    .p(px(12.))
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(8.))
                    .child(ui::icon("cable", 16., Tokens::foreground()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(ui::medium(ui::xs("Guarded port-forward")))
                            .child(ui::muted(ui::xs(format!(
                                "Target: {}/{} ({})",
                                resource.namespace.as_deref().unwrap_or_default(),
                                resource.name,
                                resource.kind
                            )))),
                    ),
            )
            .child(field(
                if service { "Service port" } else { "Pod port" },
                if let Some(picker) = &detail.forward.service_port {
                    Select::new(picker)
                        .id("forward-service-port")
                        .h(px(28.))
                        .into_any_element()
                } else {
                    Input::new(&detail.forward.remote)
                        .h(px(28.))
                        .id("forward-remote")
                        .aria_label("Remote port")
                        .into_any_element()
                },
                if detail.forward.service_port.is_some() {
                    "TCP ports parsed from Service YAML."
                } else {
                    "Remote TCP port to forward."
                },
            ))
            .child(field(
                "Local port",
                Input::new(&detail.forward.local)
                    .h(px(28.))
                    .id("forward-local")
                    .aria_label("Local port"),
                "Optional. Must be 1024 or higher when set.",
            ))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(Variant::Default, 28., self.sessions.busy)
                            .id("forward-start")
                            .test_support()
                            .aria_label("Start")
                            .child(ui::icon("play", 14., Tokens::primary_foreground()))
                            .child(ui::xs(if self.sessions.busy {
                                "Starting..."
                            } else {
                                "Start"
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                match this.forward_request(cx) {
                                    Ok(request) => this.start_forward_request(request, window, cx),
                                    Err(e) => this.sessions.error = Some(e),
                                }
                            })),
                    )
                    .when(service, |row| {
                        row.child(
                            button(Variant::Outline, 28., false)
                                .id("forward-save")
                                .test_support()
                                .child(ui::xs("Save preset"))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_forward_preset(cx)),
                                ),
                        )
                    })
                    .child(ui::muted(ui::xs("Local URL appears after session starts."))),
            )
            .when_some(self.sessions.error.clone(), |body, error| {
                body.child(
                    div()
                        .id("forward-error")
                        .test_support()
                        .aria_label(error.clone())
                        .child(ui::xs(error).text_color(Tokens::destructive())),
                )
            })
            .child(ui::muted(ui::semibold(ui::xs("ACTIVE PORT-FORWARDS"))))
            .when(sessions.is_empty(), |body| {
                body.child(
                    ui::card()
                        .min_h(px(96.))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::medium(ui::sm("No port-forwards")))
                        .child(ui::muted(ui::xs(
                            "Start a guarded session for this target.",
                        ))),
                )
            })
            .children(sessions.into_iter().map(|s| self.forward_card(s, cx)))
    }
}
