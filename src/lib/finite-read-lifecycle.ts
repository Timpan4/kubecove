import type { Query, QueryClient, QueryKey } from "@tanstack/svelte-query";
import type { CancelBackendRequestsResult, CancellableRequest } from "./cancellation-types";
import { diagnosticLog } from "./diagnostics";

// This module owns TanStack query metadata (ADR 0016), so it types it for every query.
type QueryMetadata = {
	finiteRead?: true;
	/** Backend scope of the query's in-flight request; set only on backend-cancellable reads. */
	cancelScope?: string;
	/** Diagnostics event prefix, for example "resources.topology". */
	label?: string;
	namespace?: string;
};

declare module "@tanstack/svelte-query" {
	interface Register {
		queryMeta: QueryMetadata;
	}
}

type FiniteReadMetadata = QueryMetadata & { finiteRead: true };

let requestSequence = 0;

/** Request identity for one backend call; TanStack reads get theirs from `finiteRead`. */
export function createFiniteReadRequest(
	cancelScope: string,
	label: string,
): CancellableRequest {
	requestSequence = (requestSequence + 1) % Number.MAX_SAFE_INTEGER;
	return {
		cancelScope,
		requestId: `${label}-${Date.now().toString(36)}-${requestSequence.toString(36)}`,
	};
}

export function finiteReadMeta(
	meta: Omit<FiniteReadMetadata, "finiteRead"> = {},
): FiniteReadMetadata {
	return { ...meta, finiteRead: true };
}

export function isFiniteReadQuery(query: Query): boolean {
	return query.options.meta?.finiteRead === true;
}

// Commands reject with AppError values; kind `cancelled` is intentional cancellation (ADR 0009).
function isCancelled<Cause>(cause: Cause): boolean {
	return cause instanceof Object && "kind" in cause && cause.kind === "cancelled";
}

/**
 * Backend scope of the finite read for `queryKey`. A read outside TanStack Query that
 * belongs to that query uses it, so the query's cleanup cancels both.
 */
export function finiteReadScope(queryKey: QueryKey): string {
	// Catalog keys hold only primitives and arrays, so JSON is a stable identity.
	return JSON.stringify(queryKey);
}

export interface FiniteRead<T, Key extends QueryKey> {
	queryKey: Key;
	label: string;
	read: (request: CancellableRequest) => Promise<T>;
}

/**
 * TanStack key, queryFn, and metadata for one backend-cancellable finite read.
 * Spread the remaining query options beside it. The installed lifecycle cancels
 * the read once its query has no observers.
 */
export function finiteRead<T, Key extends QueryKey>({ queryKey, label, read }: FiniteRead<T, Key>) {
	const cancelScope = finiteReadScope(queryKey);
	return {
		queryKey,
		// Never read the query signal: TanStack would then abort on the last observer
		// removal itself, before the next-task check can keep a remounted read.
		queryFn: async () => {
			try {
				return await read(createFiniteReadRequest(cancelScope, label));
			} catch (error) {
				if (isCancelled(error)) diagnosticLog(`${label}.cancel`);
				throw error;
			}
		},
		meta: finiteReadMeta({ cancelScope, label }),
	};
}

/**
 * Cancels a backend-cancellable finite read when its query loses its last observer
 * and stays unobserved into the next task, in ADR 0009 order: frontend query, then
 * the typed backend scope. Returns the unsubscribe function.
 */
export function installFiniteReadLifecycle(
	queryClient: QueryClient,
	cancelBackend: (cancelScope: string) => Promise<CancelBackendRequestsResult>,
): () => void {
	const cache = queryClient.getQueryCache();
	const pending = new Map<string, ReturnType<typeof setTimeout>>();
	const unobserved = (queryKey: QueryKey) =>
		(cache.find({ queryKey, exact: true })?.getObserversCount() ?? 0) === 0;

	async function cancel(queryKey: QueryKey, cancelScope: string, label: string) {
		await queryClient.cancelQueries({ queryKey, exact: true });
		// A remount during frontend cancellation owns a new backend request in this scope.
		if (!unobserved(queryKey)) return;
		try {
			const result = await cancelBackend(cancelScope);
			if (result.cancelled > 0) diagnosticLog(`${label}.cancel`, { cancelled: result.cancelled });
		} catch (error) {
			diagnosticLog(`${label}.cancel.error`, {
				error: error instanceof Error ? error.message : String(error),
			});
		}
	}

	const unsubscribe = cache.subscribe((event) => {
		if (event.type !== "observerRemoved") return;
		const { query } = event;
		const { cancelScope, label = "finite-read" } = query.options.meta ?? {};
		if (!isFiniteReadQuery(query) || cancelScope === undefined) return;
		if (query.getObserversCount() > 0) return;
		clearTimeout(pending.get(query.queryHash));
		// One task lets a remount or key change in the same turn keep the read.
		const timer = setTimeout(() => {
			pending.delete(query.queryHash);
			if (unobserved(query.queryKey)) void cancel(query.queryKey, cancelScope, label);
		}, 0);
		pending.set(query.queryHash, timer);
	});
	return () => {
		unsubscribe();
		for (const timer of pending.values()) clearTimeout(timer);
		pending.clear();
	};
}
