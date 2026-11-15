//! Controlling owner metadata displayed by the delete review (ADR 0024).

use crate::models::ResourceActionController;
use serde_json::Value;

pub(super) fn action_controller(object: &Value) -> Option<ResourceActionController> {
    let owner = object
        .pointer("/metadata/ownerReferences")?
        .as_array()?
        .iter()
        .find(|owner| owner.get("controller").and_then(Value::as_bool) == Some(true))?;
    let kind = owner
        .get("kind")?
        .as_str()
        .filter(|kind| !kind.is_empty())?;
    let name = owner
        .get("name")?
        .as_str()
        .filter(|name| !name.is_empty())?;
    Some(ResourceActionController {
        kind: kind.to_owned(),
        name: name.to_owned(),
    })
}
