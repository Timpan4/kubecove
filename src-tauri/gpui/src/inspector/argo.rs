//! Argo CD: the detail panel's Argo CD tab, the Application workspace on the
//! GitOps page, the connection to an Argo CD server and the reviewed
//! operations with their outcomes. The rest of the inspector uses the types
//! exported here and the `Inspector` methods the family marks
//! `pub(in crate::inspector)`.

mod connection;
mod connection_dialog;
mod connection_view;
mod highlight;
mod operations;
mod options;
mod outcomes;
mod panels;
mod profiles;
mod snapshot;
mod tab;
mod tunnel;
mod workspace;

pub(super) use connection::DialogHost;
pub(super) use tab::{ArgoTab, raw};
pub(super) use workspace::ApplicationWorkspace;

use super::Inspector;
use gpui_kit::{Context, Window};

/// The Argo CD state that outlives an open detail.
pub(super) struct ArgoState {
    connections: connection::Connections,
    /// The YAML highlighter the diff views share, built on first use.
    highlighter: Option<gpui_kit::component::highlighter::SyntaxHighlighter>,
    /// The outcome observed for each Application's last operation.
    outcomes: std::collections::HashMap<String, outcomes::Outcome>,
    /// Submissions still in flight when their detail closed. Dropping them
    /// would cancel the submit and lose its outcome; each removes itself once
    /// it finishes.
    submissions: Vec<operations::Submission>,
}

impl ArgoState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        Self {
            connections: connection::Connections::new(window, cx),
            highlighter: None,
            outcomes: Default::default(),
            submissions: Vec::new(),
        }
    }

    /// Forgets what the user chose and typed in one connection dialog, once
    /// the surface that hosts it goes away.
    pub(super) fn reset_dialog(&mut self, host: DialogHost, cx: &mut gpui_kit::App) {
        self.connections.reset_dialog(host, cx);
    }
}
