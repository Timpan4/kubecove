//! The RBAC Cockpit page: the access posture, the review queue, the selected
//! object's evidence and the exact-action verifier. The rest of the inspector
//! uses `RbacState` and the `Inspector` methods the family marks
//! `pub(in crate::inspector)`.

mod inspector;
mod page;
mod queue;
mod review;
mod verify;

pub(super) use page::RbacState;
