import {
	type FriendlyErrorBucket,
	friendlyErrorBucket,
	messageFromFriendlyError,
} from "@/lib/friendly-errors";
import {
	createTauriClient,
	listResourceScope,
} from "@/lib/tauri";
import {
	CLUSTER_SCOPED_KINDS,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
	type ResourceListRequest,
	type ResourceSummary,
} from "@/lib/types";
import type { WorkspaceScope } from "@/lib/workspaces";
import { workspaceScopeContexts } from "@/lib/workspaces";

export interface WorkspaceFetchKey {
	kind: ResourceKindSelection;
	namespace?: string;
}

export interface WorkspaceFetchPlan {
	clusterContext: string;
	requests: ResourceListRequest[];
}

interface WorkspaceResourceFailure {
	clusterContext: string;
	reason: unknown;
}

export class WorkspaceResourceLoadError extends Error {
	readonly clusterContexts: string[];
	readonly failureBuckets: FriendlyErrorBucket[];
	readonly kind: FriendlyErrorBucket;

	constructor(failures: WorkspaceResourceFailure[]) {
		const detail = failures
			.map(
				({ clusterContext, reason }) =>
					`Context "${clusterContext}", operation "resource discovery": ${messageFromFriendlyError(reason)}`,
			)
			.join("\n");
		super(detail);
		this.name = "WorkspaceResourceLoadError";
		this.clusterContexts = failures.map(({ clusterContext }) => clusterContext);
		this.failureBuckets = failures.map(({ reason }) => friendlyErrorBucket(reason));
		const uniqueBuckets = new Set(this.failureBuckets);
		const onlyConnectionFailures = this.failureBuckets.every(
			(bucket) => bucket === "authentication" || bucket === "networkTransient",
		);
		this.kind = uniqueBuckets.size === 1
			? (this.failureBuckets[0] ?? "unknown")
			: onlyConnectionFailures
				? "mixedWorkspaceConnection"
				: "unknown";
	}
}

function isClusterScopedKind(kind: ResourceKindSelection): boolean {
	if (isDiscoveredResourceKind(kind)) return !kind.namespaced;
	// SAFETY: CLUSTER_SCOPED_KINDS contains only resource-kind strings.
	return (CLUSTER_SCOPED_KINDS as readonly string[]).includes(kind);
}

export function buildWorkspaceFetchKeys(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
): WorkspaceFetchKey[] {
	const namespaceSet = availableNamespaces ? new Set(availableNamespaces) : null;
	const namespaces = namespaceSet
		? scope.namespaces.filter((namespace) => namespaceSet.has(namespace))
		: scope.namespaces;

	return scope.kinds.flatMap((kind) => {
		if (isClusterScopedKind(kind)) return [{ kind }];
		if (scope.namespaces.length === 0) return [{ kind }];
		if (namespaces.length === 0) return [];
		return namespaces.map((namespace) => ({ kind, namespace }));
	});
}

export function workspaceFetchKeyRequest({
	kind,
	namespace,
}: WorkspaceFetchKey): ResourceListRequest {
	return isDiscoveredResourceKind(kind)
		? { resourceKind: kind, namespace }
		: { kind, namespace };
}

function isDiscoveredResourceKind(
	kind: ResourceKindSelection,
): kind is DiscoveredResourceKind {
	return Object(kind) === kind;
}

export function buildWorkspaceFetchPlans(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
): WorkspaceFetchPlan[] {
	const contexts = workspaceScopeContexts(scope);
	return contexts.map((clusterContext) => {
		const namespaceScope =
			contexts.length === 1 ? availableNamespaces : undefined;
		return {
			clusterContext,
			requests: buildWorkspaceFetchKeys(scope, namespaceScope).map(
				workspaceFetchKeyRequest,
			),
		};
	});
}

export async function fetchWorkspaceResources(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
	kubeconfigEnvVar?: string,
): Promise<ResourceSummary[]> {
	const client = createTauriClient();
	const plans = buildWorkspaceFetchPlans(scope, availableNamespaces);
	const results = await Promise.allSettled(
		plans.map((plan) =>
			listResourceScope(
				client,
				plan.clusterContext,
				plan.requests,
				kubeconfigEnvVar,
			),
		),
	);
	const failures = results.flatMap((result, index) =>
		result.status === "rejected"
			? [{ clusterContext: plans[index].clusterContext, reason: result.reason }]
			: [],
	);
	if (failures.length > 0) {
		throw new WorkspaceResourceLoadError(failures);
	}
	const rows = results.flatMap((result) =>
		result.status === "fulfilled" ? result.value : [],
	);
	return rows;
}
