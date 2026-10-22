import type { Query, QueryClient } from "@tanstack/svelte-query";
import { friendlyErrorBucket } from "./friendly-errors";
import { messageFromError } from "./error-redaction";
import { queryMatchers, type QueryMatch } from "./queryKeys";
import { refreshResourceCache } from "./tauri";
import type { TauriClient } from "./tauri";
import type { WatchResourceKey } from "./types";

export async function refreshCurrentView({
	client,
	queryClient,
	clusterContext,
	kubeconfigEnvVar,
	keys,
	namespaces,
	reads,
	cancelRefetch = true,
}: {
	client: TauriClient;
	queryClient: QueryClient;
	clusterContext: string;
	kubeconfigEnvVar?: string;
	keys: WatchResourceKey[];
	namespaces: string[];
	/** Permission recovery limits invalidation to these reads. */
	reads?: readonly QueryMatch[];
	/** Automatic recovery waits for running reads before clearing caches. */
	cancelRefetch?: boolean;
}): Promise<string[]> {
	// Capture the exact active reads. Navigation during refresh must not broaden its scope.
	const inCurrentView = queryMatchers.currentView(clusterContext, kubeconfigEnvVar);
	const queries = new Set(
		queryClient
			.getQueryCache()
			.findAll({
				predicate: (query) => inCurrentView(query) && (!reads || reads.some((read) => read(query))),
			}),
	);
	const predicate = (query: Query) => queries.has(query);
	if (!cancelRefetch) {
		await Promise.allSettled([...queries].map((query) => query.promise));
	} else await queryClient.cancelQueries({ predicate });
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
			{ throwOnError: true, cancelRefetch },
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
