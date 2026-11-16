import type { ClusterContext, KubeconfigSourcesSummary } from "@/lib/types";

/** The fixture kubeconfig file that defines one extra context, `ADDED_CONTEXT`. */
export const ADDED_CONTEXT_KUBECONFIG = "/fixtures/mock-added.kubeconfig";
const ADDED_CONTEXT = "mock-added";

/**
 * The source key of `source`. The backend's key hashes the env var and the added paths, so adding
 * or removing the fixture file changes it. Other paths keep the base key, so the sessions the mock
 * records under it stay in scope.
 */
export function kubeconfigSourceKeyMock(source: KubeconfigSourcesSummary): string {
	return hasAddedContext(source) ? `${source.sourceKey}|${ADDED_CONTEXT}` : source.sourceKey;
}

/** `contexts`, plus the fixture file's context while that file is an added path. */
export function kubeContextsMock(contexts: readonly ClusterContext[], source: KubeconfigSourcesSummary): ClusterContext[] {
	return hasAddedContext(source) ? [...contexts, { name: ADDED_CONTEXT, isCurrent: false }] : [...contexts];
}

function hasAddedContext(source: KubeconfigSourcesSummary): boolean {
	return source.paths.some((entry) => entry.path === ADDED_CONTEXT_KUBECONFIG);
}

/** Adds trimmed, non-empty paths once each, in the order the command receives them. */
export function addKubeconfigPathsMock(source: KubeconfigSourcesSummary, paths: readonly string[] = []): KubeconfigSourcesSummary {
	for (const path of paths.map((value) => value.trim())) {
		if (path && !source.paths.some((entry) => entry.path === path)) source.paths = [...source.paths, { path }];
	}
	return { ...source };
}

/** Orders the known paths as listed. Paths the list omits keep their relative order at the end. */
export function reorderKubeconfigPathsMock(source: KubeconfigSourcesSummary, paths: readonly string[] = []): KubeconfigSourcesSummary {
	const remaining = [...source.paths];
	const ordered = [];
	for (const path of paths) {
		const index = remaining.findIndex((entry) => entry.path === path);
		if (index >= 0) ordered.push(...remaining.splice(index, 1));
	}
	source.paths = [...ordered, ...remaining];
	return { ...source };
}
