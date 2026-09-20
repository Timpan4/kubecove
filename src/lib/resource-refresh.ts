import type { Query, QueryClient } from "@tanstack/svelte-query";
import { isFiniteReadQuery } from "./finite-read-lifecycle";
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
}): Promise<void> {
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
	await queryClient.invalidateQueries(
		{ predicate, refetchType: "active" },
		{ throwOnError: true },
	);
}
