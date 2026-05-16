import {
	createTauriClient,
	listDynamicResources,
	listResources,
} from "@/lib/tauri";
import {
	CLUSTER_SCOPED_KINDS,
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
		if (namespaces.length === 0) return [];
		return namespaces.map((namespace) => ({ kind, namespace }));
	});
}

export async function fetchWorkspaceResources(
	scope: WorkspaceScope,
	availableNamespaces?: string[],
): Promise<ResourceSummary[]> {
	const client = createTauriClient();
	const fetchKeys = buildWorkspaceFetchKeys(scope, availableNamespaces);
	const results = await Promise.all(
		fetchKeys.map(({ kind, namespace }) => {
			if (typeof kind === "string") {
				return listResources(client, scope.clusterContext, kind, namespace);
			}
			return listDynamicResources(client, scope.clusterContext, kind, namespace);
		}),
	);
	return results.flat();
}
