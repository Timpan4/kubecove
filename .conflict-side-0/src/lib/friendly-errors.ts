import type { AppError } from "./types";
import type { RbacAccessReviewTarget } from "./rbac-types";

export type FriendlyErrorMode = "full" | "compact";

export type FriendlyErrorBucket =
	| "kubeconfigConfig"
	| "forbiddenRbac"
	| "notFoundStale"
	| "validation"
	| "serialization"
	| "admissionPolicy"
	| "immutableField"
	| "fieldManagerConflict"
	| "liveSessionTargetUnavailable"
	| "providerDiscoveryUnavailable"
	| "networkTransient"
	| "unknown";

export type FriendlyErrorOperation =
	| "contextLoad"
	| "detailsLoad"
	| "eventsLoad"
	| "resourcesLoad"
	| "surfaceLoad"
	| "yamlLoad"
	| "yamlDryRun"
	| "yamlApply"
	| "liveSession"
	| "exec"
	| "portForward"
	| "providerDetection"
	| "partial";

export interface FriendlyErrorContext {
	operation?: FriendlyErrorOperation;
	fallbackTitle?: string;
	target?: string;
	partial?: boolean;
	requiredPermission?: RbacAccessReviewTarget;
	permissionSourceLabel?: string;
}

export interface FriendlyErrorPresentation {
	bucket: FriendlyErrorBucket;
	title: string;
	summary: string;
	next?: string;
	stillWorks?: string;
	technicalDetail: string;
	copyText: string;
	tone: "destructive" | "warning";
}

const MESSAGE_BUCKETS: Array<[FriendlyErrorBucket, RegExp]> = [
	[
		"immutableField",
		/pod updates may not change fields|field is immutable|fieldvalueforbidden|immutable/i,
	],
	[
		"fieldManagerConflict",
		/fieldmanagerconflict|field manager conflict|conflicts? with|apply failed with conflicts/i,
	],
	[
		"admissionPolicy",
		/admission webhook|denied the request|podsecurity|policy.*denied|violates/i,
	],
	["forbiddenRbac", /\bforbidden\b|\b403\b|cannot (?:get|list|watch|create|patch|update|delete)\b/i],
	["notFoundStale", /\bnot ?found\b|\b404\b|no such resource/i],
	[
		"kubeconfigConfig",
		/kubeconfig|current context|failed to infer config|no configuration has been provided|invalid configuration|context .* not found/i,
	],
	[
		"liveSessionTargetUnavailable",
		/no ready pod|no matching pods?|selector.*matched no pods?|container .* not found|address already in use|port .*already in use|port-forward|exec session/i,
	],
	[
		"providerDiscoveryUnavailable",
		/discovery|customresourcedefinition|\bcrd\b|metrics\.k8s\.io|metrics api|provider.*unavailable|provider.*detect/i,
	],
	[
		"networkTransient",
		/timed? out|connection refused|connection reset|unreachable|temporarily unavailable|transport|tls|certificate|dns|i\/o timeout/i,
	],
	["serialization", /serialize|serialization|deserialize|json|yaml.*format|parse/i],
	["validation", /validation|invalid|required|must be|missing/i],
];

const KIND_BUCKETS: Record<string, FriendlyErrorBucket> = {
	admissionDenied: "admissionPolicy",
	applyImmutableField: "immutableField",
	fieldManagerConflict: "fieldManagerConflict",
	forbidden: "forbiddenRbac",
	immutableField: "immutableField",
	invalidResource: "validation",
	kubeconfig: "kubeconfigConfig",
	kubeconfigConfig: "kubeconfigConfig",
	liveSessionTargetUnavailable: "liveSessionTargetUnavailable",
	network: "networkTransient",
	networkTransient: "networkTransient",
	notFound: "notFoundStale",
	providerDiscoveryUnavailable: "providerDiscoveryUnavailable",
	providerUnavailable: "providerDiscoveryUnavailable",
	serialization: "serialization",
	validation: "validation",
};

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

export function messageFromFriendlyError(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;
	if (isRecord(error) && typeof error.message === "string") return error.message;
	return "Unknown error";
}

function appErrorKind(error: unknown): string | null {
	if (!isRecord(error)) return null;
	const candidate = error as Partial<AppError>;
	return typeof candidate.kind === "string" ? candidate.kind : null;
}

export function friendlyErrorBucket(error: unknown): FriendlyErrorBucket {
	const kind = appErrorKind(error);
	if (kind && KIND_BUCKETS[kind]) return KIND_BUCKETS[kind];

	const message = messageFromFriendlyError(error);
	for (const [bucket, pattern] of MESSAGE_BUCKETS) {
		if (pattern.test(message)) return bucket;
	}
	return "unknown";
}

export function requiredPermissionForFriendlyError(
	error: unknown,
	context: FriendlyErrorContext,
): RbacAccessReviewTarget | null {
	return friendlyErrorBucket(error) === "forbiddenRbac"
		? (context.requiredPermission ?? null)
		: null;
}

