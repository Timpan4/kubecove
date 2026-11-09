//! Discovery and preview finish before any credential leaves the connection form.
use super::{Inspector, argo_connection::ConnectStep};
use crate::backend::ArgoTarget;
use gpui_kit::{Context, Window};
use kubecove_lib::models::ArgoServerEndpoint;

impl Inspector {
    pub(super) fn discover_argo_servers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo_connections.discovery_error = None;
        let target = ArgoTarget {
            context: self.scope.context.clone(),
            source: self.argo_source_key().map(str::to_owned),
            endpoint: None,
        };
        let backend = self.backend.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.discover_argo(target.clone())
        });
        let scope = self.argo_connections.scope.clone();
        self.argo_connections.discovery = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if this.argo_connections.scope != scope {
                    return;
                }
                match result {
                    Ok(servers) => {
                        this.argo_connections.servers = servers;
                        this.settle_argo_source();
                    }
                    Err(error) => this.argo_connections.discovery_error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn preview_argo_target(
        &mut self,
        endpoint: ArgoServerEndpoint,
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
                // A preview that fails leaves no reconfirm offer, as on the web.
                this.argo_connections.step = match result {
                    Ok(preview) => ConnectStep::ConfirmTarget { endpoint, preview },
                    Err(error) => ConnectStep::failed(error.message),
                };
                cx.notify();
            });
        });
        self.argo_connections.step = ConnectStep::Busy { _read: task.into() };
        cx.notify();
    }
}
