import {
	createTauriClient,
	listResourceScope,
} from "@/lib/tauri";
import {
	CLUSTER_SCOPED_KINDS,
	type ResourceListRequest,
	type ResourceKindSelection,
	type ResourceSummary,
} from "@/lib/types";
import type { WorkspaceScope } from "@/lib/workspaces";

interface WorkspaceFetchKey {
	kind: ResourceKindSelection;
	namespace?: string;
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

function fetchKeyRequest({ kind, namespace }: WorkspaceFetchKey): ResourceListRequest {
	if (typeof kind === "string") {
		return { kind, namespace };
	}
	return { resourceKind: kind, namespace };
}

export async function fetchWorkspaceResources(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
): Promise<ResourceSummary[]> {
	const client = createTauriClient();
	const fetchKeys = buildWorkspaceFetchKeys(scope, availableNamespaces);
	return listResourceScope(
		client,
		scope.clusterContext,
		fetchKeys.map(fetchKeyRequest),
	);
}
