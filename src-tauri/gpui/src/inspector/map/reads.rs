//! One topology read across all resource watches. Changes during its delay
//! join that read; changes during the read schedule a subsequent snapshot.
use super::{Inspector, TopologyLoad};
use crate::backend::Scope;
use crate::inspector::{
    Page,
    live_data::COALESCE,
    load_trace::{LoadKind, LoadTrace},
    request::InFlight,
};
use gpui_kit::Context;
use kubecove_lib::models::AppError;
use std::{rc::Rc, time::Instant};

#[derive(Default)]
pub(in crate::inspector) struct ReadState {
    read: Option<InFlight>,
    /// Set by changes not yet included in a topology read.
    invalidated: bool,
    pub(in crate::inspector) error: Option<AppError>,
}

impl ReadState {
    pub(in crate::inspector) fn cancel(&mut self) {
        *self = Self::default();
    }
}

impl Inspector {
    pub(in crate::inspector) fn load_topology(&mut self, cx: &mut Context<Self>) {
        self.map.read.cancel();
        if !matches!(self.page, Page::Resources | Page::ClusterOverview) {
            return;
        }
        self.map.cache.take();
        self.map.replace_topology(TopologyLoad::Loading);
        self.start_topology_read(false, cx);
    }

    pub(in crate::inspector) fn invalidate_topology(
        &mut self,
        scope: &Scope,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.page, Page::Resources | Page::ClusterOverview)
            || scope.context != self.map_context()
            || scope.namespace != self.scope.namespace
        {
            return;
        }
        self.map.read.invalidated = true;
        if self.map.read.read.is_none() {
            self.start_topology_read(true, cx);
        }
    }

    fn start_topology_read(&mut self, mut delayed: bool, cx: &mut Context<Self>) {
        let scope = Scope {
            context: self.map_context(),
            namespace: self.scope.namespace.clone(),
        };
        let request = self.request.clone();
        let network = self.map.network;
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let task = cx.spawn(async move |this, cx| {
            loop {
                if delayed {
                    cx.background_executor().timer(COALESCE).await;
                }
                let valid = this
                    .update(cx, |this, _| {
                        if !this.topology_target(&scope, network, &request) {
                            return false;
                        }
                        this.map.read.invalidated = false;
                        this.map_trace = Some(LoadTrace::start(
                            LoadKind::Map,
                            &scope.context,
                            scope.namespace.as_deref(),
                        ));
                        true
                    })
                    .unwrap_or(false);
                if !valid {
                    return;
                }
                let result = crate::read_retry::cancelled_read(
                    || {
                        let read = if network {
                            backend.network_topology(scope.clone())
                        } else {
                            backend.topology(scope.clone())
                        };
                        timings.measure("list_resource_topology", read)
                    },
                    |delay| cx.background_executor().timer(delay),
                )
                .await;
                let again = this
                    .update(cx, |this, cx| {
                        if !this.topology_target(&scope, network, &request) {
                            return false;
                        }
                        let started = Instant::now();
                        match result {
                            Ok(topology) => {
                                this.map.read.error = None;
                                this.map.replace_topology(TopologyLoad::Ready(topology));
                            }
                            Err(error) if matches!(this.map.topology, TopologyLoad::Ready(_)) => {
                                this.map.read.error = Some(error);
                            }
                            Err(error) => this.map.replace_topology(TopologyLoad::Failed(error)),
                        }
                        let again = this.map.read.invalidated;
                        if !again {
                            this.map.read.read = None;
                        }
                        cx.notify();
                        this.note_apply(LoadKind::Map, started.elapsed(), true);
                        again
                    })
                    .unwrap_or(false);
                if !again {
                    return;
                }
                delayed = true;
            }
        });
        self.map.read.read = Some(task.into());
        cx.notify();
    }

    fn topology_target(&self, scope: &Scope, network: bool, request: &Rc<()>) -> bool {
        Rc::ptr_eq(&self.request, request)
            && self.map.network == network
            && matches!(self.page, Page::Resources | Page::ClusterOverview)
            && scope.context == self.map_context()
            && scope.namespace == self.scope.namespace
    }
}
