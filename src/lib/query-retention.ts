import type { QueryClient } from "@tanstack/svelte-query";

export const LARGE_QUERY_ROOTS = [
	"resources",
	"resource-topology",
	"resource-metrics",
] as const;

export const LARGE_QUERY_GC_TIME_MS = 90_000;

export function configureLargeQueryRetention(queryClient: QueryClient): void {
	for (const root of LARGE_QUERY_ROOTS) {
		queryClient.setQueryDefaults([root], { gcTime: LARGE_QUERY_GC_TIME_MS });
	}
}
