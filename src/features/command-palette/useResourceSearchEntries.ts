import { useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
	buildFetchKeys,
	buildResourceSearchIndex,
	fetchResourcePage,
	type ResourceSearchEntry,
} from "@/features/resources";
import { queryKeys } from "@/lib/queryKeys";
import { kubeconfigSourceKey } from "@/lib/settings";
import { createTauriClient, listNamespaces } from "@/lib/tauri";
import type { ResourceSummary } from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspaces";
import { dedupeResources } from "./entries";

interface ResourceSearchSources {
	open: boolean;
	clusterContext: string;
	kubeconfigEnvVar?: string;
	workspace: SavedWorkspace | null;
}

interface ResourceSearchData {
	index: ResourceSearchEntry[];
	namespaces: string[];
}

/**
 * Search data for the command palette: a resource search index merged from
 * every cached resources query for the active cluster (warmed by the same
 * query ResourceList runs, so opening the palette reuses fresh cache), plus
 * the cluster's namespace names.
 */
export function useResourceSearchEntries({
	open,
	clusterContext,
	kubeconfigEnvVar,
	workspace,
}: ResourceSearchSources): ResourceSearchData {
	const queryClient = useQueryClient();
	const sourceKey = kubeconfigSourceKey(kubeconfigEnvVar);

	const fetchKeys = useMemo(
		() =>
			workspace
				? buildFetchKeys(workspace.scope.namespaces, workspace.scope.kinds)
				: [],
		[workspace],
	);

	const { data: warmupData } = useQuery({
		queryKey: queryKeys.resources(clusterContext, fetchKeys, kubeconfigEnvVar),
		queryFn: () => fetchResourcePage(clusterContext, fetchKeys, kubeconfigEnvVar),
		enabled: open && Boolean(clusterContext) && fetchKeys.length > 0,
		staleTime: 30_000,
	});

	const { data: namespaceData } = useQuery({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigEnvVar),
		queryFn: () =>
			listNamespaces(createTauriClient(), clusterContext, kubeconfigEnvVar),
		enabled: open && Boolean(clusterContext),
		staleTime: 30_000,
	});

	const index = useMemo(() => {
		if (!open) return [];
		const cached = queryClient.getQueriesData<ResourceSummary[]>({
			queryKey: ["resources", sourceKey, clusterContext],
		});
		const merged: ResourceSummary[] = [];
		for (const [, rows] of cached) {
			if (rows) merged.push(...rows);
		}
		return buildResourceSearchIndex(dedupeResources(merged));
		// warmupData feeds the same cache; its identity marks fresh data.
	}, [open, queryClient, sourceKey, clusterContext, warmupData]);

	const namespaces = useMemo(
		() => (namespaceData ?? []).map((namespace) => namespace.name),
		[namespaceData],
	);

	return { index, namespaces };
}
