import type { Query, QueryClient } from "@tanstack/svelte-query";
import type { CancellableRequest } from "./types";

const FINITE_READ_QUERY_ROOTS = [
	"kube-namespaces",
	"kube-resource-kinds",
	"kube-present-custom-resource-kinds",
	"resources",
	"deployment-revisions",
	"resource-topology",
	"resource-metrics",
	"resource-details",
	"resource-yaml",
	"resource-events",
	"argo-server-discovery",
	"argo-detect",
	"argo-apps",
	"argo-appsets",
	"argo-appprojects",
	"argo-app-details",
	"argo-workspace",
	"argo-appset-details",
	"argo-appproject-details",
	"flux-detect",
	"flux-resources",
	"flux-resource-details",
	"helm-releases",
	"helm-release-details",
	"helm-release-reconciliation",
	"rbac-inspection",
	"incident-cockpit",
] as const;

const FINITE_READ_META = { finiteRead: true } as const;

interface FiniteReadMetadata extends Record<string, unknown> {
	finiteRead: true;
	namespace?: string;
}

let requestSequence = 0;

export function createCancelScope(label: string, queryKey: readonly unknown[]): string {
	return `${label}:${JSON.stringify(queryKey)}`;
}

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

export function cancellableArg(
	request?: CancellableRequest,
): Partial<CancellableRequest> {
	return request ?? {};
}

export function finiteReadMeta(
	meta: Omit<FiniteReadMetadata, "finiteRead"> = {},
): FiniteReadMetadata {
	return { ...meta, ...FINITE_READ_META };
}

export function configureFiniteReadQueryDefaults(queryClient: QueryClient): void {
	for (const root of FINITE_READ_QUERY_ROOTS) {
		queryClient.setQueryDefaults([root], { meta: finiteReadMeta() });
	}
}

export function isFiniteReadQuery(query: Query): boolean {
	return query.options.meta?.finiteRead === true;
}

interface CleanupHooks<T> {
	onCancelled?: (result: T) => void;
	onError?: (cause: unknown) => void;
}

export interface FiniteReadCleanup<T> {
	schedule: (
		cancelScope: string,
		queryKey: readonly unknown[],
		hooks?: CleanupHooks<T>,
	) => void;
	cancelPending: (cancelScope: string) => void;
}

export function createFiniteReadCleanup<T>(
	queryClient: QueryClient,
	cancelBackend: (cancelScope: string) => Promise<T>,
): FiniteReadCleanup<T> {
	const pending = new Map<string, ReturnType<typeof setTimeout>>();

	function cancelPending(cancelScope: string): void {
		const timer = pending.get(cancelScope);
		if (!timer) return;
		clearTimeout(timer);
		pending.delete(cancelScope);
	}

	function schedule(
		cancelScope: string,
		queryKey: readonly unknown[],
		hooks: CleanupHooks<T> = {},
	): void {
		cancelPending(cancelScope);
		const timer = setTimeout(() => {
			pending.delete(cancelScope);
			const query = queryClient.getQueryCache().find({ queryKey, exact: true });
			if ((query?.getObserversCount() ?? 0) > 0) return;
			void (async () => {
				try {
					await queryClient.cancelQueries({ queryKey, exact: true });
				} catch (error) {
					hooks.onError?.(error);
				}
				const currentQuery = queryClient.getQueryCache().find({ queryKey, exact: true });
				if ((currentQuery?.getObserversCount() ?? 0) > 0) return;
				try {
					const result = await cancelBackend(cancelScope);
					hooks.onCancelled?.(result);
				} catch (error) {
					hooks.onError?.(error);
				}
			})();
		}, 0);
		pending.set(cancelScope, timer);
	}

	return { schedule, cancelPending };
}
