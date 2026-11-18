import type { Query, QueryClient } from "@tanstack/svelte-query";
import { friendlyErrorBucket } from "./friendly-errors";
import { messageFromError } from "./error-redaction";
import { queryMatchers, type QueryMatch } from "./queryKeys";
import { refreshResourceCache } from "./tauri";
import type { TauriClient } from "./tauri";
import type { WatchResourceKey } from "./types";

const deferredInvalidations = new WeakMap<Query, Promise<unknown>>();
const manualReads = new WeakMap<Query, { revision: number; pending: number }>();

export async function refreshCurrentView({
	client,
	queryClient,
	clusterContext,
	kubeconfigEnvVar,
	keys,
	namespaces,
	reads,
	cancelRefetch = true,
	independentReads = [],
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
	/** Reads refreshed after eviction without holding up watch catch-up. */
	independentReads?: readonly QueryMatch[];
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
	const shared: Query[] = [];
	const independent = new Map<Query, number>();
	const manual: { pending: number }[] = [];
	for (const query of queries) {
		let state = manualReads.get(query);
		if (cancelRefetch) {
			state ??= { revision: 0, pending: 0 };
			manualReads.set(query, state);
			state.revision++;
			state.pending++;
			manual.push(state);
			deferredInvalidations.delete(query);
		}
		if (!cancelRefetch && independentReads.some((match) => match(query))) {
			if (!state?.pending) independent.set(query, state?.revision ?? 0);
		} else {
			shared.push(query);
		}
	}
	try {
		if (cancelRefetch) await queryClient.cancelQueries({ predicate: (query) => queries.has(query) });
		else await Promise.allSettled(shared.map((query) => query.promise));
		await refreshResourceCache(
			client,
			clusterContext,
			keys,
			namespaces,
			kubeconfigEnvVar,
		);
		// Keep the last displayed data while making every selected cache entry stale.
		const invalidate = (query: Query, throwOnError: boolean) =>
			queryClient.invalidateQueries(
				{ predicate: (current) => current === query, refetchType: "active" },
				{ throwOnError, cancelRefetch },
			);
		for (const [query, revision] of independent) {
			if ((manualReads.get(query)?.revision ?? 0) !== revision || deferredInvalidations.has(query)) continue;
			const settled = Promise.allSettled([query.promise]);
			deferredInvalidations.set(query, settled);
			void settled.then(() => {
				if (deferredInvalidations.get(query) !== settled) return;
				deferredInvalidations.delete(query);
				return invalidate(query, false);
			});
		}
		const results = await Promise.allSettled(shared.map((query) => invalidate(query, true)));
		const warnings: string[] = [];
		for (const result of results) {
			if (result.status !== "rejected") continue;
			if (friendlyErrorBucket(result.reason) !== "forbiddenRbac") throw result.reason;
			warnings.push(messageFromError(result.reason));
		}
		return [...new Set(warnings)];
	} finally {
		for (const state of manual) state.pending--;
	}
}