function operationSubject(operation: FriendlyErrorOperation | undefined): string {
	switch (operation) {
		case "contextLoad":
			return "cluster contexts";
		case "detailsLoad":
			return "resource details";
		case "eventsLoad":
			return "events";
		case "resourcesLoad":
			return "resources";
		case "surfaceLoad":
			return "this view";
		case "yamlLoad":
			return "YAML";
		case "yamlDryRun":
			return "dry run";
		case "yamlApply":
			return "YAML apply";
		case "exec":
			return "exec session";
		case "portForward":
			return "port-forward";
		case "providerDetection":
			return "provider data";
		case "partial":
			return "some data";
		default:
			return "this request";
	}
}

function partialTitle(context: FriendlyErrorContext, fallback: string): string {
	if (!context.partial) return fallback;
	return context.fallbackTitle ?? `Some ${operationSubject(context.operation)} could not load`;
}

export function friendlyError(
	error: unknown,
	context: FriendlyErrorContext = {},
): FriendlyErrorPresentation {
	const bucket = friendlyErrorBucket(error);
	const technicalDetail = messageFromFriendlyError(error);
	const target = context.target ? ` for ${context.target}` : "";
	const subject = operationSubject(context.operation);
	const partial = context.partial === true;
	const tone = partial ? "warning" : "destructive";

	const base = {
		bucket,
		technicalDetail,
		copyText: technicalDetail,
		tone,
	} satisfies Pick<
		FriendlyErrorPresentation,
		"bucket" | "technicalDetail" | "copyText" | "tone"
	>;

	switch (bucket) {
		case "kubeconfigConfig":
			return {
				...base,
				title: partialTitle(context, "KubeCove could not load the selected kubeconfig"),
				summary: "The selected kubeconfig source is missing, invalid, or has no usable context.",
				next: "Choose another kubeconfig source or fix the selected kubeconfig, then retry.",
			};
		case "forbiddenRbac":
			return {
				...base,
				title: partialTitle(context, "KubeCove does not have permission for this"),
				summary: `The current context can connect, but Kubernetes RBAC denied ${subject}${target}.`,
				next: "Use a context with the required permission or ask a cluster admin to grant access.",
			};
		case "notFoundStale":
			return {
				...base,
				title: partialTitle(context, "This Kubernetes object is no longer available"),
				summary: `Kubernetes could not find ${subject}${target}. It may have been deleted or replaced.`,
				next: "Refresh the view and select the current object if it still exists.",
			};
		case "validation":
			return {
				...base,
				title: partialTitle(context, "Kubernetes could not accept this request"),
				summary: `The request for ${subject}${target} is missing required data or has invalid values.`,
				next: "Check the target, namespace, and YAML fields, then retry.",
			};
		case "serialization":
			return {
				...base,
				title: partialTitle(context, "KubeCove could not format this data"),
				summary: `KubeCove received data for ${subject}${target}, but could not convert it for this view.`,
				next: "Retry after refreshing. If it repeats, copy the technical detail for debugging.",
			};
		case "admissionPolicy":
			return {
				...base,
				title: partialTitle(context, "Cluster policy rejected this change"),
				summary: `Kubernetes reached an admission policy while handling ${subject}${target}.`,
				next: "Adjust the YAML to satisfy the policy, or use the cluster-approved workflow.",
			};
		case "immutableField":
			return {
				...base,
				title: partialTitle(context, "This field cannot be changed in place"),
				summary: `Kubernetes blocked ${subject}${target} because one or more existing fields are immutable.`,
				next: "Create a replacement resource or recreate it outside KubeCove.",
			};
		case "fieldManagerConflict":
			return {
				...base,
				title: partialTitle(context, "Another manager owns part of this resource"),
				summary: `Kubernetes blocked ${subject}${target} because another field manager owns one or more fields.`,
				next: "Retry with force conflicts only if you mean to take ownership of those fields.",
			};
		case "liveSessionTargetUnavailable":
			return {
				...base,
				title: partialTitle(context, "The live-session target is unavailable"),
				summary: `KubeCove could not start or update ${subject}${target} against the selected target.`,
				next: "Check that the Pod or Service target exists, has a ready Pod, and the local port is free.",
			};
		case "providerDiscoveryUnavailable":
			return {
				...base,
				title: partialTitle(context, "Some provider data is unavailable"),
				summary: `KubeCove could not discover or read part of ${subject}${target}.`,
				stillWorks: partial
					? "Other loaded data is still usable."
					: "The rest of the app is still usable.",
				next: "Refresh after the provider CRDs or metrics API are available.",
			};
		case "networkTransient":
			return {
				...base,
				title: partialTitle(context, "KubeCove could not reach the Kubernetes API"),
				summary: `The connection failed while loading ${subject}${target}.`,
				next: "Check the cluster connection or VPN, then retry.",
			};
		case "unknown":
			return {
				...base,
				title: context.fallbackTitle ?? "KubeCove could not simplify this error yet",
				summary: "The original technical error is still available below.",
				next: "Copy the detail if this needs debugging.",
			};
	}
}
