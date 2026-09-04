import { queryOptions } from "@tanstack/svelte-query";
import { queryRetry } from "./query-retry";
import { getKubeconfigSources, type TauriClient } from "./tauri";

export const KUBECONFIG_SOURCES_QUERY_KEY = ["kubeconfig-sources"] as const;

export function kubeconfigSourcesQueryOptions(client: TauriClient) {
	return queryOptions({
		queryKey: KUBECONFIG_SOURCES_QUERY_KEY,
		queryFn: () => getKubeconfigSources(client),
		staleTime: 60_000,
		retry: queryRetry,
	});
}
