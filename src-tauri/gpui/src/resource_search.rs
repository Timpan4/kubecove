//! Search text for a resource row, mirroring `buildResourceSearchIndex` in
//! `src/features/resources/helpers.ts`. Fields are joined with newlines, so a
//! query cannot match across two fields.

use crate::model::gitops_owner;
use kubecove_lib::models::ResourceSummary;

/// The row's lowercase search text: identity, API identity, owner reference,
/// Helm release and the GitOps owner's terms.
pub fn haystack(row: &ResourceSummary) -> String {
    let owner = gitops_owner(row);
    let mut fields: Vec<&str> = vec![row.name.as_str(), row.kind.as_str()];
    fields.extend(row.namespace.as_deref());
    fields.extend(row.api_version.as_deref());
    fields.extend(row.group.as_deref());
    fields.extend(row.plural.as_deref());
    fields.extend(row.owner_ref.as_deref());
    fields.extend(row.helm_release.as_deref());
    if let Some(owner) = &owner {
        fields.extend([
            owner.provider.as_str(),
            owner.kind.as_str(),
            owner.name.as_str(),
        ]);
        fields.extend(owner.namespace.as_deref());
        fields.extend([owner.confidence.as_str(), owner.provenance.as_str()]);
    }
    fields.retain(|field| !field.is_empty());
    fields.join("\n").to_lowercase()
}
