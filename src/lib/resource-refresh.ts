import type { Query, QueryClient } from "@tanstack/svelte-query";
import { friendlyErrorBucket } from "./friendly-errors";
import { messageFromError } from "./error-redaction";
import { queryMatchers, type QueryMatch } from "./queryKeys";
import { refreshResourceCache } from "./tauri";
import type { TauriClient } from "./tauri";
import type { WatchResourceKey } from "./types";

interface ReadRefresh {
	manual: number;
	manualRevision: number;
	waiting: boolean;
	deferred?: { manualRevision: number; refetch: () => Promise<void> };
}

const readsInRefresh = new WeakMap<Query, ReadRefresh>();

function refreshState(query: Query): ReadRefresh {
	const state = readsInRefresh.get(query) ?? { manual: 0, manualRevision: 0, waiting: false };
	readsInRefresh.set(query, state);
	return state;
}

export function manualRefreshOwns(query: Query): boolean {
	return Boolean(readsInRefresh.get(query)?.manual);
}

function flushDeferredRead(query: Query, state: ReadRefresh) {
	const deferred = state.deferred;
	if (!deferred || state.manual || state.waiting) return;
	// A manual read that started after this eviction already covers it.
	if (state.manualRevision !== deferred.manualRevision) {
		state.deferred = undefined;
		return;
	}
	if (query.state.fetchStatus !== "idle" && query.promise) {
		state.waiting = true;
		void Promise.allSettled([query.promise]).then(() => {
			state.waiting = false;
			flushDeferredRead(query, state);
		});
		return;
	}
	state.deferred = undefined;
	void deferred.refetch();
}

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
	const independent: Query[] = [];
	for (const query of queries) {
		if (cancelRefetch) refreshState(query).manual++;
		if (!cancelRefetch && independentReads.some((match) => match(query))) independent.push(query);
		else shared.push(query);
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
		for (const query of independent) {
			const state = refreshState(query);
			state.deferred = { manualRevision: state.manualRevision, refetch: () => invalidate(query, false) };
			flushDeferredRead(query, state);
		}
		const results = await Promise.allSettled(shared.map((query) => {
			if (cancelRefetch && query.isActive()) refreshState(query).manualRevision++;
			return invalidate(query, true);
		}));
		const warnings: string[] = [];
		for (const result of results) {
			if (result.status !== "rejected") continue;
			if (friendlyErrorBucket(result.reason) !== "forbiddenRbac") throw result.reason;
			warnings.push(messageFromError(result.reason));
		}
		return [...new Set(warnings)];
	} finally {
		if (cancelRefetch) {
			for (const query of queries) {
				const state = refreshState(query);
				state.manual--;
				flushDeferredRead(query, state);
			}
		}
	}
}
