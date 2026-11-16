//! A saved profile that does not remember its credential asks for one in its own row, as the
//! web's `ArgoProfileSignIn`. The new-connection form's values may be meant for another server.
use super::super::{
    Inspector,
    controls::{Variant, button},
};
use super::connection_view::toggled;
use crate::targets::TestSupportExt;
use crate::ui;
use gpui_kit::{
    AppContext, Context, Div, Entity, Subscription, Window,
    component::input::{Input, InputEvent, InputState},
    div,
    prelude::*,
    px,
};
use kubecove_lib::models::{ArgoConnectionProfile, ArgoServerEndpoint};

const EXTERNAL_TLS_HINT: &str = "To use a custom CA or accept an invalid certificate, connect from the form below with this server's URL.";

/// Masked credential inputs. The new-connection form and a saved profile's row each own a set.
#[derive(Clone)]
pub(super) struct CredentialFields {
    pub token: Entity<InputState>,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
}

/// The credential a connect request sends. A remembered profile sends none and the
/// backend reads its keyring entry.
#[derive(Default)]
pub(super) struct Credential {
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Credential {
    pub fn is_empty(&self) -> bool {
        self.token.is_none() && self.username.is_none()
    }
}

impl CredentialFields {
    pub fn new(window: &mut Window, cx: &mut gpui_kit::App) -> Self {
        Self {
            token: cx.new(|cx| InputState::new(window, cx).masked(true)),
            username: cx.new(|cx| InputState::new(window, cx)),
            password: cx.new(|cx| InputState::new(window, cx).masked(true)),
        }
    }
    pub fn clear(&self, window: &mut Window, cx: &mut gpui_kit::App) {
        for field in [&self.token, &self.username, &self.password] {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }
    /// The typed token, or the local login when both its username and password are typed.
    pub fn read(&self, local: bool, cx: &gpui_kit::App) -> Credential {
        let value = |field: &Entity<InputState>| {
            let text = field.read(cx).value().to_string();
            (!text.is_empty()).then_some(text)
        };
        if !local {
            return Credential {
                token: value(&self.token),
                ..Credential::default()
            };
        }
        match (value(&self.username), value(&self.password)) {
            (Some(username), Some(password)) => Credential {
                token: None,
                username: Some(username),
                password: Some(password),
            },
            _ => Credential::default(),
        }
    }
}

/// The saved profile whose row asks for its credential. Dropping it drops what was typed.
pub(super) struct SignIn {
    pub profile_id: String,
    pub local: bool,
    pub fields: CredentialFields,
    _edits: [Subscription; 3],
}

impl Inspector {
    /// Opens the row sign-in of a saved profile that does not remember its credential.
    pub(super) fn open_argo_sign_in(
        &mut self,
        profile_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self
            .argo
            .connections
            .sign_in
            .as_ref()
            .map(|s| s.profile_id.clone());
        if previous.as_deref() == Some(profile_id.as_str()) {
            return;
        }
        // Another profile's row closes first, with its pending confirmation, as on the web.
        if let Some(previous) = previous {
            self.close_argo_sign_in(&previous);
        }
        let fields = CredentialFields::new(window, cx);
        // Typing re-renders the row, so Sign in enables once a credential is complete.
        let _edits = [&fields.token, &fields.username, &fields.password].map(|field| {
            cx.subscribe_in(field, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
        });
        self.argo.connections.sign_in = Some(SignIn {
            profile_id,
            local: false,
            fields,
            _edits,
        });
        cx.notify();
    }

    /// Closes the row sign-in of `profile_id` and drops its pending tunnel confirmation, so
    /// neither a later confirm nor the form's Connect signs in to that profile without a credential.
    pub(super) fn close_argo_sign_in(&mut self, profile_id: &str) {
        let connections = &mut self.argo.connections;
        if connections
            .sign_in
            .as_ref()
            .is_some_and(|s| s.profile_id == profile_id)
        {
            connections.sign_in = None;
        }
        if connections
            .editing
            .as_ref()
            .is_some_and(|p| p.id == profile_id)
        {
            connections.editing = None;
            connections.step.clear_preview();
        }
    }

    /// The sign-in row under `profile`, while that profile asks for its credential.
    pub(super) fn argo_sign_in_row(
        &self,
        profile: &ArgoConnectionProfile,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let sign_in = self
            .argo
            .connections
            .sign_in
            .as_ref()
            .filter(|s| s.profile_id == profile.id)?;
        let label = profile.url.clone();
        let methods = [("Token", false), ("Username and password", true)].map(|(text, local)| {
            button(Variant::Outline, 24., false)
                .id(gpui_kit::SharedString::from(format!(
                    "argo-sign-in-auth:{text}"
                )))
                .test_support()
                .aria_label(text)
                .aria_toggled(toggled(sign_in.local == local))
                .child(text)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(sign_in) = this.argo.connections.sign_in.as_mut() {
                        sign_in.local = local;
                    }
                    cx.notify();
                }))
        });
        let mut row = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(ui::muted(ui::xs(
                "This profile does not remember its credential. Enter one for this server.",
            )))
            .child(div().flex().gap(px(8.)).children(methods));
        row = if sign_in.local {
            row.child(
                Input::new(&sign_in.fields.username)
                    .id("argo-sign-in-username")
                    .aria_label(format!("Username for {label}")),
            )
            .child(
                Input::new(&sign_in.fields.password)
                    .id("argo-sign-in-password")
                    .aria_label(format!("Password for {label}")),
            )
        } else {
            row.child(
                Input::new(&sign_in.fields.token)
                    .id("argo-sign-in-token")
                    .aria_label(format!("Token for {label}")),
            )
        };
        // Custom CA and invalid-certificate choices are session-only and never saved, so a
        // profile that needs them connects again through the form with the same URL.
        if matches!(profile.endpoint, ArgoServerEndpoint::ExternalHttps { .. }) {
            row = row.child(ui::muted(ui::xs(EXTERNAL_TLS_HINT)));
        }
        let busy = self.argo.connections.busy();
        // As on the web, Sign in waits for a token or a full local login.
        let missing = sign_in.fields.read(sign_in.local, cx).is_empty();
        let saved = profile.clone();
        let cancel_id = profile.id.clone();
        Some(
            row.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        button(Variant::Default, 24., busy || missing)
                            .id("argo-sign-in-submit")
                            .test_support()
                            .child("Sign in")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.argo.connections.editing = Some(saved.clone());
                                this.argo.connections.step.clear_preview();
                                this.connect_argo_profile(None, window, cx);
                            })),
                    )
                    .child(
                        // A running preview or connect finishes first, as on the web.
                        button(Variant::Outline, 24., busy)
                            .id("argo-sign-in-cancel")
                            .test_support()
                            .aria_label("Cancel sign-in")
                            .child("Cancel")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.close_argo_sign_in(&cancel_id);
                                cx.notify();
                            })),
                    ),
            ),
        )
    }
}
