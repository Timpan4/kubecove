//! Discovery and preview finish before any credential leaves the connection form.
use super::super::Inspector;
use super::connection::ConnectStep;
use crate::backend::ArgoTarget;
use gpui_kit::{Context, Window};
use kubecove_lib::models::{ArgoConnectionProfile, ArgoServerEndpoint};

impl Inspector {
    pub(super) fn discover_argo_servers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo.connections.discovery_error = None;
        let target = ArgoTarget {
            context: self.scope.context.clone(),
            source: self.argo_source_key().map(str::to_owned),
            endpoint: None,
        };
        let backend = self.backend.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.discover_argo(target.clone())
        });
        let scope = self.argo.connections.scope.clone();
        self.argo.connections.discovery = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if this.argo.connections.scope != scope {
                    return;
                }
                match result {
                    Ok(servers) => {
                        this.argo.connections.servers = servers;
                        this.settle_argo_source();
                    }
                    Err(error) => this.argo.connections.discovery_error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn preview_argo_target(
        &mut self,
        endpoint: ArgoServerEndpoint,
        saved: Option<ArgoConnectionProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let read = self.backend.preview_argo(ArgoTarget {
            context: self.scope.context.clone(),
            source: self.argo_source_key().map(str::to_owned),
            endpoint: Some(endpoint.clone()),
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                // A saved profile without a keyring credential confirms only while its row is
                // open, as on the web. Its confirmation would otherwise carry no credential.
                let row_closed = saved.as_ref().is_some_and(|p| {
                    !p.remember_credential
                        && this
                            .argo
                            .connections
                            .sign_in
                            .as_ref()
                            .is_none_or(|s| s.profile_id != p.id)
                });
                // A preview that fails leaves no reconfirm offer, as on the web.
                this.argo.connections.step = match result {
                    Ok(_) if row_closed => ConnectStep::Idle,
                    Ok(preview) => ConnectStep::ConfirmTarget {
                        endpoint,
                        preview,
                        saved,
                    },
                    Err(error) => ConnectStep::failed(error.message),
                };
                cx.notify();
            });
        });
        self.argo.connections.step = ConnectStep::Busy { _read: task.into() };
        cx.notify();
    }
}
