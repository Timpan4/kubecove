//! Native controls for the same connection choices exposed by the Svelte dialog.
use super::{
    Inspector,
    argo_connection::{ConnectStep, Preference},
    controls::{Variant, button},
};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{
    Context, Div, Window,
    base::FocusTrapElement,
    component::input::{Input, Textarea},
    div,
    prelude::*,
    px, relative,
};
use kubecove_lib::models::ArgoConfirmedTarget;

impl Inspector {
    pub(super) fn open_argo_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo_connections.editing = None;
        self.argo_connections.endpoint = None;
        self.argo_connections.step.clear_preview();
        // The detail panel opens its own dialog. Inside the GitOps page dialog this
        // button only shows the form, so that dialog keeps its own choices.
        if !self.argo_connections.dialog {
            self.argo_connections.host = super::argo_connection::DialogHost::Detail;
        }
        self.clear_argo_secrets(window, cx);
        self.argo_connections.open = true;
        self.argo_connections.dialog = true;
        self.focus_argo_connection_dialog(window, cx);
        self.discover_argo_servers(window, cx);
        cx.notify();
    }
    /// Moves focus into the connection dialog once it is rendered, so its
    /// focus trap governs Tab from the first press.
    pub(super) fn focus_argo_connection_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.argo_connections.dialog_focus.clone();
        cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
    }
    pub(super) fn argo_connection_panel(&self, cx: &mut Context<Self>) -> Div {
        if self.argo_connections.dialog {
            return div();
        }
        self.argo_connection_content(cx)
    }
    pub(super) fn argo_connection_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let dialog_focus = self.argo_connections.dialog_focus.clone();
        self.argo_connections.dialog.then(|| {
            let overlay = div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Tokens::background().opacity(0.8))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation()
                })
                .child(
                    div()
                        .id("argo-connection-dialog")
                        .test_support()
                        .role(gpui_kit::Role::Dialog)
                        .aria_label("Argo CD connection")
                        .track_focus(&dialog_focus)
                        .w(px(672.))
                        .max_h(relative(0.9))
                        .overflow_y_scroll()
                        .on_key_down(cx.listener(
                            |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                                if event.keystroke.key == "escape" {
                                    this.cancel_argo_connection(window, cx);
                                    cx.stop_propagation();
                                }
                            },
                        ))
                        .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                            this.cancel_argo_connection(window, cx)
                        }))
                        .child(self.argo_connection_content(cx))
                        .child(
                            button(Variant::Outline, 24., false)
                                .id("argo-connection-close")
                                .test_support()
                                .child("Close connection")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_argo_connection(window, cx)
                                })),
                        ),
                );
            overlay
                .id("argo-connection-overlay")
                .focus_trap("argo-connection-dialog", &dialog_focus)
                .into_any_element()
        })
    }
    fn argo_connection_content(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.argo_connections;
        let choices = state.choices();
        let mut panel = super::argo_panels::panel()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-connect-open")
                    .test_support()
                    .child(if self.argo_connection_id().is_some() {
                        "Manage connection"
                    } else {
                        "Connect Argo CD"
                    })
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_argo_connection(window, cx)),
                    ),
            );
        if !state.open {
            return super::argo_panels::panel().flex().flex_col().gap(px(8.))
                .child(gpui_kit::component::select::Select::new(&state.picker).id("argo-inspection-preference").w_full())
                .child(div().flex().flex_wrap().items_center().gap(px(8.))
                    .child(ui::muted(ui::xs(if self.argo_connection_id().is_some() {
                        "Connected Argo CD authorization is evaluated during operation preflight."
                    } else { "Argo RBAC is not evaluated. Exact manifests require a connected profile." })).flex_1().min_w_0())
                    .child(button(Variant::Outline, 24., false).id("argo-connect-open").test_support().child(if self.argo_connection_id().is_some() { "Manage connection" } else { "Connect Argo CD" })
                        .on_click(cx.listener(|this, _, window, cx| this.open_argo_connection(window, cx)))));
        }
        panel = panel.child(ui::sm("Argo CD connection"));
        for status in &state.profiles {
            let Some(p) = status
                .profile
                .as_ref()
                .filter(|p| self.argo_profile_matches(p))
            else {
                continue;
            };
            let saved = p.clone();
            let id = p.id.clone();
            let forget_id = id.clone();
            panel = panel.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .id(gpui_kit::SharedString::from(format!(
                        "argo-saved:{}",
                        p.url
                    )))
                    .test_support()
                    .aria_label(format!(
                        "Saved {}{}",
                        p.url,
                        if p.remember_credential {
                            " · reconnects automatically"
                        } else {
                            ""
                        }
                    ))
                    .child(ui::xs(p.url.clone()))
                    .child(
                        button(Variant::Outline, 24., state.busy())
                            .id(gpui_kit::SharedString::from(format!("argo-profile:{id}")))
                            .test_support()
                            .aria_label(if status.connected {
                                "Disconnect"
                            } else {
                                "Reconnect"
                            })
                            .child(if status.connected {
                                "Disconnect"
                            } else {
                                "Reconnect"
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.argo_connections.profiles.iter().any(|s| {
                                    s.connected && s.profile.as_ref().is_some_and(|p| p.id == id)
                                }) {
                                    this.disconnect_argo_profile(id.clone(), window, cx);
                                } else {
                                    this.argo_connections.url.update(cx, |input, cx| {
                                        input.set_value(saved.url.clone(), window, cx)
                                    });
                                    this.argo_connections.editing = Some(saved.clone());
                                    this.argo_connections.step.clear_preview();
                                    let confirmed = if saved.remember_credential {
                                        match &saved.endpoint {
                                            kubecove_lib::models::ArgoServerEndpoint::ServiceTunnel { namespace, service_name, .. } => Some(ArgoConfirmedTarget { namespace: namespace.clone(), service_name: service_name.clone(), pod_name: None, certificate_sha256: None }),
                                            _ => None,
                                        }
                                    } else { None };
                                    this.connect_argo_profile(confirmed, window, cx);
                                }
                            })),
                    )
                    .child(
                        button(Variant::Outline, 24., state.busy())
                            .id(gpui_kit::SharedString::from(format!(
                                "argo-forget:{forget_id}"
                            )))
                            .test_support()
                            .aria_label("Forget")
                            .child("Forget")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.forget_argo_profile(forget_id.clone(), window, cx)
                            })),
                    ),
            );
        }
        if let Some(error) = &state.discovery_error {
            panel = panel
                .child(
                    ui::xs(error.clone())
                        .id("argo-discovery-error")
                        .test_support()
                        .aria_label(error.clone()),
                )
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-discovery-retry")
                        .test_support()
                        .child("Retry")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.discover_argo_servers(window, cx)
                        })),
                );
        }
        panel = panel.child(
            div()
                .flex()
                .gap(px(8.))
                .children(["Automatic", "Kubernetes"].map(|label| {
                    button(Variant::Outline, 24., false)
                        .id(gpui_kit::SharedString::from(format!(
                            "argo-preference:{label}"
                        )))
                        .test_support()
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.argo_connections.selected = if label == "Automatic" {
                                Preference::Automatic
                            } else {
                                Preference::Kubernetes
                            };
                            this.persist_argo_preferences(cx);
                            this.reload_argo(window, cx);
                        }))
                })),
        );
        panel = panel.child(
            button(Variant::Outline, 24., false)
                .id("argo-external")
                .test_support()
                .child("External URL")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.argo_connections.endpoint = None;
                    this.argo_connections.editing = None;
                    this.argo_connections.step.clear_preview();
                    cx.notify();
                })),
        );
        for server in &state.servers {
            if let Some(reason) = &server.unavailable_reason {
                panel = panel.child(ui::muted(ui::xs(format!("{}: {reason}", server.name))));
                continue;
            }
            let Some(endpoint) = server.endpoint.clone() else {
                continue;
            };
            panel = panel.child(
                button(Variant::Outline, 24., false)
                    .id(gpui_kit::SharedString::from(format!(
                        "argo-server:{}",
                        server.id
                    )))
                    .test_support()
                    .aria_label(super::argo_profiles::endpoint_label(&endpoint))
                    .child(format!(
                        "{}{}",
                        super::argo_profiles::endpoint_label(&endpoint),
                        if server.argo_labeled {
                            ""
                        } else {
                            " · No Argo CD label"
                        }
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.argo_connections.endpoint = Some(endpoint.clone());
                        this.argo_connections.editing = None;
                        this.argo_connections.step.clear_preview();
                        cx.notify();
                    })),
            );
        }
        if state.endpoint.is_none() {
            panel = panel
                .child(
                    Input::new(&state.url)
                        .id("argo-url")
                        .aria_label("Argo CD server URL"),
                )
                .child(ui::xs("Custom CA PEM (session only)"))
                .child(
                    div().id("argo-ca").test_support().child(
                        Textarea::new(&state.ca)
                            .accessibility_id("argo-ca-input")
                            .aria_label("Custom CA PEM (session only)"),
                    ),
                )
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-insecure")
                        .test_support()
                        .aria_label("Accept invalid TLS certificate for this session")
                        .aria_toggled(toggled(choices.insecure))
                        .child(format!(
                            "Accept invalid TLS certificate for this session: {}",
                            choices.insecure
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.argo_connections.choices_mut().insecure ^= true;
                            cx.notify();
                        })),
                );
            if choices.insecure {
                panel = panel.child(
                    ui::xs("Insecure session. Certificate validation is disabled and never saved.")
                        .id("argo-insecure-warning")
                        .test_support()
                        .aria_label("Certificate validation is disabled and never saved.")
                        .text_color(Tokens::destructive()),
                );
            }
        }
        if state.endpoint.is_some() {
            panel = panel.child(div().flex().gap(px(8.)).children(["https", "http"].map(|scheme| {
                button(Variant::Outline, 24., state.busy()).id(gpui_kit::SharedString::from(format!("argo-protocol:{scheme}"))).test_support().child(scheme.to_uppercase())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(kubecove_lib::models::ArgoServerEndpoint::ServiceTunnel { scheme: value, .. }) = this.argo_connections.endpoint.as_mut() { *value = scheme.into(); }
                        this.argo_connections.step.clear_preview();
                        cx.notify();
                    }))
            })))
                .child(ui::muted(ui::xs("HTTPS pins the server certificate you confirm. HTTP sends the credential without TLS inside the private tunnel.")))
                .child(Input::new(&state.root_path).id("argo-root-path").aria_label("Root path"))
                .child(Input::new(&state.tls_name).id("argo-tls-name").aria_label("TLS server name"));
        }
        panel = panel.child(
            div()
                .flex()
                .gap(px(8.))
                .children(["Token", "Local login"].map(|label| {
                    button(Variant::Outline, 24., false)
                        .id(gpui_kit::SharedString::from(format!("argo-auth:{label}")))
                        .test_support()
                        .aria_label(label)
                        .aria_toggled(toggled(choices.local == (label == "Local login")))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.argo_connections.choices_mut().local = label == "Local login";
                            cx.notify();
                        }))
                })),
        );
        if choices.local {
            panel = panel
                .child(
                    Input::new(&state.username)
                        .id("argo-username")
                        .aria_label("Username"),
                )
                .child(
                    Input::new(&state.password)
                        .id("argo-password")
                        .aria_label("Password"),
                );
        } else {
            panel = panel.child(
                Input::new(&state.token)
                    .id("argo-token")
                    .aria_label("Argo CD token"),
            );
        }
        panel = panel.child(
            button(Variant::Outline, 24., false)
                .id("argo-remember")
                .test_support()
                .aria_label("Remember credential in native keyring")
                .aria_toggled(toggled(choices.remember))
                .child(format!(
                    "Remember credential in native keyring: {}",
                    choices.remember
                ))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.argo_connections.choices_mut().remember ^= true;
                    cx.notify();
                })),
        );
        if let ConnectStep::ConfirmTarget { preview, .. } = &state.step {
            let target = ArgoConfirmedTarget {
                namespace: preview.namespace.clone(),
                service_name: preview.service_name.clone(),
                pod_name: Some(preview.pod_name.clone()),
                certificate_sha256: preview.certificate_sha256.clone(),
            };
            panel = panel.child(ui::sm("Confirm tunnel target"))
                .child(ui::xs("Your Argo CD credential will be sent to this workload. A workload behind this Service can receive it."))
                .child(ui::xs(format!("Namespace {} · Service {} · Pod {}", preview.namespace, preview.service_name, preview.pod_name)).id("argo-tunnel-target").test_support().aria_label(format!("{} {} {}", preview.namespace, preview.service_name, preview.pod_name)))
                .when_some(preview.certificate_sha256.clone(), |p, sha| p.child(ui::xs(format!("SHA-256 {sha}. Only this certificate is accepted later. If Argo CD replaces it, you confirm again."))))
                .child(ui::xs("Continue only if you recognise this workload as your Argo CD server."))
                .child(button(Variant::Default, 24., state.busy()).id("argo-tunnel-confirm").test_support().child("Confirm and connect")
                    .on_click(cx.listener(move |this, _, window, cx| this.connect_argo_profile(Some(target.clone()), window, cx))));
        }
        panel = panel
            .child(
                button(Variant::Default, 24., state.busy())
                    .id("argo-connect-submit")
                    .test_support()
                    .child(if state.busy() {
                        "Connecting…"
                    } else {
                        "Connect"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.connect_argo_profile(None, window, cx)
                    })),
            )
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-connect-cancel")
                    .test_support()
                    .child("Cancel")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.cancel_argo_connection(window, cx)),
                    ),
            );
        if let ConnectStep::Failed {
            reconfirm: Some(profile),
            ..
        } = &state.step
        {
            let profile = profile.clone();
            panel = panel.child(
                button(Variant::Outline, 24., state.busy())
                    .id("argo-target-retry")
                    .test_support()
                    .child("Confirm target again")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.argo_connections.editing = Some(profile.clone());
                        this.preview_argo_target(profile.endpoint.clone(), window, cx)
                    })),
            );
        }
        panel.when_some(state.step.error().map(str::to_owned), |p, error| {
            p.child(
                ui::xs(error.clone())
                    .id("argo-connection-error")
                    .test_support()
                    .aria_label(error)
                    .text_color(Tokens::destructive()),
            )
        })
    }
}

/// The checked state the web's switches and sign-in radios expose.
fn toggled(on: bool) -> gpui_kit::accesskit::Toggled {
    if on {
        gpui_kit::accesskit::Toggled::True
    } else {
        gpui_kit::accesskit::Toggled::False
    }
}
