//! Search text for a resource row, mirroring `buildResourceSearchIndex` in
//! `src/features/resources/helpers.ts`. Fields are joined with newlines, so a
//! query cannot match across two fields.

use crate::model::OwnerRef;
use kubecove_lib::models::ResourceSummary;

/// The row's lowercase search text.
#[cfg(test)]
pub fn haystack(row: &ResourceSummary) -> String {
    let mut text = String::new();
    push_haystack(row, crate::model::owner_ref(row), None, &mut text);
    text
}

/// Appends the row's lowercase search text to `text`: identity, API identity,
/// owner reference, Helm release and the GitOps owner's terms (`owner_ref` of
/// the row). `context` is the row's context in a view of several contexts, and
/// `None` otherwise, so a one-context view does not match its context name.
/// Each field is lowercased alone, which matches lowercasing the
/// joined text because no case mapping looks across a newline.
pub fn push_haystack(
    row: &ResourceSummary,
    owner: Option<OwnerRef<'_>>,
    context: Option<&str>,
    text: &mut String,
) {
    let fields = [
        Some(row.name.as_str()),
        Some(row.kind.as_str()),
        row.namespace.as_deref(),
        row.api_version.as_deref(),
        row.group.as_deref(),
        row.plural.as_deref(),
        row.owner_ref.as_deref(),
        row.helm_release.as_deref(),
        owner.map(|owner| owner.provider),
        owner.map(|owner| owner.kind),
        owner.map(|owner| owner.name),
        owner.and_then(|owner| owner.namespace),
        owner.map(|owner| owner.confidence),
        owner.map(|owner| owner.provenance),
        context,
    ];
    let mut first = true;
    for field in fields
        .into_iter()
        .flatten()
        .filter(|field| !field.is_empty())
    {
        if !first {
            text.push('\n');
        }
        first = false;
        if field.is_ascii() {
            let start = text.len();
            text.push_str(field);
            text[start..].make_ascii_lowercase();
        } else {
            text.push_str(&field.to_lowercase());
        }
    }
}
