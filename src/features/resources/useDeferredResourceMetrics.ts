import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
	createCancellableRequest,
	createCancelScope,
	useCancelBackendScopes,
} from "@/lib/cancellable-loads";
import { diagnosticLog } from "@/lib/diagnostics";
import { queryKeys } from "@/lib/queryKeys";
import { isAppError, listResourceMetrics, type TauriClient } from "@/lib/tauri";

const BACKGROUND_METRICS_DELAY_MS = 1_500;

interface UseDeferredResourceMetricsQueryArgs {
	client: TauriClient;
	clusterContext: string;
	namespaces: string[];
	kubeconfigEnvVar?: string;
	resourceRowsReady: boolean;
	resourceRowCount: number;
	waitForTopology: boolean;
	topologyPending: boolean;
}

export function useDeferredResourceMetricsQuery({
	client,
	clusterContext,
	namespaces,
	kubeconfigEnvVar,
	resourceRowsReady,
	resourceRowCount,
	waitForTopology,
	topologyPending,
}: UseDeferredResourceMetricsQueryArgs) {
	const queryKey = useMemo(
		() => queryKeys.resourceMetrics(clusterContext, namespaces, kubeconfigEnvVar),
		[clusterContext, namespaces, kubeconfigEnvVar],
	);
	const cancelScope = useMemo(
		() => createCancelScope("resource-metrics", queryKey),
		[queryKey],
	);
	const scopeKey = useMemo(() => JSON.stringify(queryKey), [queryKey]);
	const [queryReady, setQueryReady] = useState(false);
	const cancelEntries = useMemo(
		() => [
			{
				cancelScope,
				queryKey,
				event: "resources.metrics.cancel",
			},
		],
		[cancelScope, queryKey],
	);
	useCancelBackendScopes(client, cancelEntries);

	useEffect(() => {
		setQueryReady(false);
	}, [scopeKey]);
	useEffect(() => {
		if (
			queryReady ||
			!clusterContext ||
			!resourceRowsReady ||
			(waitForTopology && topologyPending)
		) {
			return;
		}
		diagnosticLog("resources.metrics.defer", {
			ms: BACKGROUND_METRICS_DELAY_MS,
			rows: resourceRowCount,
			mapOpen: waitForTopology,
		});
		const timerId = window.setTimeout(() => {
			diagnosticLog("resources.metrics.enable", {
				rows: resourceRowCount,
				mapOpen: waitForTopology,
			});
			setQueryReady(true);
		}, BACKGROUND_METRICS_DELAY_MS);
		return () => window.clearTimeout(timerId);
	}, [
		cancelScope,
		clusterContext,
		queryReady,
		resourceRowCount,
		resourceRowsReady,
		topologyPending,
		waitForTopology,
	]);

	return useQuery({
		queryKey,
		queryFn: () =>
			listResourceMetrics(
				client,
				clusterContext,
				namespaces,
				kubeconfigEnvVar,
				createCancellableRequest(cancelScope, "metrics"),
			).catch((error) => {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("resources.metrics.cancel", {
						namespaces: namespaces.length,
					});
				}
				throw error;
			}),
		enabled: queryReady && Boolean(clusterContext),
		retry: false,
		staleTime: 30_000,
	});
}
