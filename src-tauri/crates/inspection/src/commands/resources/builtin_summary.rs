use crate::commands::helpers::{assess_kubernetes_health, base_resource_summary, resource_age_at};
use crate::models::{
    HealthAssessment, HealthAssessmentCompleteness, HealthAssessmentState, ResourceHealth,
    ResourceSummary,
};
use chrono::{DateTime, TimeZone, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use serde_json::Value;

/// Status, readiness and restarts read from one object, with the Kubernetes health they imply.
#[derive(Default)]
pub struct KindStatus {
    pub status: Option<String>,
    pub ready: Option<String>,
    pub restarts: Option<i32>,
    pub health: ResourceHealth,
}

/// The one object-to-summary projection of a built-in kind. List rows, details and topology
/// nodes all start from [`builtin_summary`]; details and topology add only their own extras.
pub trait BuiltinSummary: k8s_openapi::Resource + k8s_openapi::Metadata<Ty = ObjectMeta> {
    /// `None` for kinds without Kubernetes health semantics, which keep the default assessment.
    fn kind_status(&self) -> Option<KindStatus> {
        None
    }

    /// The object's `status`, which details return beside the summary.
    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        None
    }

    /// Identity beyond kind and apiVersion, for kinds that describe an API themselves.
    fn add_identity(_summary: &mut ResourceSummary) {}
}

pub fn builtin_summary<K: BuiltinSummary>(
    cluster: &str,
    object: &K,
    now: DateTime<Utc>,
) -> ResourceSummary {
    let Some(kind_status) = object.kind_status() else {
        return metadata_summary::<K>(cluster, object.metadata(), now);
    };
    // Health is assessed below, so skip the default assessment's allocations.
    let assessed_below = HealthAssessment {
        state: HealthAssessmentState::NotEvaluated,
        completeness: HealthAssessmentCompleteness::Complete,
        winning_sources: Vec::new(),
        reasons: Vec::new(),
        evidence: Vec::new(),
    };
    let mut summary = identity_summary::<K>(cluster, object.metadata(), now, assessed_below);
    summary.status = kind_status.status;
    summary.ready = kind_status.ready;
    summary.restarts = kind_status.restarts;
    assess_kubernetes_health(&mut summary, kind_status.health);
    summary
}

/// Summary of an object listed as metadata only: identity and ownership evidence, no status.
pub fn metadata_summary<K: BuiltinSummary>(
    cluster: &str,
    metadata: &ObjectMeta,
    now: DateTime<Utc>,
) -> ResourceSummary {
    identity_summary::<K>(cluster, metadata, now, HealthAssessment::default())
}

fn identity_summary<K: BuiltinSummary>(
    cluster: &str,
    metadata: &ObjectMeta,
    now: DateTime<Utc>,
    health_assessment: HealthAssessment,
) -> ResourceSummary {
    let mut summary = base_resource_summary(
        K::KIND,
        K::API_VERSION,
        cluster,
        metadata,
        age_from_metadata_at(metadata, now),
        health_assessment,
    );
    K::add_identity(&mut summary);
    summary
}

pub(crate) fn age_from_metadata_at(metadata: &ObjectMeta, now: DateTime<Utc>) -> String {
    metadata
        .creation_timestamp
        .as_ref()
        .map(|time| {
            Utc.timestamp_opt(time.0.as_second(), 0)
                .single()
                .unwrap_or(now)
        })
        .map_or_else(|| "unknown".to_string(), |time| resource_age_at(time, now))
}
