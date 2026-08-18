use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthAssessmentState {
    Healthy,
    NeedsAttention,
    Degraded,
    Unknown,
    NotEvaluated,
}

impl HealthAssessmentState {
    fn rank(self) -> Option<u8> {
        match self {
            Self::Healthy => Some(1),
            Self::NeedsAttention => Some(2),
            Self::Degraded => Some(3),
            Self::Unknown | Self::NotEvaluated => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthAssessmentCompleteness {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthAssessmentSource {
    Kubernetes,
    ArgoHealth,
    ArgoSync,
    WarningEvent,
    ContainerRestart,
    Provider,
    HealthContract,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthAssessmentEvidence {
    pub source: HealthAssessmentSource,
    pub raw: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<HealthAssessmentState>,
    pub current: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthAssessment {
    pub state: HealthAssessmentState,
    pub completeness: HealthAssessmentCompleteness,
    pub winning_sources: Vec<HealthAssessmentSource>,
    pub reasons: Vec<String>,
    pub evidence: Vec<HealthAssessmentEvidence>,
}

impl Default for HealthAssessment {
    fn default() -> Self {
        Self {
            state: HealthAssessmentState::NotEvaluated,
            completeness: HealthAssessmentCompleteness::Complete,
            winning_sources: vec![],
            reasons: vec!["No recognized health semantics".to_string()],
            evidence: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthAssessmentInput {
    pub recognized_semantics: bool,
    pub provider_available: bool,
    pub evidence: Vec<HealthAssessmentEvidence>,
}

pub fn evaluate_health(mut input: HealthAssessmentInput) -> HealthAssessment {
    let completeness = if input.provider_available {
        HealthAssessmentCompleteness::Complete
    } else {
        HealthAssessmentCompleteness::Partial
    };
    if !input.provider_available {
        input.evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::Provider,
            raw: Value::String("unavailable".to_string()),
            state: Some(HealthAssessmentState::Unknown),
            current: true,
            reason: "Expected health provider is unavailable".to_string(),
        });
    }
    if !input.recognized_semantics {
        input.evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::HealthContract,
            raw: Value::String("no recognized health semantics".to_string()),
            state: Some(HealthAssessmentState::NotEvaluated),
            current: true,
            reason: "No recognized health semantics".to_string(),
        });
        return HealthAssessment {
            state: HealthAssessmentState::NotEvaluated,
            completeness,
            winning_sources: vec![HealthAssessmentSource::HealthContract],
            reasons: vec!["No recognized health semantics".to_string()],
            evidence: input.evidence,
        };
    }

    let winning_rank = input
        .evidence
        .iter()
        .filter(|evidence| evidence.current)
        .filter_map(|evidence| evidence.state.and_then(HealthAssessmentState::rank))
        .max();
    if winning_rank.is_none()
        && !input.evidence.iter().any(|evidence| {
            evidence.current && evidence.state == Some(HealthAssessmentState::Unknown)
        })
    {
        input.evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::HealthContract,
            raw: Value::String("no classifying source".to_string()),
            state: Some(HealthAssessmentState::Unknown),
            current: true,
            reason: "No current source can classify health".to_string(),
        });
    }
    let winning_evidence: Vec<&HealthAssessmentEvidence> = if let Some(rank) = winning_rank {
        input
            .evidence
            .iter()
            .filter(|evidence| {
                evidence.current
                    && evidence.state.and_then(HealthAssessmentState::rank) == Some(rank)
            })
            .collect()
    } else {
        input
            .evidence
            .iter()
            .filter(|evidence| {
                evidence.current && evidence.state == Some(HealthAssessmentState::Unknown)
            })
            .collect()
    };
    let mut winning_sources = vec![];
    let mut reasons = vec![];
    for evidence in &winning_evidence {
        if !winning_sources.contains(&evidence.source) {
            winning_sources.push(evidence.source);
        }
        if !reasons.contains(&evidence.reason) {
            reasons.push(evidence.reason.clone());
        }
    }
    let state = winning_rank.map_or(HealthAssessmentState::Unknown, |rank| match rank {
        1 => HealthAssessmentState::Healthy,
        2 => HealthAssessmentState::NeedsAttention,
        _ => HealthAssessmentState::Degraded,
    });

    HealthAssessment {
        state,
        completeness,
        winning_sources,
        reasons,
        evidence: input.evidence,
    }
}

pub fn argo_health_assessment(
    health_status: Option<&str>,
    sync_status: Option<&str>,
) -> HealthAssessment {
    let mut evidence = vec![];
    if let Some(status) = health_status {
        evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::ArgoHealth,
            raw: Value::String(status.to_string()),
            state: match status {
                "Missing" | "Degraded" => Some(HealthAssessmentState::Degraded),
                "Progressing" => Some(HealthAssessmentState::NeedsAttention),
                "Healthy" => Some(HealthAssessmentState::Healthy),
                _ => Some(HealthAssessmentState::Unknown),
            },
            current: true,
            reason: format!("Argo CD health is {status}"),
        });
    }
    if let Some(status) = sync_status {
        evidence.push(HealthAssessmentEvidence {
            source: HealthAssessmentSource::ArgoSync,
            raw: Value::String(status.to_string()),
            state: match status {
                "OutOfSync" => Some(HealthAssessmentState::NeedsAttention),
                "Synced" => Some(HealthAssessmentState::Healthy),
                _ => Some(HealthAssessmentState::Unknown),
            },
            current: true,
            reason: format!("Argo CD sync is {status}"),
        });
    }
    evaluate_health(HealthAssessmentInput {
        recognized_semantics: true,
        provider_available: true,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evidence(
        source: HealthAssessmentSource,
        state: HealthAssessmentState,
        current: bool,
    ) -> HealthAssessmentEvidence {
        HealthAssessmentEvidence {
            source,
            raw: json!("raw status"),
            state: Some(state),
            current,
            reason: "test evidence".to_string(),
        }
    }

    #[test]
    fn ranks_current_sources_and_keeps_all_tied_winners() {
        let assessment = evaluate_health(HealthAssessmentInput {
            recognized_semantics: true,
            provider_available: true,
            evidence: vec![
                evidence(
                    HealthAssessmentSource::Kubernetes,
                    HealthAssessmentState::Degraded,
                    true,
                ),
                evidence(
                    HealthAssessmentSource::ArgoHealth,
                    HealthAssessmentState::Degraded,
                    true,
                ),
                evidence(
                    HealthAssessmentSource::ArgoSync,
                    HealthAssessmentState::NeedsAttention,
                    true,
                ),
            ],
        });

        assert_eq!(assessment.state, HealthAssessmentState::Degraded);
        assert_eq!(
            assessment.winning_sources,
            vec![
                HealthAssessmentSource::Kubernetes,
                HealthAssessmentSource::ArgoHealth
            ]
        );
    }

    #[test]
    fn historical_evidence_does_not_worsen_current_health() {
        let assessment = evaluate_health(HealthAssessmentInput {
            recognized_semantics: true,
            provider_available: true,
            evidence: vec![
                evidence(
                    HealthAssessmentSource::Kubernetes,
                    HealthAssessmentState::Healthy,
                    true,
                ),
                evidence(
                    HealthAssessmentSource::ArgoHealth,
                    HealthAssessmentState::Degraded,
                    false,
                ),
            ],
        });

        assert_eq!(assessment.state, HealthAssessmentState::Healthy);
        assert_eq!(
            assessment.winning_sources,
            vec![HealthAssessmentSource::Kubernetes]
        );
    }

    #[test]
    fn unavailable_provider_keeps_kubernetes_fallback_partial() {
        let assessment = evaluate_health(HealthAssessmentInput {
            recognized_semantics: true,
            provider_available: false,
            evidence: vec![evidence(
                HealthAssessmentSource::Kubernetes,
                HealthAssessmentState::Healthy,
                true,
            )],
        });

        assert_eq!(assessment.state, HealthAssessmentState::Healthy);
        assert_eq!(
            assessment.completeness,
            HealthAssessmentCompleteness::Partial
        );
        assert!(assessment
            .evidence
            .iter()
            .any(|evidence| evidence.source == HealthAssessmentSource::Provider));
    }

    #[test]
    fn argo_conflicts_follow_contract() {
        let missing = argo_health_assessment(Some("Missing"), Some("OutOfSync"));
        let progressing = argo_health_assessment(Some("Healthy"), Some("OutOfSync"));

        assert_eq!(missing.state, HealthAssessmentState::Degraded);
        assert_eq!(progressing.state, HealthAssessmentState::NeedsAttention);
    }

    #[test]
    fn unknown_and_not_evaluated_identify_their_source() {
        let unknown = evaluate_health(HealthAssessmentInput {
            recognized_semantics: true,
            provider_available: false,
            evidence: vec![],
        });
        let not_evaluated = evaluate_health(HealthAssessmentInput {
            recognized_semantics: false,
            provider_available: true,
            evidence: vec![],
        });

        assert_eq!(unknown.state, HealthAssessmentState::Unknown);
        assert_eq!(
            unknown.winning_sources,
            vec![HealthAssessmentSource::Provider]
        );
        assert_eq!(not_evaluated.state, HealthAssessmentState::NotEvaluated);
        assert_eq!(
            not_evaluated.winning_sources,
            vec![HealthAssessmentSource::HealthContract]
        );
    }
}
