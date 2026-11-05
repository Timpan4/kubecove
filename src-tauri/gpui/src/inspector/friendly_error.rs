//! Classifies read failures the way `src/lib/friendly-errors.ts` does, so the
//! native error panel shows the same title, summary and next step as the web.
//! Pure logic only: `feedback.rs` renders the result.

use kubecove_lib::models::{AppErrorKind, RbacAccessReviewTarget, ResourceSummary};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bucket {
    KubeconfigConfig,
    Authentication,
    ForbiddenRbac,
    NotFoundStale,
    Validation,
    Serialization,
    AdmissionPolicy,
    ImmutableField,
    FieldManagerConflict,
    LiveSessionTargetUnavailable,
    ProviderDiscoveryUnavailable,
    NetworkTransient,
    UnsupportedOperation,
    Unknown,
}

/// The `FriendlyErrorOperation` values the native reads report. Each maps to
/// the subject the web's summary sentences use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Contexts,
    Details,
    Resources,
}

impl Operation {
    fn subject(self) -> &'static str {
        match self {
            Self::Contexts => "cluster contexts",
            Self::Details => "resource details",
            Self::Resources => "resources",
        }
    }
}

/// `FriendlyErrorContext` without the permission handoff, which the caller
/// supplies separately. `compact` is the presentation mode, which the view reads.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Context<'a> {
    pub operation: Option<Operation>,
    pub fallback_title: Option<&'a str>,
    pub target: Option<&'a str>,
    pub partial: bool,
    pub compact: bool,
}

#[derive(Debug)]
pub(crate) struct Presentation {
    pub bucket: Bucket,
    pub title: String,
    pub summary: String,
    pub still_works: Option<&'static str>,
    pub next: Option<&'static str>,
    /// Partial failures use the warning tone; the others are destructive.
    pub warning: bool,
}

/// `MESSAGE_BUCKETS`, in the web's priority order. The first match wins.
static MESSAGE_BUCKETS: LazyLock<Vec<(Bucket, Regex)>> = LazyLock::new(|| {
    const PATTERNS: [(Bucket, &str); 12] = [
        (
            Bucket::ImmutableField,
            r"(?i)pod updates may not change fields|field is immutable|fieldvalueforbidden|immutable",
        ),
        (
            Bucket::FieldManagerConflict,
            r"(?i)fieldmanagerconflict|field manager conflict|conflicts? with|apply failed with conflicts",
        ),
        (
            Bucket::AdmissionPolicy,
            r"(?i)admission webhook|denied the request|podsecurity|policy.*denied|violates",
        ),
        (
            Bucket::Authentication,
            r"(?i)\bunauthorized\b|\b401\b|authentication (?:failed|required)|invalid credentials?|expired (?:token|credential)|token has expired|exec (?:credential )?plugin.*(?:failed|error)",
        ),
        (
            Bucket::ForbiddenRbac,
            r"(?i)\bforbidden\b|\b403\b|cannot (?:get|list|watch|create|patch|update|delete)\b",
        ),
        (
            Bucket::NotFoundStale,
            r"(?i)\bnot ?found\b|\b404\b|no such resource",
        ),
        (
            Bucket::KubeconfigConfig,
            r"(?i)kubeconfig|current context|failed to infer config|no configuration has been provided|invalid configuration|context .* not found",
        ),
        (
            Bucket::LiveSessionTargetUnavailable,
            r"(?i)no ready pod|no matching pods?|selector.*matched no pods?|container .* not found|address already in use|port .*already in use|port-forward|exec session",
        ),
        (
            Bucket::ProviderDiscoveryUnavailable,
            r"(?i)discovery|customresourcedefinition|\bcrd\b|metrics\.k8s\.io|metrics api|provider.*unavailable|provider.*detect",
        ),
        (
            Bucket::NetworkTransient,
            r"(?i)serviceerror|client error \(connect\)|failed to connect|timed? out|connection refused|connection reset|unreachable|temporarily unavailable|transport|tls|certificate|dns|i/o timeout",
        ),
        (
            Bucket::Serialization,
            r"(?i)serialize|serialization|deserialize|json|yaml.*format|parse",
        ),
        (
            Bucket::Validation,
            r"(?i)validation|invalid|required|must be|missing",
        ),
    ];
    // Literal patterns from friendly-errors.ts; a failure here is a code bug.
    PATTERNS
        .into_iter()
        .map(|(bucket, pattern)| (bucket, Regex::new(pattern).expect("static pattern")))
        .collect()
});

