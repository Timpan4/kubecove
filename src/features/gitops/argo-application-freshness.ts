import { queryKeys } from "@/lib/queryKeys";
import type { StreamMessage } from "@/lib/types";

export function argoListInvalidationKey(
	event: StreamMessage,
	kubeconfigEnvVar?: string,
): readonly unknown[] | null {
	if (event.type !== "resourceChanged") return null;
	if (event.target.kind === "Application") {
		return queryKeys.argoApps(event.target.cluster, kubeconfigEnvVar);
	}
	if (event.target.kind === "ApplicationSet") {
		return queryKeys.argoAppSets(event.target.cluster, kubeconfigEnvVar);
	}
	if (event.target.kind === "AppProject") {
		return queryKeys.argoAppProjects(event.target.cluster, kubeconfigEnvVar);
	}
	return null;
}

export function createArgoListFreshness(
	invalidate: (queryKey: readonly unknown[]) => void,
	kubeconfigEnvVar?: string,
) {
	let disposed = false;
	let debounce: ReturnType<typeof setTimeout> | null = null;
	const queryKeys = new Map<string, readonly unknown[]>();

	return {
		handle(event: StreamMessage) {
			const queryKey = argoListInvalidationKey(event, kubeconfigEnvVar);
			if (disposed || !queryKey) return;
			queryKeys.set(JSON.stringify(queryKey), queryKey);
			if (debounce) clearTimeout(debounce);
			debounce = setTimeout(() => {
				debounce = null;
				if (disposed) return;
				for (const queryKey of queryKeys.values()) invalidate(queryKey);
				queryKeys.clear();
			}, 250);
		},
		dispose() {
			disposed = true;
			if (debounce) clearTimeout(debounce);
			queryKeys.clear();
		},
	};
}
