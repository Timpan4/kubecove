//! Conservative grouping from ResourceSummary evidence, matching the web overview model.
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};
use std::collections::{HashMap, HashSet};

pub fn identity(row: &ResourceSummary) -> String {
    let group = row
        .group
        .as_deref()
        .filter(|group| !group.is_empty())
        .or_else(|| api_group(row.api_version.as_deref()));
    serde_json::to_string(&(
        &row.cluster,
        &row.namespace,
        &row.kind,
        &row.api_version,
        group,
        &row.name,
    ))
    .expect("resource identity serializes")
}
pub fn entry_identity(entry: &super::entries::Entry) -> String {
    let kind = if entry.kind == super::entries::EntryKind::App {
        "Application"
    } else {
        entry.resource_kind.as_deref().unwrap_or("Resource")
    };
    serde_json::to_string(&(
        &entry.cluster_context,
        &entry.namespace,
        kind,
        &entry.api_version,
        api_group(entry.api_version.as_deref()),
        &entry.name,
    ))
    .expect("entry identity serializes")
}
fn api_group(version: Option<&str>) -> Option<&str> {
    version.and_then(|version| version.split_once('/').map(|(group, _)| group))
}

pub struct Problem {
    pub root: usize,
    pub cause: usize,
    pub related: Vec<usize>,
    pub state: HealthAssessmentState,
}
pub struct Namespace {
    pub name: String,
    pub total: usize,
    pub degraded: usize,
    pub attention: usize,
}
#[derive(Default)]
pub struct Overview {
    pub problems: Vec<Problem>,
    pub namespaces: Vec<Namespace>,
    pub problem_resources: usize,
}
pub fn failing(state: HealthAssessmentState) -> bool {
    matches!(
        state,
        HealthAssessmentState::Degraded | HealthAssessmentState::NeedsAttention
    )
}

/// Ownership with a namespace identifies one Application; a bare app name does not.
pub fn application_owner(row: &ResourceSummary) -> Option<ResourceSummary> {
    let owner = row.git_ops_owner.as_ref()?;
    if owner.provider != "argo" || owner.kind != "Application" {
        return None;
    }
    Some(ResourceSummary {
        kind: "Application".into(),
        cluster: row.cluster.clone(),
        name: owner.name.clone(),
        namespace: Some(owner.namespace.as_ref()?.clone()),
        age: String::new(),
        api_version: Some("argoproj.io/v1alpha1".into()),
        group: Some("argoproj.io".into()),
        version: Some("v1alpha1".into()),
        plural: Some("applications".into()),
        namespaced: Some(true),
        dynamic: Some(true),
        health: Default::default(),
        health_assessment: Default::default(),
        created_at: None,
        status: None,
        ready: None,
        restarts: None,
        owner_ref: None,
        uid: None,
        controller_owner: None,
        argo_app: None,
        helm_release: None,
        git_ops_ownership_partial: false,
        git_ops_owner: Some(owner.clone()),
    })
}
pub fn build(rows: &[&ResourceSummary]) -> Overview {
    let key = |row: &ResourceSummary, uid: &str| {
        (row.cluster.clone(), row.namespace.clone(), uid.to_owned())
    };
    let mut owners = HashMap::<_, Option<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(uid) = row.uid.as_deref().filter(|uid| !uid.is_empty()) {
            owners
                .entry(key(row, uid))
                .and_modify(|entry| *entry = None)
                .or_insert(Some(index));
        }
    }
    let mut groups = HashMap::<usize, Vec<usize>>::new();
    let mut namespaces = HashMap::<&str, Namespace>::new();
    let mut model = Overview::default();
    for (index, row) in rows.iter().enumerate() {
        let state = row.health_assessment.state;
        if let Some(name) = row.namespace.as_deref() {
            let namespace = namespaces.entry(name).or_insert_with(|| Namespace {
                name: name.to_owned(),
                total: 0,
                degraded: 0,
                attention: 0,
            });
            namespace.total += 1;
            namespace.degraded += usize::from(state == HealthAssessmentState::Degraded);
            namespace.attention += usize::from(state == HealthAssessmentState::NeedsAttention);
        }
        if !failing(state) {
            continue;
        }
        model.problem_resources += 1;
        let mut root = index;
        let mut current = index;
        let mut seen = HashSet::from([index]);
        while let Some(reference) = rows[current].controller_owner.as_ref() {
            let local = owners.get(&key(rows[current], &reference.uid));
            let cluster = rows[current].namespace.as_ref().and_then(|_| {
                owners.get(&(rows[current].cluster.clone(), None, reference.uid.clone()))
            });
            let candidate = match (local, cluster) {
                (Some(local), None) => *local,
                (None, Some(cluster)) => *cluster,
                _ => None,
            };
            let Some(owner) = candidate else {
                break;
            };
            let parent = rows[owner];
            if parent.name != reference.name
                || parent.kind != reference.kind
                || parent.api_version.as_deref() != Some(&reference.api_version)
            {
                break;
            }
            if !seen.insert(owner) {
                root = index;
                break;
            }
            if failing(rows[owner].health_assessment.state) {
                root = owner;
            }
            current = owner;
        }
        groups.entry(root).or_default().push(index);
    }
    for (root, members) in groups {
        let state = if members
            .iter()
            .any(|&index| rows[index].health_assessment.state == HealthAssessmentState::Degraded)
        {
            HealthAssessmentState::Degraded
        } else {
            HealthAssessmentState::NeedsAttention
        };
        let mut cause = root;
        for &index in &members {
            let row = rows[index];
            if row.health_assessment.state != state {
                continue;
            }
            let current = rows[cause];
            if current.health_assessment.state != state
                || (row.kind == "Pod"
                    && (current.kind != "Pod"
                        || row.restarts.unwrap_or(0) > current.restarts.unwrap_or(0)))
            {
                cause = index;
            }
        }
        model.problems.push(Problem {
            root,
            cause,
            state,
            related: members.into_iter().filter(|index| *index != root).collect(),
        });
    }
    model.problems.sort_by(|left, right| {
        let a = rows[left.root];
        let b = rows[right.root];
        (left.state != HealthAssessmentState::Degraded)
            .cmp(&(right.state != HealthAssessmentState::Degraded))
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.cluster.cmp(&b.cluster))
            .then_with(|| a.api_version.cmp(&b.api_version))
    });
    model.namespaces = namespaces.into_values().collect();
    model.namespaces.sort_by(|a, b| {
        b.degraded
            .cmp(&a.degraded)
            .then_with(|| b.attention.cmp(&a.attention))
            .then_with(|| a.name.cmp(&b.name))
    });
    model
}
#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
