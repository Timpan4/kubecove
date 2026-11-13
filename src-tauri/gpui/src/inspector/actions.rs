//! The Actions tab of the open resource: the permission baseline, the row
//! editors with their typed drafts, and the dry-run review that guards every
//! cluster change (ADR 0004). The rest of the inspector uses the names
//! exported here and the `Inspector` methods the family marks
//! `pub(in crate::inspector)`.

mod delete_review;
mod drafts;
mod editor_view;
mod editors;
mod fields;
mod review;
mod rollback_view;
mod state;
mod tab;

pub(super) use state::ActionsState;
pub(super) use tab::actions_tab;
