import { diagnosticLog } from "@/lib/diagnostics";
import { createTauriClient, listResources } from "@/lib/tauri";
import type { ResourceSummary } from "@/lib/types";
import type { FetchKey } from "./helpers";

export async function fetchResourcePage(
	clusterContext: string,
	fetchKeys: FetchKey[],
): Promise<ResourceSummary[]> {
	const started = performance.now();
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => key.kind).join("|"),
	});
	const client = createTauriClient();
	const results = await Promise.all(
		fetchKeys.map(({ kind, namespace }) =>
			listResources(
				client,
				clusterContext,
				kind,
				namespace === "" ? undefined : namespace,
			),
		),
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
