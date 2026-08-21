import { messageFromFriendlyError } from "@/lib/friendly-errors";
import {
	createTauriClient,
	listResourceScope,
} from "@/lib/tauri";
import {
	CLUSTER_SCOPED_KINDS,
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
	readonly kind?: string;

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

		const firstReason = failures[0]?.reason;
		if (
			typeof firstReason === "object" &&
			firstReason !== null &&
			"kind" in firstReason &&
			typeof firstReason.kind === "string"
		) {
			this.kind = firstReason.kind;
		}
	}
}

function isClusterScopedKind(kind: ResourceKindSelection): boolean {
	if (typeof kind !== "string") return !kind.namespaced;
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
	if (typeof kind === "string") {
		return { kind, namespace };
	}
	return { resourceKind: kind, namespace };
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
