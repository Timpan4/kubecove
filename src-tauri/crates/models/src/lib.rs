mod argo;
mod cancellation;
mod cluster;
mod command_contract;
mod diagnostics;
mod discovery;
mod error;
mod events;
mod flux;
mod health;
mod helm;
mod incidents;
mod mcp;
mod metrics;
mod namespace;
mod operations;
mod rbac;
mod resource;
mod resource_actions;
mod sessions;
mod streams;
mod tunnel;
mod usage;

pub use argo::normalize_service_tunnel_endpoint;
pub use argo::{
    ArgoAppProjectDetails, ArgoAppProjectSummary, ArgoApplicationDetails, ArgoApplicationHistory,
    ArgoApplicationInspector, ArgoApplicationRef, ArgoApplicationSetDetails,
    ArgoApplicationSetSummary, ArgoApplicationSourceSummary, ArgoApplicationSummary,
    ArgoConfirmedTarget, ArgoConnectedFallback, ArgoConnectionProfile, ArgoConnectionStatus,
    ArgoInspectionFailure, ArgoManagedResource, ArgoOperationConfirmation, ArgoOperationPreflight,
    ArgoOperationRequest, ArgoOperationResult, ArgoResourceComparison, ArgoServerCapability,
    ArgoServerEndpoint, ArgoServiceTunnelUnavailableReason, ArgoTunnelTargetPreview,
};
pub use cancellation::{CancelBackendRequestsResult, CancelWorkspaceRequestsResult};
pub use cluster::ClusterContext;
pub use command_contract::{CommandArgs, CommandTransport, DECLARED_COMMANDS, DeclaredCommand};
pub use diagnostics::{
    BackendCacheDiagnosticSnapshot, BackendDiagnosticEvent, BackendDiagnosticField,
    BackendDiagnosticStatus,
};
pub use discovery::DiscoveredResourceKind;
pub use error::{AppError, AppErrorKind};
pub use error::{WorkspaceRequestCancelled, kube_error_kind};
pub use events::ResourceEventSummary;
pub use flux::{
    FluxDetectionSummary, FluxInventoryResource, FluxResourceDetails, FluxResourceKind,
    FluxResourceSummary,
};
pub use health::{
    HealthAssessment, HealthAssessmentCompleteness, HealthAssessmentEvidence,
    HealthAssessmentInput, HealthAssessmentSource, HealthAssessmentState,
    argo_application_set_health_assessment, argo_health_assessment, condition_evidence,
    evaluate_health,
};
pub use helm::{
    HelmManifestResourceSummary, HelmManifestSummary, HelmReconciliationResource,
    HelmReconciliationStatus, HelmReconciliationTotals, HelmReleaseDetails, HelmReleaseList,
    HelmReleaseReconciliation, HelmReleaseSummary, HelmValuesSummary,
};
pub use incidents::{
    IncidentCockpitItem, IncidentCockpitSummary, IncidentSeverity, IncidentSignalState,
    IncidentSignalSummary,
};
pub use mcp::McpStatus;
pub use metrics::{
    ResourceMetricSummary, ResourceMetricsAvailability, ResourceMetricsAvailabilityStatus,
    ResourceMetricsSummary,
};
pub use namespace::NamespaceSummary;
pub use operations::{ClusterOperationResult, ClusterOperationTarget};
pub use rbac::{
    RbacAccessReviewIdentity, RbacAccessReviewOutcome, RbacAccessReviewRequest,
    RbacAccessReviewResult, RbacAccessReviewTarget, RbacBindingSummary, RbacCoverageStatus,
    RbacFamily, RbacFamilyCoverage, RbacInspectionSummary, RbacNamespaceAccessSummary,
    RbacRequestMode, RbacRiskIndicator, RbacRiskLevel, RbacRoleSummary, RbacRuleSummary,
    RbacSubjectSummary, ServiceAccountSummary,
};
pub use resource::{
    DeploymentRevision, GitOpsOwnerSummary, KubernetesYamlLintDiagnostic, KubernetesYamlLintResult,
    KubernetesYamlLintSeverity, KubernetesYamlLintStatusNote, OwnerReferenceSummary,
    ResourceDetailsFull, ResourceHealth, ResourceListRequest, ResourceListVersion,
    ResourceScopeFailure, ResourceScopeResult, ResourceSummary, ResourceTopology, TopologyEdge,
    TopologyNode, TopologyRelation, VersionedList, YamlApplyPreview, YamlApplyRequest,
    YamlApplyResult, YamlApplyTarget, YamlEncoding, YamlViewMode,
};
pub use resource_actions::{
    ResourceActionContainer, ResourceActionController, ResourceActionEdits, ResourceActionEnv,
    ResourceActionKind, ResourceActionPermission, ResourceActionPreview, ResourceActionRequest,
    ResourceActionRevision, ResourceActionState, ResourceActionStateRequest, ResourceContainerEdit,
    ResourceEnvEdit,
};
pub use sessions::{
    LiveSessionCleanupRequest, LiveSessionCleanupResult, PodExecConfirmation,
    PodExecSessionMessage, PodExecSessionRequest, PodExecSessionStatus, PodExecSessionSummary,
    PodExecTerminalSize, PortForwardRequest, PortForwardSessionStatus, PortForwardSessionSummary,
};
pub use streams::{
    AggregatedLogStreamRequest, CHANNEL_MESSAGE_ENVELOPE_BYTES, LogLineSource,
    MAX_WEBSOCKET_MESSAGE_BYTES, PodLogStreamRequest, ResourceCacheRefreshResult,
    ResourceWatchStart, StreamMessage, StreamStatus, WatchResourceKey, WatchResourceKind,
    WatchResourceTarget,
};
pub use tunnel::{
    MAX_TUNNEL_FRAME_BYTES, MAX_TUNNEL_PAYLOAD_BYTES, decode_tunnel_frame, encode_tunnel_frame,
    is_tunnel_connection_id,
};
pub use usage::{AppUsageMemory, AppUsageMetrics, AppUsageMetricsBreakdown};