/// `KIND_BUCKETS`, keyed by the camelCase `AppErrorKind` names.
fn kind_bucket(kind: AppErrorKind) -> Option<Bucket> {
    Some(match kind.as_str() {
        "admissionDenied" => Bucket::AdmissionPolicy,
        "authentication" | "credentialUnavailable" | "unauthorized" => Bucket::Authentication,
        "fieldManagerConflict" => Bucket::FieldManagerConflict,
        "forbidden" => Bucket::ForbiddenRbac,
        "immutableField" => Bucket::ImmutableField,
        "invalidResource" | "validation" => Bucket::Validation,
        "kubeconfig" | "kubeconfigConfig" => Bucket::KubeconfigConfig,
        "liveSessionTargetUnavailable" => Bucket::LiveSessionTargetUnavailable,
        "network" | "networkTransient" => Bucket::NetworkTransient,
        "notFound" => Bucket::NotFoundStale,
        "providerDiscoveryUnavailable" | "providerUnavailable" => {
            Bucket::ProviderDiscoveryUnavailable
        }
        "serialization" => Bucket::Serialization,
        "unsupportedOperation" => Bucket::UnsupportedOperation,
        _ => return None,
    })
}

/// `friendlyErrorBucket`: the error kind wins when it maps, otherwise the
/// message text decides.
pub(crate) fn bucket(kind: Option<AppErrorKind>, message: &str) -> Bucket {
    if let Some(bucket) = kind.and_then(kind_bucket) {
        return bucket;
    }
    MESSAGE_BUCKETS
        .iter()
        .find(|(_, pattern)| pattern.is_match(message))
        .map_or(Bucket::Unknown, |(bucket, _)| *bucket)
}

/// `requiredPermissionForResource` in `src/features/rbac/handoff.ts`.
pub(crate) fn resource_permission(
    resource: &ResourceSummary,
    verb: &str,
) -> Option<RbacAccessReviewTarget> {
    let plural = resource.plural.as_deref()?.trim();
    if plural.is_empty() || verb.trim().is_empty() {
        return None;
    }
    let namespace = resource
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if resource.namespaced == Some(true) && namespace.is_none() {
        return None;
    }
    let api_group = match resource.group.as_deref().map(str::trim) {
        Some(group) => group.to_owned(),
        None => resource
            .api_version
            .as_deref()
            .and_then(|version| version.split_once('/'))
            .map(|(group, _)| group.to_owned())
            .unwrap_or_default(),
    };
    Some(RbacAccessReviewTarget::Resource {
        verb: verb.trim().to_owned(),
        resource: plural.to_owned(),
        api_group,
        namespace: (resource.namespaced != Some(false))
            .then_some(namespace)
            .flatten(),
        subresource: None,
        name: Some(resource.name.clone()),
    })
}

