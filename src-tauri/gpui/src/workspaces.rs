//! Saved workspaces and the launcher form, ported from
//! `src/features/workspaces/workspaceLauncherModel.ts` and the scope helpers
//! in `src/lib/workspace-model.ts`.
//!
//! The Backend seam persists local saved records under ADR 0030.

use crate::model::WORKSPACE_KINDS;
pub mod entries;
pub mod overview;
pub mod reads;
pub mod shortcuts;
pub mod transfer;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedWorkspace {
    pub id: u64,
    pub name: String,
    pub context: String,
    /// Cluster group members after the primary context, without duplicates.
    pub group: Vec<String>,
    pub group_name: Option<String>,
    pub namespaces: Vec<String>,
    pub kinds: Vec<SavedKind>,
    /// Monotonic edit order; the launcher lists the latest first.
    pub updated: u64,
    /// Local review context belongs to this workspace, even for the same cluster.
    pub reviews: Vec<crate::rbac::review::ReviewRecord>,
}

/// One saved workspace kind, stored as `ResourceKindSelection` in
/// `src/lib/types.ts` is: a built-in kind's name, or a discovered kind with
/// its API identity, so same-named kinds of other API groups stay apart.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SavedKind {
    Name(String),
    Api(ApiKind),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiKind {
    pub group: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
}

impl SavedKind {
    /// `kindLabel`.
    pub fn label(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Api(api) => &api.kind,
        }
    }

    /// `kindKey`: a name, or `apiVersion/plural/kind`; the two never collide.
    pub fn key(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Api(api) => format!("{}/{}/{}", api.api_version, api.plural, api.kind),
        }
    }

    /// `kindMatcher`: a name matches by name; an API kind needs the
    /// resource's apiVersion, and matches by name only when it has none.
    /// An empty apiVersion counts as none, as the web's truthiness test does.
    pub fn matches(&self, kind: &str, api_version: Option<&str>) -> bool {
        match self {
            Self::Name(name) => name == kind,
            Self::Api(api) => {
                api.kind == kind
                    && api_version
                        .filter(|version| !version.is_empty())
                        .is_none_or(|version| version == api.api_version)
            }
        }
    }
}

impl SavedWorkspace {
    /// `workspaceScopeContexts`.
    pub fn contexts(&self) -> Vec<&str> {
        let mut unique = Vec::<&str>::with_capacity(self.group.len() + 1);
        for context in std::iter::once(&self.context).chain(&self.group) {
            if !context.is_empty() && !unique.contains(&context.as_str()) {
                unique.push(context);
            }
        }
        unique
    }

    /// `summarizeWorkspaceScope`.
    pub fn summary(&self) -> String {
        let contexts = self.contexts();
        let context = if contexts.len() <= 1 {
            self.context.clone()
        } else {
            format!(
                "{} ({})",
                self.group_name.as_deref().unwrap_or("Cluster group"),
                contexts.len()
            )
        };
        let namespaces = match self.namespaces.as_slice() {
            [] => "All namespaces".to_owned(),
            [one] => one.clone(),
            [first, second] => format!("{first}, {second}"),
            [first, second, rest @ ..] => format!("{first}, {second} +{}", rest.len()),
        };
        let labels: Vec<&str> = self.kinds.iter().map(SavedKind::label).collect();
        let kinds = match labels.as_slice() {
            labels if labels.len() <= 2 => labels.join(", "),
            [first, second, rest @ ..] => format!("{first}, {second} +{}", rest.len()),
            _ => unreachable!("slices longer than two have two leading items"),
        };
        format!("{context} / {namespaces} / {kinds}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discovery {
    Loading,
    Failed,
    Ready,
}

/// `getWorkspaceCreationAvailability`: `Err` carries the disabled reason.
pub fn availability(
    effective: &str,
    context_missing: bool,
    discovery: Discovery,
) -> Result<(), &'static str> {
    if effective.is_empty() {
        Err("Select a context before creating this workspace.")
    } else if context_missing {
        Err("Select an available context before creating this workspace.")
    } else if discovery == Discovery::Failed {
        Err("Retry namespace discovery before creating this workspace.")
    } else if discovery == Discovery::Loading {
        Err("Wait for namespace discovery to finish.")
    } else {
        Ok(())
    }
}

/// The launcher form's values, as `buildWorkspaceInput` reads them.
pub struct Draft<'a> {
    pub name: &'a str,
    pub effective: &'a str,
    pub group: &'a [String],
    pub namespaces: &'a [String],
    pub kinds: Option<&'a [SavedKind]>,
}

impl Draft<'_> {
    /// `buildWorkspaceInput` applied to a workspace record.
    pub fn build(&self, id: u64, updated: u64) -> SavedWorkspace {
        let trimmed = self.name.trim();
        let name = if trimmed.is_empty() {
            self.effective
        } else {
            trimmed
        };
        let group: Vec<String> = self
            .group
            .iter()
            .filter(|context| context.as_str() != self.effective)
            .cloned()
            .collect();
        let mut namespaces = self.namespaces.to_vec();
        namespaces.sort_by(|a, b| crate::collate::collate(a, b, false));
        namespaces.dedup();
        SavedWorkspace {
            id,
            name: name.to_owned(),
            context: self.effective.to_owned(),
            group_name: (!group.is_empty()).then(|| format!("{name} group")),
            group,
            namespaces,
            kinds: self.kinds.map_or_else(
                || {
                    WORKSPACE_KINDS
                        .iter()
                        .map(|kind| SavedKind::Name((*kind).to_owned()))
                        .collect()
                },
                <[SavedKind]>::to_vec,
            ),
            updated,
            reviews: vec![],
        }
    }
}

/// `pickEffectiveContext`.
pub fn effective_context<'a>(selected: &'a str, choices: &'a [(String, bool)]) -> &'a str {
    if !selected.is_empty() {
        return selected;
    }
    choices
        .iter()
        .find(|(_, current)| *current)
        .or(choices.first())
        .map_or("", |(name, _)| name.as_str())
}
