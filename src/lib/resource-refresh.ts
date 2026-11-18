import type { Query, QueryClient } from "@tanstack/svelte-query";
import { createSubscriber } from "svelte/reactivity";
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
	subscribe: () => void;
	notify?: () => void;
}

const readsInRefresh = new WeakMap<Query, ReadRefresh>();

function refreshState(query: Query): ReadRefresh {
	const existing = readsInRefresh.get(query);
	if (existing) return existing;
	const state: ReadRefresh = {
		manual: 0,
		manualRevision: 0,
		waiting: false,
		subscribe: createSubscriber((notify) => {
			state.notify = notify;
			return () => { state.notify = undefined; };
		}),
	};
	readsInRefresh.set(query, state);
	return state;
}

export function readRefreshPending(query: Query | undefined): boolean {
	if (!query) return false;
	const state = refreshState(query);
	state.subscribe();
	return state.manual > 0 || state.deferred !== undefined;
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
		state.notify?.();
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
	state.notify?.();
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
		if (cancelRefetch) {
			const state = refreshState(query);
			state.manual++;
			state.notify?.();
		}
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
		// Watch changes may have started a new read while cache eviction was pending.
		if (cancelRefetch) await queryClient.cancelQueries({ predicate: (query) => queries.has(query) });
		// Keep the last displayed data while making every selected cache entry stale.
		const invalidate = (query: Query, throwOnError: boolean) =>
			queryClient.invalidateQueries(
				{ predicate: (current) => current === query, refetchType: "active" },
				{ throwOnError, cancelRefetch },
			);
		for (const query of independent) {
			const state = refreshState(query);
			state.deferred = { manualRevision: state.manualRevision, refetch: () => invalidate(query, false) };
			state.notify?.();
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
				state.notify?.();
			}
		}
	}
}
