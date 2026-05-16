import { diagnosticLog } from "@/lib/diagnostics";
import {
	createTauriClient,
	listDynamicResources,
	listResources,
} from "@/lib/tauri";
import type { ResourceSummary } from "@/lib/types";
import {
	isDiscoveredResourceKind,
	resourceKindLabel,
	type FetchKey,
} from "./helpers";

export async function fetchResourcePage(
	clusterContext: string,
	fetchKeys: FetchKey[],
): Promise<ResourceSummary[]> {
	const started = performance.now();
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => resourceKindLabel(key.kind)).join("|"),
	});
	const client = createTauriClient();
	const results = await Promise.all(
		fetchKeys.map(({ kind, namespace }) => {
			const normalizedNamespace = namespace === "" ? undefined : namespace;
			if (isDiscoveredResourceKind(kind)) {
				return listDynamicResources(
					client,
					clusterContext,
					kind,
					normalizedNamespace,
				);
			}
			return listResources(client, clusterContext, kind, normalizedNamespace);
		}),
	);
	const rows = results.flat();
	diagnosticLog("resources.fetch.done", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		rows: rows.length,
		ms: Math.round(performance.now() - started),
	});
	return rows;
}
