import {
	type FriendlyErrorBucket,
	friendlyErrorBucket,
	messageFromFriendlyError,
} from "@/lib/friendly-errors";
import {
	isClusterScopedKind,
	type KindFetchKey,
	kindFetchKeys,
	kindListRequest,
} from "@/lib/kind-identity";
import {
	createTauriClient,
	listResourceScope,
} from "@/lib/tauri";
import type {
	ResourceListRequest,
	ResourceSummary,
	ResourceScopeFailure,
} from "@/lib/types";
import type { WorkspaceScope } from "@/lib/workspaces";
import { workspaceScopeContexts } from "@/lib/workspaces";

export interface WorkspaceFetchPlan {
	clusterContext: string;
	requests: ResourceListRequest[];
}

export interface WorkspaceResourceResult {
	resources: ResourceSummary[];
	failures: ResourceScopeFailure[];
	coverage: WorkspaceFetchPlan[];
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

export function buildWorkspaceFetchKeys(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
): KindFetchKey[] {
	const namespaceSet = availableNamespaces ? new Set(availableNamespaces) : null;
	const namespaces = namespaceSet
		? scope.namespaces.filter((namespace) => namespaceSet.has(namespace))
		: scope.namespaces;
	// Saved namespaces that no longer exist read nothing rather than every namespace.
	const kinds = scope.namespaces.length > 0 && namespaces.length === 0
		? scope.kinds.filter(isClusterScopedKind)
		: scope.kinds;
	return kindFetchKeys(namespaces, kinds);
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
			requests: buildWorkspaceFetchKeys(scope, namespaceScope).map(kindListRequest),
		};
	});
}

export async function fetchWorkspaceResources(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
	kubeconfigEnvVar?: string,
): Promise<WorkspaceResourceResult> {
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
	return {
		resources: results.flatMap((result) => result.status === "fulfilled" ? result.value.resources : []),
		failures: results.flatMap((result) => result.status === "fulfilled" ? result.value.failures : []),
		coverage: results.flatMap((result, index) => result.status === "fulfilled"
			? [{ clusterContext: plans[index].clusterContext, requests: result.value.completedRequests }]
			: []),
	};
}