fn subject(context: &Context<'_>) -> &'static str {
    context.operation.map_or("this request", Operation::subject)
}

fn target_suffix(context: &Context<'_>) -> String {
    context
        .target
        .map(|target| format!(" for {target}"))
        .unwrap_or_default()
}

/// `partialTitle`: partial failures may replace the fixed title.
fn partial_title(context: &Context<'_>, fallback: &str) -> String {
    if !context.partial {
        return fallback.to_owned();
    }
    context
        .fallback_title
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Some {} could not load", subject(context)))
}

/// `friendlyError`: builds the title, summary and next step for a failure.
pub(crate) fn present(
    kind: Option<AppErrorKind>,
    message: &str,
    context: &Context<'_>,
) -> Presentation {
    let bucket = bucket(kind, message);
    let subject = subject(context);
    let target = target_suffix(context);
    let warning = context.partial;
    let plain = |title: String, summary: String| Presentation {
        bucket,
        title,
        summary,
        still_works: None,
        next: None,
        warning,
    };
    let with_next = |title: String, summary: String, next: &'static str| Presentation {
        next: Some(next),
        ..plain(title, summary)
    };
    match bucket {
        Bucket::UnsupportedOperation => Presentation {
            title: context
                .fallback_title
                .unwrap_or("This operation is unavailable")
                .to_owned(),
            summary: message.to_owned(),
            ..plain(String::new(), String::new())
        },
        Bucket::Authentication => with_next(
            partial_title(context, "Kubernetes authentication failed"),
            format!("KubeCove could not authenticate while loading {subject}{target}."),
            "Sign in through the selected context's credential provider or refresh its credentials, then retry.",
        ),
        Bucket::KubeconfigConfig => with_next(
            partial_title(context, "KubeCove could not load the selected kubeconfig"),
            "The selected kubeconfig source is missing, invalid, or has no usable context."
                .to_owned(),
            "Choose another kubeconfig source or fix the selected kubeconfig, then retry.",
        ),
        Bucket::ForbiddenRbac => with_next(
            partial_title(context, "KubeCove does not have permission for this"),
            format!(
                "The current context can connect, but Kubernetes RBAC denied {subject}{target}."
            ),
            "Use a context with the required permission or ask a cluster admin to grant access.",
        ),
        Bucket::NotFoundStale => with_next(
            partial_title(context, "This Kubernetes object is no longer available"),
            format!(
                "Kubernetes could not find {subject}{target}. It may have been deleted or replaced."
            ),
            "Refresh the view and select the current object if it still exists.",
        ),
        Bucket::Validation => with_next(
            partial_title(context, "Kubernetes could not accept this request"),
            format!(
                "The request for {subject}{target} is missing required data or has invalid values."
            ),
            "Check the target, namespace, and YAML fields, then retry.",
        ),
        Bucket::Serialization => with_next(
            partial_title(context, "KubeCove could not format this data"),
            format!(
                "KubeCove received data for {subject}{target}, but could not convert it for this view."
            ),
            "Retry after refreshing. If it repeats, copy the technical detail for debugging.",
        ),
        Bucket::AdmissionPolicy => with_next(
            partial_title(context, "Cluster policy rejected this change"),
            format!("Kubernetes reached an admission policy while handling {subject}{target}."),
            "Adjust the YAML to satisfy the policy, or use the cluster-approved workflow.",
        ),
        Bucket::ImmutableField => with_next(
            partial_title(context, "This field cannot be changed in place"),
            format!(
                "Kubernetes blocked {subject}{target} because one or more existing fields are immutable."
            ),
            "Create a replacement resource or recreate it outside KubeCove.",
        ),
        Bucket::FieldManagerConflict => with_next(
            partial_title(context, "Another manager owns part of this resource"),
            format!(
                "Kubernetes blocked {subject}{target} because another field manager owns one or more fields."
            ),
            "Retry with force conflicts only if you mean to take ownership of those fields.",
        ),
        Bucket::LiveSessionTargetUnavailable => with_next(
            partial_title(context, "The live-session target is unavailable"),
            format!(
                "KubeCove could not start or update {subject}{target} against the selected target."
            ),
            "Check that the Pod or Service target exists, has a ready Pod, and the local port is free.",
        ),
        Bucket::ProviderDiscoveryUnavailable => Presentation {
            still_works: Some(if context.partial {
                "Other loaded data is still usable."
            } else {
                "The rest of the app is still usable."
            }),
            next: Some("Refresh after the provider CRDs or metrics API are available."),
            ..plain(
                partial_title(context, "Some provider data is unavailable"),
                format!("KubeCove could not discover or read part of {subject}{target}."),
            )
        },
        Bucket::NetworkTransient => with_next(
            partial_title(context, "KubeCove could not reach the Kubernetes API"),
            format!("The connection failed while loading {subject}{target}."),
            "Check the selected context's API endpoint, network or VPN path, and TLS certificate trust, then retry.",
        ),
        Bucket::Unknown => Presentation {
            next: Some("Copy the detail if this needs debugging."),
            ..plain(
                context
                    .fallback_title
                    .unwrap_or("KubeCove could not simplify this error yet")
                    .to_owned(),
                "The original technical error is still available below.".to_owned(),
            )
        },
    }
}
