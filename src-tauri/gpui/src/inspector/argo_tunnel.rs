//! Discovery and preview finish before any credential leaves the connection form.
use super::Inspector;
use crate::backend::ArgoTarget;
use gpui_kit::{Context, Window};
use kubecove_lib::models::ArgoServerEndpoint;
use std::rc::Rc;

impl Inspector {
    pub(super) fn discover_argo_servers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.discover_argo(ArgoTarget {
            context: self.scope.context.clone(),
            source: self.argo_source_key().map(str::to_owned),
            endpoint: None,
        });
        let scope = self.argo_connections.scope.clone();
        self.argo_connections.discovery = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if this.argo_connections.scope != scope {
                    return;
                }
                match result {
                    Ok(servers) => this.argo_connections.servers = servers,
                    Err(error) => this.argo_connections.error = Some(error.message),
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
        self.argo_connections.preview_endpoint = Some(endpoint.clone());
        let read = self.backend.preview_argo(ArgoTarget {
            context: self.scope.context.clone(),
            source: self.argo_source_key().map(str::to_owned),
            endpoint: Some(endpoint),
        });
        self.argo_connections.request = Rc::new(());
        let request = self.argo_connections.request.clone();
        self.argo_connections.busy = true;
        self.argo_connections.preview = None;
        self.argo_connections.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&request, &this.argo_connections.request) {
                    return;
                }
                this.argo_connections.busy = false;
                match result {
                    Ok(preview) => this.argo_connections.preview = Some(preview),
                    Err(error) => this.argo_connections.error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
}
