import type { Query, QueryClient } from "@tanstack/svelte-query";
import { isFiniteReadQuery } from "./finite-read-lifecycle";
import { friendlyErrorBucket } from "./friendly-errors";
import { messageFromError } from "./error-redaction";
import { kubeconfigSourceKey } from "./settings";
import { refreshResourceCache } from "./tauri";
import type { TauriClient } from "./tauri";
import type { WatchResourceKey } from "./types";

export function isCurrentViewRead(
	query: Query,
	context: string,
	source?: string,
): boolean {
	const key = query.queryKey;
	const contextIndex =
		key[0] === "resource-details" || key[0] === "resource-yaml" ? 3 : 2;
	return (
		isFiniteReadQuery(query) &&
		query.isActive() &&
		key[1] === kubeconfigSourceKey(source) &&
		key[contextIndex] === context
	);
}

export async function refreshCurrentView({
	client,
	queryClient,
	clusterContext,
	kubeconfigEnvVar,
	keys,
	namespaces,
}: {
	client: TauriClient;
	queryClient: QueryClient;
	clusterContext: string;
	kubeconfigEnvVar?: string;
	keys: WatchResourceKey[];
	namespaces: string[];
}): Promise<string[]> {
	// Capture the exact active reads. Navigation during refresh must not broaden its scope.
	const queries = new Set(
		queryClient
			.getQueryCache()
			.findAll({
				predicate: (query) =>
					isCurrentViewRead(query, clusterContext, kubeconfigEnvVar),
			}),
	);
	const predicate = (query: Query) => queries.has(query);
	await queryClient.cancelQueries({ predicate });
	await refreshResourceCache(
		client,
		clusterContext,
		keys,
		namespaces,
		kubeconfigEnvVar,
	);
	// Keep the last displayed data while making every selected cache entry stale.
	const results = await Promise.allSettled([...queries].map((query) =>
		queryClient.invalidateQueries(
			{ predicate: (current) => current === query, refetchType: "active" },
			{ throwOnError: true },
		),
	));
	const warnings: string[] = [];
	for (const result of results) {
		if (result.status !== "rejected") continue;
		if (friendlyErrorBucket(result.reason) !== "forbiddenRbac") throw result.reason;
		warnings.push(messageFromError(result.reason));
	}
	return [...new Set(warnings)];
}
