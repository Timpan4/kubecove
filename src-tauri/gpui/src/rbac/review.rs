//! Reviewed-context records, ported from `src/features/rbac/reviewModel.ts`.
//! A record is active while its evidence fingerprint matches the loaded
//! inspection and stale once that evidence changes.

use super::{CockpitItem, ObjectRef, bindings, binds_account, binds_role, subject_label};
use kubecove_lib::models::{
    RbacBindingSummary, RbacInspectionSummary, RbacRiskIndicator, RbacRoleSummary,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Expected,
    Anomalous,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRecord {
    pub cluster_context: String,
    pub object_key: String,
    pub evidence_fingerprint: String,
    pub disposition: Disposition,
    pub note: String,
    pub reviewed_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    Stale,
    None,
}

pub fn find<'a>(records: &'a [ReviewRecord], cluster: &str, key: &str) -> Option<&'a ReviewRecord> {
    records
        .iter()
        .find(|record| record.cluster_context == cluster && record.object_key == key)
}

/// `reviewStatus`.
pub fn status(
    record: Option<&ReviewRecord>,
    data: &RbacInspectionSummary,
    item: &CockpitItem,
) -> Status {
    let Some(record) = record else {
        return Status::None;
    };
    if record.cluster_context == data.cluster
        && record.object_key == item.key
        && fingerprint(data, item).as_deref() == Some(record.evidence_fingerprint.as_str())
    {
        Status::Active
    } else {
        Status::Stale
    }
}

/// `upsertRbacReviewRecord`: rejects an empty note, replaces the record for
/// the same cluster and object, and appends otherwise.
pub fn upsert(records: &mut Vec<ReviewRecord>, mut record: ReviewRecord) -> Result<(), String> {
    record.note = record.note.trim().to_owned();
    if record.note.is_empty() {
        return Err("RBAC review note must not be empty.".into());
    }
    match records.iter_mut().find(|candidate| {
        candidate.cluster_context == record.cluster_context
            && candidate.object_key == record.object_key
    }) {
        Some(existing) => *existing = record,
        None => records.push(record),
    }
    Ok(())
}

pub fn remove(records: &mut Vec<ReviewRecord>, cluster: &str, key: &str) {
    records.retain(|record| record.cluster_context != cluster || record.object_key != key);
}

/// `rbacReviewSubjectLabels`: the identities the object's access reaches.
pub fn subject_labels(data: &RbacInspectionSummary, item: &CockpitItem) -> Vec<String> {
    if !item.category.reviewable() {
        return vec![];
    }
    let bindings: Vec<&RbacBindingSummary> = match item.object.get(data) {
        ObjectRef::Account(account) => {
            return vec![format!(
                "ServiceAccount:{}/{}",
                account.namespace, account.name
            )];
        }
        ObjectRef::Binding(binding) => vec![binding],
        ObjectRef::Role(role) => role_bindings(data, role),
        ObjectRef::Namespace(_) => vec![],
    };
    let mut labels: Vec<String> = bindings
        .iter()
        .flat_map(|binding| binding.subjects.iter().map(subject_label))
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

fn role_bindings<'a>(
    data: &'a RbacInspectionSummary,
    role: &RbacRoleSummary,
) -> Vec<&'a RbacBindingSummary> {
    bindings(data)
        .filter(|binding| binds_role(binding, role))
        .collect()
}

/// `buildRbacEvidenceFingerprint`: FNV-1a over the normalized evidence.
pub fn fingerprint(data: &RbacInspectionSummary, item: &CockpitItem) -> Option<String> {
    item.category.reviewable().then(|| {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for unit in evidence(data, item).to_string().encode_utf16() {
            hash ^= u64::from(unit);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("rbac-v1-{}", base36(hash))
    })
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
        if value == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).expect("base36 digits are ASCII")
}

fn sorted(mut values: Vec<Value>) -> Value {
    values.sort_by_key(Value::to_string);
    Value::Array(values)
}

fn risks(risks: &[RbacRiskIndicator]) -> Value {
    sorted(
        risks
            .iter()
            .map(|risk| json!({"level": risk.level, "label": risk.label, "reason": risk.reason}))
            .collect(),
    )
}

fn role_evidence(role: &RbacRoleSummary) -> Value {
    let strings = |values: &[String]| {
        let mut values = values.to_vec();
        values.sort();
        values
    };
    json!({
        "identity": {"kind": role.kind, "namespace": role.namespace, "name": role.name},
        "rules": sorted(role.rules.iter().map(|rule| json!({
            "verbs": strings(&rule.verbs),
            "apiGroups": strings(&rule.api_groups),
            "resources": strings(&rule.resources),
            "resourceNames": strings(&rule.resource_names),
            "nonResourceUrls": strings(&rule.non_resource_urls),
            "risks": risks(&rule.risks),
        })).collect()),
        "risks": risks(&role.risks),
    })
}

fn bound_role<'a>(
    data: &'a RbacInspectionSummary,
    binding: &RbacBindingSummary,
) -> Option<&'a RbacRoleSummary> {
    let roles = if binding.role_ref_kind == "Role" {
        &data.roles
    } else {
        &data.cluster_roles
    };
    roles.iter().find(|role| {
        role.name == binding.role_ref_name
            && (binding.role_ref_kind != "Role" || role.namespace == binding.namespace)
    })
}

fn binding_evidence(binding: &RbacBindingSummary, role: Option<&RbacRoleSummary>) -> Value {
    json!({
        "identity": {"kind": binding.kind, "namespace": binding.namespace, "name": binding.name},
        "roleRef": {"kind": binding.role_ref_kind, "name": binding.role_ref_name},
        "subjects": sorted(binding.subjects.iter().map(|subject| json!({
            "kind": subject.kind, "namespace": subject.namespace, "name": subject.name,
        })).collect()),
        "risks": risks(&binding.risks),
        "inheritedRole": role.map_or_else(|| json!({"missing": true}), role_evidence),
    })
}

fn evidence(data: &RbacInspectionSummary, item: &CockpitItem) -> Value {
    let mut evidence = json!({
        "cluster": data.cluster,
        "objectKey": item.key,
        "category": item.category.label(),
        "identity": {"kind": item.kind, "namespace": item.namespace, "name": item.name},
        "scope": item.namespace.as_deref().unwrap_or("cluster"),
        "risks": risks(&item.risks),
    });
    let extra = match item.object.get(data) {
        ObjectRef::Role(role) => {
            evidence["role"] = role_evidence(role);
            (
                "bindingPaths",
                sorted(
                    role_bindings(data, role)
                        .into_iter()
                        .map(|binding| binding_evidence(binding, Some(role)))
                        .collect(),
                ),
            )
        }
        ObjectRef::Binding(binding) => (
            "binding",
            binding_evidence(binding, bound_role(data, binding)),
        ),
        ObjectRef::Account(account) => (
            "serviceAccount",
            json!({
                "automountToken": account.automount_token,
                "secretsCount": account.secrets_count,
                "imagePullSecretsCount": account.image_pull_secrets_count,
                "inheritedPaths": sorted(bindings(data)
                    .filter(|binding| binds_account(binding, &account.namespace, &account.name))
                    .map(|binding| binding_evidence(binding, bound_role(data, binding)))
                    .collect()),
            }),
        ),
        ObjectRef::Namespace(_) => return evidence,
    };
    evidence[extra.0] = extra.1;
    evidence
}
