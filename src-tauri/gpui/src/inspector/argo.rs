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

pub(super) use connection::{Connections, DialogHost};
pub(super) use operations::Submission;
pub(super) use outcomes::Outcome;
pub(super) use tab::{ArgoTab, raw};
pub(super) use workspace::ApplicationWorkspace;
